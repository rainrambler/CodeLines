/// Analyzer module: parses file content line by line, classifying each line
/// as code, comment, blank, or mixed (code + comment).
use crate::language::{
    BlockComment, BoundaryAfter, CharLiteral, CommentRule, LineComment, LineScope, RegionComment,
    StringEscape, StringLiteral,
};

/// Per-file line-level statistics result
#[derive(Debug, Clone, Default)]
pub struct LineStats {
    /// Total lines in the file
    pub total_lines: usize,
    /// Pure code lines (without comments)
    pub code_lines: usize,
    /// Pure comment lines (without code)
    pub comment_lines: usize,
    /// Mixed lines (code + comment on the same line)
    pub mixed_lines: usize,
    /// Blank lines
    pub blank_lines: usize,
}

impl std::ops::Add for LineStats {
    type Output = LineStats;

    fn add(self, rhs: LineStats) -> LineStats {
        LineStats {
            total_lines: self.total_lines + rhs.total_lines,
            code_lines: self.code_lines + rhs.code_lines,
            comment_lines: self.comment_lines + rhs.comment_lines,
            mixed_lines: self.mixed_lines + rhs.mixed_lines,
            blank_lines: self.blank_lines + rhs.blank_lines,
        }
    }
}

impl std::ops::AddAssign for LineStats {
    fn add_assign(&mut self, rhs: LineStats) {
        self.total_lines += rhs.total_lines;
        self.code_lines += rhs.code_lines;
        self.comment_lines += rhs.comment_lines;
        self.mixed_lines += rhs.mixed_lines;
        self.blank_lines += rhs.blank_lines;
    }
}

impl LineStats {
    /// Effective code lines (pure code + mixed)
    pub fn effective_code_lines(&self) -> usize {
        self.code_lines + self.mixed_lines
    }

    /// Total comment lines (pure comment + mixed)
    pub fn total_comment_lines(&self) -> usize {
        self.comment_lines + self.mixed_lines
    }

    /// Code ratio
    pub fn code_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.effective_code_lines() as f64 / self.total_lines as f64
        }
    }

    /// Comment ratio
    pub fn comment_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.total_comment_lines() as f64 / self.total_lines as f64
        }
    }
}

/// Compiled form of a [`CommentRule`] used by the analyzer.
#[derive(Debug, Clone)]
pub struct CompiledSyntax {
    strings: Vec<StringLiteral>,
    blocks: Vec<BlockComment>,
    lines: Vec<LineComment>,
    chars: Vec<CharLiteral>,
    regions: Vec<RegionComment>,
    heredocs: Vec<&'static str>,
}

