/// Analyzer module: parses file content line by line, classifying each line
/// as code, comment, blank, or mixed (code + comment).
use crate::language::CommentRule;

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

/// String delimiter info
#[derive(Debug, Clone)]
struct DelimInfo {
    delim: String,
    char_count: usize,
}

/// State maintained across lines during parsing
#[derive(Debug, Clone, Default)]
struct ParseState {
    /// Whether inside a block comment
    in_block_comment: bool,
    /// Whether inside a multi-line string, and its delimiter char count
    in_string: Option<usize>,
}

/// Analyze a single file's content, returning per-line statistics.
pub fn analyze_content(content: &str, rule: &CommentRule) -> LineStats {
    let mut stats = LineStats::default();
    let mut state = ParseState::default();

    // Preprocess string delimiter info, sorted by length descending
    // (match longer ones first, e.g. """ before ")
    let mut string_delims: Vec<DelimInfo> = rule
        .string_delimiters
        .iter()
        .map(|d| DelimInfo {
            delim: d.to_string(),
            char_count: d.chars().count(),
        })
        .collect();
    string_delims.sort_by(|a, b| b.char_count.cmp(&a.char_count));

    for raw_line in content.lines() {
        stats.total_lines += 1;

        let trimmed = raw_line.trim();

        // Blank lines inside multi-line strings or block comments are also categorized
        if trimmed.is_empty() && state.in_string.is_none() && !state.in_block_comment {
            stats.blank_lines += 1;
            continue;
        }

        // Parse this line
        let (has_code, has_comment, new_state) =
            parse_line(trimmed, rule, &state, &string_delims);

        state = new_state;

        if has_code && has_comment {
            stats.mixed_lines += 1;
        } else if has_comment {
            stats.comment_lines += 1;
        } else if has_code {
            stats.code_lines += 1;
        } else {
            // Pure blank or empty line inside block comment / string
            stats.blank_lines += 1;
        }
    }

    stats
}

/// Parse one line of text, returning (has_code, has_comment, new_state)
fn parse_line(
    line: &str,
    rule: &CommentRule,
    state: &ParseState,
    string_delims: &[DelimInfo],
) -> (bool, bool, ParseState) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut pos = 0;

    let mut has_code = false;
    let mut has_comment = false;
    let mut in_block_comment = state.in_block_comment;
    let mut in_string = state.in_string;

    let block_start_len = rule.block_comment_start.map(|s| s.chars().count());
    let block_end_len = rule.block_comment_end.map(|s| s.chars().count());

    while pos < len {
        // ─── Inside block comment ───
        if in_block_comment {
            if let Some(end) = rule.block_comment_end {
                if starts_with(&chars, pos, end) {
                    has_comment = true;
                    pos += block_end_len.unwrap();
                    in_block_comment = false;
                    continue;
                }
            }
            has_comment = true;
            pos += 1;
            continue;
        }

        // ─── Inside multi-line string ───
        if let Some(delim_len) = in_string {
            // Skip escaped characters
            if chars[pos] == '\\' && pos + 1 < len {
                has_code = true;
                pos += 2;
                continue;
            }
            // Detect string closing
            let mut closed = false;
            for di in string_delims {
                if di.char_count == delim_len && starts_with(&chars, pos, &di.delim) {
                    has_code = true;
                    in_string = None;
                    pos += delim_len;
                    closed = true;
                    break;
                }
            }
            if closed {
                continue;
            }
            has_code = true;
            pos += 1;
            continue;
        }

        // ─── Normal state ───

        // Escape character
        if chars[pos] == '\\' && pos + 1 < len {
            has_code = true;
            pos += 2;
            continue;
        }

        // Detect string delimiter (match longer ones first, descending length)
        let mut found_string = false;
        for di in string_delims {
            if starts_with(&chars, pos, &di.delim) {
                in_string = Some(di.char_count);
                pos += di.char_count;
                found_string = true;
                break;
            }
        }
        if found_string {
            continue;
        }

        // Detect block comment start
        if let Some(start) = rule.block_comment_start {
            if starts_with(&chars, pos, start) {
                has_comment = true;
                in_block_comment = true;
                pos += block_start_len.unwrap();
                continue;
            }
        }

        // Detect line comment
        let mut found_line_comment = false;
        for lc in &rule.line_comments {
            if starts_with(&chars, pos, lc) {
                has_comment = true;
                pos = len;
                found_line_comment = true;
                break;
            }
        }
        if found_line_comment {
            continue;
        }

        // Regular code character
        has_code = true;
        pos += 1;
    }

    (
        has_code,
        has_comment,
        ParseState {
            in_block_comment,
            in_string,
        },
    )
}

