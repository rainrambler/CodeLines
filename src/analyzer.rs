/// Analyzer module: parses file content line by line, classifying each line
/// as code, comment, blank, or mixed (code + comment).
use crate::language::{
    BlockComment, BoundaryAfter, CharLiteral, CommentRule, Heredoc, LineComment, LineScope,
    RawString, RegionComment, StringEscape, StringLiteral,
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
    raw_strings: Vec<RawString>,
    blocks: Vec<BlockComment>,
    lines: Vec<LineComment>,
    chars: Vec<CharLiteral>,
    regions: Vec<RegionComment>,
    heredocs: Vec<Heredoc>,
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

        let mut heredocs = rule.heredoc_rules.clone();
        heredocs.extend(rule.heredocs.iter().map(|marker| Heredoc::new(marker)));

        Self {
            strings,
            raw_strings: rule.raw_strings.clone(),
            blocks,
            lines,
            chars: rule.chars.clone(),
            regions,
            heredocs,
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
    RawString {
        close: String,
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
    // Delimiters are ASCII, so scanning bytes directly is safe: UTF-8
    // continuation bytes (>= 0x80) can never form an ASCII marker.
    let bytes = line.as_bytes();
    let len = bytes.len();
    let first_nonspace = bytes.iter().position(|b| !b.is_ascii_whitespace());
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
        match std::mem::take(&mut mode) {
            Mode::Block { rule, depth } => {
                let block = &syntax.blocks[rule];
                if pos >= len {
                    mode = Mode::Block { rule, depth };
                    break;
                }
                if scope_matches(block.end_scope, bytes, pos, first_nonspace, block.end.len())
                    && starts_with(bytes, pos, block.end)
                {
                    has_comment = true;
                    pos += block.end.len();
                    mode = if depth <= 1 {
                        Mode::Normal
                    } else {
                        Mode::Block {
                            rule,
                            depth: depth - 1,
                        }
                    };
                    continue;
                }
                if block.nested && starts_with(bytes, pos, block.start) {
                    has_comment = true;
                    pos += block.start.len();
                    mode = Mode::Block {
                        rule,
                        depth: depth + 1,
                    };
                    continue;
                }
                has_comment = true;
                pos += 1;
                mode = Mode::Block { rule, depth };
                continue;
            }
            Mode::String { rule } => {
                let string = &syntax.strings[rule];
                if pos >= len {
                    mode = Mode::String { rule };
                    break;
                }
                let escape = string.escape;
                if escape == StringEscape::Backslash && bytes[pos] == b'\\' && pos + 1 < len {
                    has_code = true;
                    pos += 2;
                    mode = Mode::String { rule };
                    continue;
                }
                if escape == StringEscape::Backtick && bytes[pos] == b'`' && pos + 1 < len {
                    has_code = true;
                    pos += 2;
                    mode = Mode::String { rule };
                    continue;
                }
                if scope_matches(
                    string.close_scope,
                    bytes,
                    pos,
                    first_nonspace,
                    string.close.len(),
                ) && starts_with(bytes, pos, string.close)
                {
                    let close_len = string.close.len();
                    if escape == StringEscape::DoubledQuote
                        && starts_with(bytes, pos + close_len, string.close)
                    {
                        has_code = true;
                        pos += close_len * 2;
                        mode = Mode::String { rule };
                        continue;
                    }
                    has_code = true;
                    pos += close_len;
                    mode = Mode::Normal;
                    continue;
                }
                has_code = true;
                pos += 1;
                mode = Mode::String { rule };
                continue;
            }
            Mode::RawString { close } => {
                if pos >= len {
                    mode = Mode::RawString { close };
                    break;
                }
                if starts_with(bytes, pos, &close) {
                    has_code = true;
                    pos += close.len();
                    mode = Mode::Normal;
                    continue;
                }
                has_code = true;
                pos += 1;
                mode = Mode::RawString { close };
                continue;
            }
            Mode::Region { rule } => {
                let region = &syntax.regions[rule];
                if pos >= len {
                    mode = Mode::Region { rule };
                    break;
                }
                if scope_matches(
                    region.end_scope,
                    bytes,
                    pos,
                    first_nonspace,
                    region.end.len(),
                ) && starts_with(bytes, pos, region.end)
                {
                    has_comment = true;
                    pos += region.end.len();
                    mode = Mode::Normal;
                    continue;
                }
                has_comment = true;
                pos += 1;
                mode = Mode::Region { rule };
                continue;
            }
            Mode::Heredoc { terminator, strip } => {
                mode = Mode::Heredoc { terminator, strip };
                break;
            }
            Mode::Normal => {}
        }

        if pos >= len {
            break;
        }

        // Whitespace never counts as code, otherwise indentation before a
        // comment would turn a pure comment line into a mixed line.
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
            continue;
        }

        if bytes[pos] == b'\\' && pos + 1 < len {
            has_code = true;
            pos += 2;
            continue;
        }

        if let Some((next, terminator, strip)) = match_heredoc(bytes, pos, syntax) {
            has_code = true;
            pos = next;
            pending_heredoc = Some(Mode::Heredoc { terminator, strip });
            continue;
        }

        let mut matched = false;
        for raw in &syntax.raw_strings {
            if !starts_with(bytes, pos, raw.prefix) {
                continue;
            }
            if let Some((next, close)) = match_raw_string(bytes, pos, raw) {
                has_code = true;
                pos = next;
                mode = Mode::RawString { close };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        if let Some(next) = match_char_literal(bytes, pos, &syntax.chars) {
            has_code = true;
            pos = next;
            continue;
        }

        for (index, string) in syntax.strings.iter().enumerate() {
            if starts_with(bytes, pos, string.open) && string_open_allowed(string, bytes, pos) {
                has_code = true;
                pos += string.open.len();
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
                bytes,
                pos,
                first_nonspace,
                block.start.len(),
            ) && starts_with(bytes, pos, block.start)
            {
                has_comment = true;
                pos += block.start.len();
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
                bytes,
                pos,
                first_nonspace,
                region.start.len(),
            ) && starts_with(bytes, pos, region.start)
            {
                has_comment = true;
                pos += region.start.len();
                mode = Mode::Region { rule: index };
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        for line_comment in &syntax.lines {
            if line_comment_matches(bytes, pos, first_nonspace, line_comment) {
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
    bytes: &[u8],
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
                && bytes
                    .get(pos + prefix_len..)
                    .is_none_or(|rest| rest.iter().all(|b| b.is_ascii_whitespace()))
        }
        LineScope::Column(column) => pos == column,
    }
}

fn string_open_allowed(string: &StringLiteral, bytes: &[u8], pos: usize) -> bool {
    if !string.open_requires_line_end {
        return true;
    }
    bytes
        .get(pos + string.open.len()..)
        .is_none_or(|rest| rest.iter().all(|b| b.is_ascii_whitespace()))
}

fn line_comment_matches(
    bytes: &[u8],
    pos: usize,
    first_nonspace: Option<usize>,
    rule: &LineComment,
) -> bool {
    if !scope_matches(rule.scope, bytes, pos, first_nonspace, rule.prefix.len()) {
        return false;
    }
    if rule.require_whitespace_before && pos > 0 {
        let previous = bytes[pos - 1];
        if !previous.is_ascii_whitespace() && !matches!(previous, b';' | b'|' | b'&' | b'(' | b')')
        {
            return false;
        }
    }
    let matched = if rule.case_insensitive {
        starts_with_ignore_case(bytes, pos, rule.prefix)
    } else {
        starts_with(bytes, pos, rule.prefix)
    };
    if !matched {
        return false;
    }
    match rule.boundary_after {
        None => true,
        Some(BoundaryAfter::Word) => bytes
            .get(pos + rule.prefix.len())
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_')),
        Some(BoundaryAfter::Symbol) => bytes
            .get(pos + rule.prefix.len())
            .is_none_or(|b| !is_haskell_symbol(*b)),
    }
}

fn match_heredoc(
    bytes: &[u8],
    pos: usize,
    syntax: &CompiledSyntax,
) -> Option<(usize, String, HeredocStrip)> {
    for rule in &syntax.heredocs {
        if !starts_with(bytes, pos, rule.marker) {
            continue;
        }
        // `<<<` is a here-string in shells, not a heredoc.
        if rule.marker == "<<" && bytes.get(pos + 2) == Some(&b'<') {
            continue;
        }
        let mut index = pos + rule.marker.len();
        let mut strip = HeredocStrip::None;
        match bytes.get(index) {
            Some(b'-') => {
                strip = HeredocStrip::Tabs;
                index += 1;
            }
            Some(b'~') => {
                strip = HeredocStrip::Whitespace;
                index += 1;
            }
            _ => {}
        }
        if bytes.get(index).is_some_and(|b| b.is_ascii_whitespace()) {
            if !rule.allow_space {
                continue;
            }
            while bytes.get(index).is_some_and(|b| b.is_ascii_whitespace()) {
                index += 1;
            }
        }
        let quote = match bytes.get(index) {
            Some(b'\'') => Some(b'\''),
            Some(b'"') => Some(b'"'),
            _ => None,
        };
        if quote.is_some() {
            index += 1;
        }
        let start = index;
        if !bytes
            .get(start)
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        {
            continue;
        }
        if quote.is_none() && rule.require_uppercase && !bytes[start].is_ascii_uppercase() {
            continue;
        }
        while index < bytes.len() {
            let current = bytes[index];
            if current.is_ascii_whitespace()
                || matches!(current, b';' | b'|' | b'&' | b'(' | b')')
                || Some(current) == quote
            {
                break;
            }
            index += 1;
        }
        if index == start {
            continue;
        }
        let terminator = String::from_utf8_lossy(&bytes[start..index]).into_owned();
        if let Some(quote) = quote {
            if bytes.get(index) != Some(&quote) {
                continue;
            }
            index += 1;
        }
        return Some((index, terminator, strip));
    }
    None
}

fn match_raw_string(bytes: &[u8], pos: usize, rule: &RawString) -> Option<(usize, String)> {
    if !rule.open.is_ascii() || !rule.close.is_ascii() {
        return None;
    }
    let open = rule.open as u8;
    let mut index = pos + rule.prefix.len();
    let delimiter_start = index;
    while index < bytes.len() {
        let current = bytes[index];
        if current == open {
            break;
        }
        if index - delimiter_start >= 16
            || current.is_ascii_whitespace()
            || matches!(current, b'\\' | b')' | b'"' | b'{' | b'}')
        {
            return None;
        }
        index += 1;
    }
    if bytes.get(index) != Some(&open) {
        return None;
    }
    let delimiter = std::str::from_utf8(&bytes[delimiter_start..index]).ok()?;
    Some((index + 1, format!("{}{}\"", rule.close, delimiter)))
}

fn match_char_literal(bytes: &[u8], pos: usize, rules: &[CharLiteral]) -> Option<usize> {
    for rule in rules {
        if !rule.quote.is_ascii() || bytes.get(pos) != Some(&(rule.quote as u8)) {
            continue;
        }
        if let Some(end) = char_literal_end(bytes, pos, *rule) {
            return Some(end);
        }
    }
    None
}

fn char_literal_end(bytes: &[u8], pos: usize, rule: CharLiteral) -> Option<usize> {
    let quote = rule.quote as u8;
    let index = pos + 1;
    if index >= bytes.len() {
        return None;
    }
    if bytes[index] == b'\\' {
        let limit = (index + 24).min(bytes.len());
        return bytes[index + 1..limit]
            .iter()
            .position(|current| *current == quote)
            .map(|offset| index + 1 + offset + 1);
    }
    let limit = (pos + 2 + rule.max_len).min(bytes.len());
    for (offset, current) in bytes[pos + 1..limit].iter().enumerate() {
        if *current == quote {
            return Some(pos + 1 + offset + 1);
        }
        if current.is_ascii_whitespace() {
            return None;
        }
    }
    None
}

fn is_haskell_symbol(current: u8) -> bool {
    matches!(
        current,
        b'!' | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'*'
            | b'+'
            | b'.'
            | b'/'
            | b'<'
            | b'='
            | b'>'
            | b'?'
            | b'@'
            | b'\\'
            | b'^'
            | b'|'
            | b'-'
            | b'~'
            | b':'
    )
}

/// Returns true if bytes[pos..] starts with target
fn starts_with(bytes: &[u8], pos: usize, target: &str) -> bool {
    target
        .as_bytes()
        .iter()
        .enumerate()
        .all(|(offset, expected)| bytes.get(pos + offset) == Some(expected))
}

fn starts_with_ignore_case(bytes: &[u8], pos: usize, target: &str) -> bool {
    target
        .as_bytes()
        .iter()
        .enumerate()
        .all(|(offset, expected)| {
            bytes
                .get(pos + offset)
                .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
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

    #[test]
    fn test_cpp_raw_string_with_custom_delimiter() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("//"))
            .block_rule(BlockComment::new("/*", "*/"))
            .string_rule(StringLiteral::new("\""))
            .raw_string_rule(RawString::cpp())
            .char_rule('\'', 8);

        let single = analyze_content(
            "auto s = R\"delim(// not a comment \" still)delim\"; // real\n",
            &rule,
        );
        assert_eq!(single.mixed_lines, 1);
        assert_eq!(single.comment_lines, 0);

        let multi = analyze_content("auto s = R\"x(\n// inside\n)x\";\n// real\n", &rule);
        assert_eq!(multi.code_lines, 3);
        assert_eq!(multi.comment_lines, 1);
    }

    #[test]
    fn test_ruby_heredoc_and_shift_operator() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .block_rule(
                BlockComment::new("=begin", "=end")
                    .at_start()
                    .end_at_start(),
            )
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .string_rule(StringLiteral::new("`"))
            .heredoc_rule(Heredoc::new("<<").require_uppercase());

        let content = "arr << x\nsql = <<~SQL\n# body\nSQL\n# real\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 4);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_perl_heredoc_and_shift_operator() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#"))
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::new("'"))
            .heredoc_rule(Heredoc::new("<<").require_uppercase());

        let content = "$x << 1;\nmy $s = <<EOF;\n# body\nEOF\n# real\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.code_lines, 4);
        assert_eq!(stats.comment_lines, 1);
    }

    #[test]
    fn test_shell_heredoc_with_space_after_marker() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("#").require_whitespace_before())
            .string_rule(StringLiteral::new("\""))
            .string_rule(StringLiteral::with_escape("'", "'", StringEscape::None))
            .heredoc_rule(Heredoc::new("<<").allow_space());

        let stats = analyze_content("cat << EOF\n# body\nEOF\n", &rule);
        assert_eq!(stats.code_lines, 3);
        assert_eq!(stats.comment_lines, 0);
    }

    #[test]
    fn test_cobol_indicator_area() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("*").with_scope(LineScope::Column(6)))
            .line_rule(LineComment::new("/").with_scope(LineScope::Column(6)))
            .line_rule(LineComment::new("*>"))
            .string_rule(StringLiteral::doubled("\"", "\""))
            .string_rule(StringLiteral::doubled("'", "'"));

        let content =
            "      * fixed comment\n      / page comment\n           DISPLAY 'HI'. *> inline\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 2);
        assert_eq!(stats.mixed_lines, 1);
    }

    #[test]
    fn test_sas_statement_comment_region() {
        let rule = CommentRule::default()
            .region_rule(RegionComment::new("*", ";").first_non_space())
            .block_rule(BlockComment::new("/*", "*/"))
            .string_rule(StringLiteral::doubled("\"", "\""))
            .string_rule(StringLiteral::doubled("'", "'"));

        let content = "* statement comment\ncontinues here;\ndata x;\n/* block */\n";
        let stats = analyze_content(content, &rule);
        assert_eq!(stats.comment_lines, 3);
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_unicode_content_does_not_panic() {
        let rule = CommentRule::default()
            .line_rule(LineComment::new("//"))
            .string_rule(StringLiteral::new("\""));
        let stats = analyze_content("// 中文注释\nlet s = \"中文 // 不是注释\";\n", &rule);
        assert_eq!(stats.comment_lines, 1);
        assert_eq!(stats.code_lines, 1);
    }
}