impl CompiledSyntax {
    pub fn new(rule: &CommentRule) -> Self {
        let mut strings = rule.strings.clone();
        strings.extend(rule.string_delimiters.iter().map(|d| StringLiteral::new(d)));
        strings.sort_by_key(|a| std::cmp::Reverse(a.open.chars().count()));

        let mut blocks = rule.blocks.clone();
        if let (Some(start), Some(end)) = (rule.block_comment_start, rule.block_comment_end) {
            blocks.push(BlockComment::new(start, end));
        }
        blocks.sort_by_key(|a| std::cmp::Reverse(a.start.chars().count()));

        let mut lines = rule.line.clone();
        lines.extend(rule.line_comments.iter().map(|p| LineComment::new(p)));
        lines.sort_by_key(|a| std::cmp::Reverse(a.prefix.chars().count()));

        let mut regions = rule.regions.clone();
        regions.sort_by_key(|a| std::cmp::Reverse(a.start.chars().count()));

        Self {
            strings,
            blocks,
            lines,
            chars: rule.chars.clone(),
            regions,
            heredocs: rule.heredocs.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum HeredocStrip {
    None,
    Tabs,
    Whitespace,
}

#[derive(Debug, Clone, Default)]
enum Mode {
    #[default]
    Normal,
    Block {
        rule: usize,
        depth: usize,
    },
    String {
        rule: usize,
    },
    Region {
        rule: usize,
    },
    Heredoc {
        terminator: String,
        strip: HeredocStrip,
    },
}

#[derive(Debug, Clone, Default)]
struct ParseState {
    mode: Mode,
}

/// Analyze a single file's content, returning per-line statistics.
pub fn analyze_content(content: &str, rule: &CommentRule) -> LineStats {
    analyze_content_compiled(content, &CompiledSyntax::new(rule))
}

/// Same as [`analyze_content`], but reuses an already compiled syntax.
pub fn analyze_content_compiled(content: &str, syntax: &CompiledSyntax) -> LineStats {
    let mut stats = LineStats::default();
    let mut state = ParseState::default();

    for raw_line in content.lines() {
        stats.total_lines += 1;

        if raw_line.trim().is_empty() {
            stats.blank_lines += 1;
            continue;
        }

        let (has_code, has_comment, new_state) = parse_line(raw_line, syntax, &state);
        state = new_state;

        if has_code && has_comment {
            stats.mixed_lines += 1;
        } else if has_comment {
            stats.comment_lines += 1;
        } else if has_code {
            stats.code_lines += 1;
        } else {
            stats.blank_lines += 1;
        }
    }

    stats
}

/// Parse one line of text, returning (has_code, has_comment, new_state)
fn parse_line(line: &str, syntax: &CompiledSyntax, state: &ParseState) -> (bool, bool, ParseState) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let first_nonspace = chars.iter().position(|c| !c.is_whitespace());
    let mut pos = 0;
    let mut has_code = false;
    let mut has_comment = false;
    let mut mode = state.mode.clone();
    let mut pending_heredoc = None;

    if let Mode::Heredoc { terminator, strip } = mode.clone() {
        let candidate = match strip {
            HeredocStrip::None => line,
            HeredocStrip::Tabs => line.trim_start_matches('\t'),
            HeredocStrip::Whitespace => line.trim_start(),
        };
        if candidate.trim_end() == terminator {
            return (true, false, ParseState { mode: Mode::Normal });
        }
        return (
            true,
            false,
            ParseState {
                mode: Mode::Heredoc { terminator, strip },
            },
        );
    }

    loop {
        match mode.clone() {
            Mode::Block { rule, depth } => {
                let block = &syntax.blocks[rule];
                if pos >= len {
                    break;
                }
                if scope_matches(
                    block.end_scope,
                    &chars,
                    pos,
                    first_nonspace,
                    block.end.chars().count(),
                ) && starts_with(&chars, pos, block.end)
                {
                    has_comment = true;
                    pos += block.end.chars().count();
                    if depth <= 1 {
                        mode = Mode::Normal;
                    } else {
                        mode = Mode::Block {
                            rule,
                            depth: depth - 1,
                        };
                    }
                    continue;
                }
                if block.nested && starts_with(&chars, pos, block.start) {
                    has_comment = true;
                    pos += block.start.chars().count();
                    mode = Mode::Block {
                        rule,
                        depth: depth + 1,
                    };
                    continue;
                }
                has_comment = true;
                pos += 1;
                continue;
            }
            Mode::String { rule } => {
                let string = &syntax.strings[rule];
                if pos >= len {
                    break;
                }
                let escape = string.escape;
                if escape == StringEscape::Backslash && chars[pos] == '\\' && pos + 1 < len {
                    has_code = true;
                    pos += 2;
                    continue;
                }
                if escape == StringEscape::Backtick && chars[pos] == '`' && pos + 1 < len {
                    has_code = true;
                    pos += 2;
                    continue;
                }
                if scope_matches(
                    string.close_scope,
                    &chars,
                    pos,
                    first_nonspace,
                    string.close.chars().count(),
                ) && starts_with(&chars, pos, string.close)
                {
                    let close_len = string.close.chars().count();
                    if escape == StringEscape::DoubledQuote
                        && starts_with(&chars, pos + close_len, string.close)
                    {
                        has_code = true;
                        pos += close_len * 2;
                        continue;
                    }
                    has_code = true;
                    pos += close_len;
                    mode = Mode::Normal;
                    continue;
                }
                has_code = true;
                pos += 1;
                continue;
            }
            Mode::Region { rule } => {
                let region = &syntax.regions[rule];
                if pos >= len {
                    break;
                }
                if scope_matches(
                    region.end_scope,
                    &chars,
                    pos,
                    first_nonspace,
                    region.end.chars().count(),
                ) && starts_with(&chars, pos, region.end)
                {
                    has_comment = true;
                    pos += region.end.chars().count();
                    mode = Mode::Normal;
                    continue;
                }
                has_comment = true;
                pos += 1;
                continue;
            }
            Mode::Heredoc { .. } => break,
            Mode::Normal => {}
        }

        if pos >= len {
            break;
        }

        // Whitespace never counts as code, otherwise indentation before a
        // comment would turn a pure comment line into a mixed line.
        if chars[pos].is_whitespace() {
            pos += 1;
            continue;
        }

        if chars[pos] == '\\' && pos + 1 < len {
            has_code = true;
            pos += 2;
            continue;
        }

        if let Some((next, terminator, strip)) = match_heredoc(&chars, pos, syntax) {
            has_code = true;
            pos = next;
            pending_heredoc = Some(Mode::Heredoc { terminator, strip });
            continue;
        }

        if let Some(next) = match_char_literal(&chars, pos, &syntax.chars) {
            has_code = true;
            pos = next;
            continue;
        }

        let mut matched = false;
        for (index, string) in syntax.strings.iter().enumerate() {
            if starts_with(&chars, pos, string.open) && string_open_allowed(string, &chars, pos) {
                has_code = true;
                pos += string.open.chars().count();
                mode = Mode::String { rule: index };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        for (index, block) in syntax.blocks.iter().enumerate() {
            if scope_matches(
                block.start_scope,
                &chars,
                pos,
                first_nonspace,
                block.start.chars().count(),
            ) && starts_with(&chars, pos, block.start)
            {
                has_comment = true;
                pos += block.start.chars().count();
                mode = Mode::Block {
                    rule: index,
                    depth: 1,
                };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        for (index, region) in syntax.regions.iter().enumerate() {
            if scope_matches(
                region.start_scope,
                &chars,
                pos,
                first_nonspace,
                region.start.chars().count(),
            ) && starts_with(&chars, pos, region.start)
            {
                has_comment = true;
                pos += region.start.chars().count();
                mode = Mode::Region { rule: index };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        for line_comment in &syntax.lines {
            if line_comment_matches(&chars, pos, first_nonspace, line_comment) {
                has_comment = true;
                pos = len;
                matched = true;
                break;
            }
        }
        if matched {
            break;
        }

        has_code = true;
        pos += 1;
    }

    let reset_single_line_string = match &mode {
        Mode::String { rule } => !syntax.strings[*rule].multiline,
        _ => false,
    };
    if reset_single_line_string {
        mode = Mode::Normal;
    }

    let mode = pending_heredoc.unwrap_or(mode);
    (has_code, has_comment, ParseState { mode })
}

fn scope_matches(
    scope: LineScope,
    chars: &[char],
    pos: usize,
    first_nonspace: Option<usize>,
    prefix_len: usize,
) -> bool {
    match scope {
        LineScope::Anywhere => true,
        LineScope::StartOfLine => pos == 0,
        LineScope::FirstNonSpace => Some(pos) == first_nonspace,
        LineScope::AloneOnLine => {
            Some(pos) == first_nonspace
                && chars
                    .get(pos + prefix_len..)
                    .is_none_or(|rest| rest.iter().all(|c| c.is_whitespace()))
        }
    }
}

fn string_open_allowed(string: &StringLiteral, chars: &[char], pos: usize) -> bool {
    if !string.open_requires_line_end {
        return true;
    }
    chars
        .get(pos + string.open.chars().count()..)
        .is_none_or(|rest| rest.iter().all(|c| c.is_whitespace()))
}

fn line_comment_matches(
    chars: &[char],
    pos: usize,
    first_nonspace: Option<usize>,
    rule: &LineComment,
) -> bool {
    if !scope_matches(
        rule.scope,
        chars,
        pos,
        first_nonspace,
        rule.prefix.chars().count(),
    ) {
        return false;
    }
    if rule.require_whitespace_before && pos > 0 {
        let previous = chars[pos - 1];
        if !previous.is_whitespace() && !matches!(previous, ';' | '|' | '&' | '(' | ')') {
            return false;
        }
    }
    let matched = if rule.case_insensitive {
        starts_with_ignore_case(chars, pos, rule.prefix)
    } else {
        starts_with(chars, pos, rule.prefix)
    };
    if !matched {
        return false;
    }
    match rule.boundary_after {
        None => true,
        Some(BoundaryAfter::Word) => chars
            .get(pos + rule.prefix.chars().count())
            .is_none_or(|c| !(c.is_alphanumeric() || *c == '_')),
        Some(BoundaryAfter::Symbol) => chars
            .get(pos + rule.prefix.chars().count())
            .is_none_or(|c| !is_haskell_symbol(*c)),
    }
}

fn match_heredoc(
    chars: &[char],
    pos: usize,
    syntax: &CompiledSyntax,
) -> Option<(usize, String, HeredocStrip)> {
    for marker in &syntax.heredocs {
        if !starts_with(chars, pos, marker) {
            continue;
        }
        // `<<<` is a here-string in shells, not a heredoc.
        if *marker == "<<" && chars.get(pos + 2) == Some(&'<') {
            continue;
        }
        let mut index = pos + marker.chars().count();
        let mut strip = HeredocStrip::None;
        match chars.get(index) {
            Some('-') => {
                strip = HeredocStrip::Tabs;
                index += 1;
            }
            Some('~') => {
                strip = HeredocStrip::Whitespace;
                index += 1;
            }
            _ => {}
        }
        // A whitespace after the marker is more likely an operator
        // (`arr << x`, `x << 2`) than a heredoc; require the delimiter
        // to follow directly.
        if chars.get(index).is_some_and(|c| c.is_whitespace()) {
            continue;
        }
        let quote = match chars.get(index) {
            Some('\'') => Some('\''),
            Some('"') => Some('"'),
            _ => None,
        };
        if quote.is_some() {
            index += 1;
        }
        let start = index;
        if !chars
            .get(start)
            .is_some_and(|current| current.is_alphabetic() || *current == '_')
        {
            continue;
        }
        while index < chars.len() {
            let current = chars[index];
            if current.is_whitespace()
                || matches!(current, ';' | '|' | '&' | '(' | ')')
                || Some(current) == quote
            {
                break;
            }
            index += 1;
        }
        if index == start {
            continue;
        }
        let terminator: String = chars[start..index].iter().collect();
        if let Some(quote) = quote {
            if chars.get(index) != Some(&quote) {
                continue;
            }
            index += 1;
        }
        return Some((index, terminator, strip));
    }
    None
}

fn match_char_literal(chars: &[char], pos: usize, rules: &[CharLiteral]) -> Option<usize> {
    for rule in rules {
        if chars.get(pos) != Some(&rule.quote) {
            continue;
        }
        if let Some(end) = char_literal_end(chars, pos, *rule) {
            return Some(end);
        }
    }
    None
}

fn char_literal_end(chars: &[char], pos: usize, rule: CharLiteral) -> Option<usize> {
    let index = pos + 1;
    if index >= chars.len() {
        return None;
    }
    if chars[index] == '\\' {
        let limit = (index + 24).min(chars.len());
        return chars[index + 1..limit]
            .iter()
            .position(|character| *character == rule.quote)
            .map(|offset| index + 1 + offset + 1);
    }
    let limit = (pos + 2 + rule.max_len).min(chars.len());
    for (offset, character) in chars[pos + 1..limit].iter().enumerate() {
        if *character == rule.quote {
            return Some(pos + 1 + offset + 1);
        }
        if character.is_whitespace() {
            return None;
        }
    }
    None
}

fn is_haskell_symbol(character: char) -> bool {
    matches!(
        character,
        '!' | '#'
            | '$'
            | '%'
            | '&'
            | '*'
            | '+'
            | '.'
            | '/'
            | '<'
            | '='
            | '>'
            | '?'
            | '@'
            | '\\'
            | '^'
            | '|'
            | '-'
            | '~'
            | ':'
    )
}

/// Returns true if chars[pos..] starts with target
fn starts_with(chars: &[char], pos: usize, target: &str) -> bool {
    target
        .chars()
        .enumerate()
        .all(|(offset, expected)| chars.get(pos + offset) == Some(&expected))
}

fn starts_with_ignore_case(chars: &[char], pos: usize, target: &str) -> bool {
    target.chars().enumerate().all(|(offset, expected)| {
        chars
            .get(pos + offset)
            .is_some_and(|actual| actual.eq_ignore_ascii_case(&expected))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_rule() -> CommentRule {
        CommentRule::default()
            .line_rule(LineComment::new("//"))
            .block_rule(BlockComment::new("/*", "*/").nested())
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::raw("r#\"", "\"#"))
            .char_rule('\'', 1)
    }

    fn python_rule() -> CommentRule {
        CommentRule {
            line_comments: vec!["#"],
            block_comment_start: Some("\"\"\""),
            block_comment_end: Some("\"\"\""),
            string_delimiters: vec!["\"\"\"", "'''", "\"", "'"],
            ..Default::default()
        }
    }

    fn shell_rule() -> CommentRule {
        CommentRule::default()
            .line_rule(LineComment::new("#").require_whitespace_before())
            .string_rule(StringLiteral::with_escape(
                "\"",
                "\"",
                StringEscape::Backslash,
            ))
            .string_rule(StringLiteral::with_escape("'", "'", StringEscape::None))
            .heredoc("<<")
    }

    fn html_rule() -> CommentRule {
        CommentRule {
            line_comments: vec![],
            block_comment_start: Some("<!--"),
            block_comment_end: Some("-->"),
            string_delimiters: vec!["\"", "'"],
            ..Default::default()
        }
    }

    fn in_block(state: &ParseState) -> bool {
        matches!(state.mode, Mode::Block { .. })
    }

    fn in_string(state: &ParseState) -> bool {
        matches!(state.mode, Mode::String { .. })
    }

    fn default_state() -> ParseState {
        ParseState::default()
    }

    fn parse(line: &str, rule: &CommentRule, state: &ParseState) -> (bool, bool, ParseState) {
        parse_line(line, &CompiledSyntax::new(rule), state)
    }

    #[test]
    fn test_rust_line_comment() {
        let (code, comment, state) = parse("// this is a comment", &rust_rule(), &default_state());
        assert!(!code);
        assert!(comment);
        assert!(!in_block(&state));
        assert!(!in_string(&state));
    }

    #[test]
    fn test_rust_code_line() {
        let (code, comment, state) = parse("let x = 5;", &rust_rule(), &default_state());
        assert!(code);
        assert!(!comment);
        assert!(!in_block(&state));
    }

    #[test]
    fn test_rust_mixed_line() {
        let (code, comment, _) = parse("let x = 5; // comment", &rust_rule(), &default_state());
        assert!(code);
        assert!(comment);
    }

    #[test]
    fn test_string_with_comment_chars() {
        let (code, comment, _) = parse(
            "let s = \"// not a comment\";",
            &rust_rule(),
            &default_state(),
        );
        assert!(code);
        assert!(!comment);
    }

    #[test]
    fn test_python_triple_quote_string() {
        let (code, _comment, state) = parse("\"\"\"docstring", &python_rule(), &default_state());
        assert!(in_string(&state));
        assert!(code);
    }

    #[test]
    fn test_python_multiline_string_continuation() {
        let (_, _, state) = parse("\"\"\"", &python_rule(), &default_state());
        assert!(in_string(&state));

        let (code, comment, state) = parse("this is inside the string", &python_rule(), &state);
        assert!(code);
        assert!(!comment);
        assert!(in_string(&state));

        let (code, _, state) = parse("\"\"\"", &python_rule(), &state);
        assert!(code);
        assert!(!in_string(&state));
    }

    #[test]
    fn test_full_rust_file() {
        let content = r#"// A simple Rust program
fn main() {
    /* multi-line
       comment */
    let x = 5; // inline
}
"#;
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.total_lines, 6);
        assert_eq!(stats.blank_lines, 0);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 2);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_python_file() {
        let content = r#""""This is a docstring"""
def hello():
    # a comment
    print("hello")
"#;
        let stats = analyze_content(content, &python_rule());
        assert_eq!(stats.total_lines, 4);
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_shell_comment() {
        let (code, comment, _) = parse("# this is a comment", &shell_rule(), &default_state());
        assert!(!code);
        assert!(comment);
    }

    #[test]
    fn test_html_comment() {
        let (code, comment, _) = parse("<!-- comment -->", &html_rule(), &default_state());
        assert!(!code);
        assert!(comment);
    }

    #[test]
    fn test_rust_block_comment_multiline() {
        let content = "/* this is\na block\ncomment */\ncode_here\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.total_lines, 4);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_rust_code_with_block_comment_on_same_line() {
        let (code, comment, state) =
            parse("let x = /* inline */ 5;", &rust_rule(), &default_state());
        assert!(code);
        assert!(comment);
        assert!(!in_block(&state));
    }

    #[test]
    fn test_rust_lifetime_is_not_a_string() {
        let content = "let x: &'a str = \"value\"; // real comment\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.mixed_lines, 1);
        assert_eq!(stats.comment_lines, 0);
    }

    #[test]
    fn test_rust_raw_string_ignores_comment_markers() {
        let content = "let s = r#\"// not a comment\"#;\n// real comment\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.mixed_lines, 0);
    }

    #[test]
    fn test_nested_block_comments() {
        let content = "/* outer /* inner */ still outer */\nfn main() {}\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.mixed_lines, 0);
    }

    #[test]
    fn test_vim_double_quote_is_a_comment() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("\""))
            .string_rule(StringLiteral::doubled("'", "'").single_line());
        let content = "\" a comment\nlet x = 'a''b'\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_smalltalk_string_is_a_comment() {
        let rule = CommentRule::default()
            .block_rule(BlockComment::new("\"", "\""))
            .string_rule(StringLiteral::doubled("'", "'"));
        let content = "\"a comment\"\nTranscript show: 'it''s'\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_shell_hash_requires_word_boundary() {
        let content = "echo ${#var}\necho hi # real comment\n";
        let stats = analyze_content(content, &shell_rule());
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_shell_heredoc_body_is_code() {
        let content = "cat <<EOF\n# not a comment\nEOF\n";
        let stats = analyze_content(content, &shell_rule());
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 0);
    }

    #[test]
    fn test_haskell_operator_is_not_a_comment() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("--").with_boundary_after(BoundaryAfter::Symbol))
            .block_rule(BlockComment::new("{-", "-}").nested())
            .string_rule(StringLiteral::new("\""))
            .char_rule('\'', 1);
        let content = "a --> b\n-- real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 1);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_batch_rem_is_case_insensitive() {
        let rule = CommentRule::default()
            .line_rule(
                LineComment::first_non_space("rem")
                    .case_insensitive()
                    .with_boundary_after(BoundaryAfter::Word),
            )
            .line_rule(LineComment::first_non_space("::"))
            .string_rule(StringLiteral::new("\""));
        let content = "REM a comment\nremove file.txt\n:: label comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_sql_string_with_comment_marker() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("--"))
            .block_rule(BlockComment::new("/*", "*/"))
            .string_rule(StringLiteral::doubled("'", "'"));
        let content = "SELECT '-- not a comment' -- real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_char_literals_and_escapes() {
        let content = "let c = '\\n'; // real comment\nlet f = '\\u{1F600}';\n";
        let stats = analyze_content(content, &rust_rule());
        assert_eq!(stats.mixed_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_multiple_block_syntaxes() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("//"))
            .block_rule(BlockComment::new("{", "}"))
            .block_rule(BlockComment::new("(*", "*)"))
            .string_rule(StringLiteral::doubled("'", "'"));
        let content = "{ comment }\n(* another *)\nwriteln('x');\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_lua_long_comments_and_raw_strings() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("--"))
            .block_rule(BlockComment::new("--[[", "]]"))
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .string_rule(StringLiteral::raw("[[", "]]"));
        let content = "--[[ block\nstill ]]\nlocal s = [[-- not a comment]]\n-- real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_powershell_here_string() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .block_rule(BlockComment::new("<#", "#>"))
            .string_rule(StringLiteral::with_escape(
                "\"",
                "\"",
                StringEscape::Backtick,
            ))
            .string_rule(StringLiteral::doubled("'", "'"))
            .string_rule(
                StringLiteral::with_escape("@\"", "\"@", StringEscape::Backtick)
                    .close_at_start()
                    .open_at_line_end(),
            )
            .string_rule(
                StringLiteral::with_escape("@'", "'@", StringEscape::None)
                    .close_at_start()
                    .open_at_line_end(),
            );
        let content = "$x = @\"\n# not a comment\n\"@\n# real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_perl_pod_region() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .region_rule(RegionComment::new("=", "=cut").at_start().end_at_start())
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .heredoc("<<");
        let content = "=pod\n# pod text\n=cut\nprint \"hi\"; # real comment\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.mixed_lines, 1);
    }
}