/// Returns true if chars[pos..] starts with target
fn starts_with(chars: &[char], pos: usize, target: &str) -> bool {
    let target_chars: Vec<char> = target.chars().collect();
    if pos + target_chars.len() > chars.len() {
        return false;
    }
    for (i, tc) in target_chars.iter().enumerate() {
        if chars[pos + i] != *tc {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_rule() -> CommentRule {
        CommentRule {
            line_comments: vec!["//"],
            block_comment_start: Some("/*"),
            block_comment_end: Some("*/"),
            string_delimiters: vec!["\"", "'"],
        }
    }

    fn python_rule() -> CommentRule {
        CommentRule {
            line_comments: vec!["#"],
            block_comment_start: Some("\"\"\""),
            block_comment_end: Some("\"\"\""),
            string_delimiters: vec!["\"\"\"", "'''", "\"", "'"],
        }
    }

    fn shell_rule() -> CommentRule {
        CommentRule {
            line_comments: vec!["#"],
            block_comment_start: None,
            block_comment_end: None,
            string_delimiters: vec!["\"", "'"],
        }
    }

    fn html_rule() -> CommentRule {
        CommentRule {
            line_comments: vec![],
            block_comment_start: Some("<!--"),
            block_comment_end: Some("-->"),
            string_delimiters: vec!["\"", "'"],
        }
    }

    fn default_state() -> ParseState {
        ParseState {
            in_block_comment: false,
            in_string: None,
        }
    }

    fn parse(line: &str, rule: &CommentRule, state: &ParseState) -> (bool, bool, ParseState) {
        let string_delims: Vec<DelimInfo> = rule
            .string_delimiters
            .iter()
            .map(|d| DelimInfo {
                delim: d.to_string(),
                char_count: d.chars().count(),
            })
            .collect();
        parse_line(line, rule, state, &string_delims)
    }

    #[test]
    fn test_rust_line_comment() {
        let (code, comment, state) = parse("// this is a comment", &rust_rule(), &default_state());
        assert!(!code);
        assert!(comment);
        assert!(!state.in_block_comment);
        assert!(state.in_string.is_none());
    }

    #[test]
    fn test_rust_code_line() {
        let (code, comment, state) = parse("let x = 5;", &rust_rule(), &default_state());
        assert!(code);
        assert!(!comment);
        assert!(!state.in_block_comment);
    }

    #[test]
    fn test_rust_mixed_line() {
        let (code, comment, _) = parse("let x = 5; // comment", &rust_rule(), &default_state());
        assert!(code);
        assert!(comment);
    }

    #[test]
    fn test_string_with_comment_chars() {
        let (code, comment, _) = parse("let s = \"// not a comment\";", &rust_rule(), &default_state());
        assert!(code);
        assert!(!comment);
    }

    #[test]
    fn test_python_triple_quote_string() {
        // """ opens a string (takes priority over block comment matching)
        let (code, _comment, state) = parse("\"\"\"docstring", &python_rule(), &default_state());
        // It enters string mode because string delimiters are checked before block comments
        assert!(state.in_string.is_some());
        // The """ delimiter starts a string, and "docstring" is inside it, so has_code=true
        assert!(code);
    }

    #[test]
    fn test_python_multiline_string_continuation() {
        // First line opens a triple-quoted string
        let (_, _, state) = parse("\"\"\"", &python_rule(), &default_state());
        assert!(state.in_string.is_some());

        // Second line is inside the string
        let (code, comment, state) = parse("this is inside the string", &python_rule(), &state);
        assert!(code); // inside a string counts as code
        assert!(!comment);
        assert!(state.in_string.is_some());

        // Third line closes the string
        let (code, _, state) = parse("\"\"\"", &python_rule(), &state);
        assert!(code);
        assert!(state.in_string.is_none());
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
        // Line 1: "// A simple Rust program" → comment
        // Line 2: "fn main() {" → code
        // Line 3: "    /* multi-line" → comment (block start, no code before /*)
        // Line 4: "       comment */" → comment (block end)
        // Line 5: "    let x = 5; // inline" → mixed
        // Line 6: "}" → code
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
        // Line 1: """This is a docstring""" → code (it's a string expression)
        // Line 2: "def hello():" → code
        // Line 3: "    # a comment" → comment
        // Line 4: "    print("hello")" → code
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
        assert_eq!(stats.comment_lines, 3); // 3 lines inside block comment
        assert_eq!(stats.code_lines, 1);
    }

    #[test]
    fn test_rust_code_with_block_comment_on_same_line() {
        let (code, comment, state) = parse("let x = /* inline */ 5;", &rust_rule(), &default_state());
        assert!(code);
        assert!(comment);
        assert!(!state.in_block_comment);
    }
}
