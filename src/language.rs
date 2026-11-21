/// Language definition module: defines file extensions and comment rules
/// for each supported programming language.
use std::collections::HashMap;
use std::sync::OnceLock;

/// Controls where on a line a rule may start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineScope {
    /// Match anywhere on the line.
    #[default]
    Anywhere,
    /// Match only at column 0.
    StartOfLine,
    /// Match only at the first non-whitespace character.
    FirstNonSpace,
    /// Match only when the rest of the line is whitespace.
    AloneOnLine,
    /// Match only at the given zero-based byte column (COBOL indicator area).
    Column(usize),
}

/// Extra condition on the character following a line-comment prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryAfter {
    /// The next character must not be a letter, digit, or underscore.
    Word,
    /// The next character must not be a Haskell-style symbol character.
    Symbol,
}

/// A line comment such as `//`, `#` or `--`.
#[derive(Debug, Clone)]
pub struct LineComment {
    pub prefix: &'static str,
    pub scope: LineScope,
    pub case_insensitive: bool,
    pub boundary_after: Option<BoundaryAfter>,
    pub require_whitespace_before: bool,
}

impl LineComment {
    pub fn new(prefix: &'static str) -> Self {
        Self {
            prefix,
            scope: LineScope::Anywhere,
            case_insensitive: false,
            boundary_after: None,
            require_whitespace_before: false,
        }
    }

    pub fn at_start(prefix: &'static str) -> Self {
        Self::new(prefix).with_scope(LineScope::StartOfLine)
    }

    pub fn first_non_space(prefix: &'static str) -> Self {
        Self::new(prefix).with_scope(LineScope::FirstNonSpace)
    }

    pub fn with_scope(mut self, scope: LineScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn case_insensitive(mut self) -> Self {
        self.case_insensitive = true;
        self
    }

    pub fn with_boundary_after(mut self, boundary: BoundaryAfter) -> Self {
        self.boundary_after = Some(boundary);
        self
    }

    pub fn require_whitespace_before(mut self) -> Self {
        self.require_whitespace_before = true;
        self
    }
}

/// A block comment such as `/* ... */` or `{- ... -}`.
#[derive(Debug, Clone)]
pub struct BlockComment {
    pub start: &'static str,
    pub end: &'static str,
    pub nested: bool,
    pub start_scope: LineScope,
    pub end_scope: LineScope,
}

impl BlockComment {
    pub fn new(start: &'static str, end: &'static str) -> Self {
        Self {
            start,
            end,
            nested: false,
            start_scope: LineScope::Anywhere,
            end_scope: LineScope::Anywhere,
        }
    }

    pub fn nested(mut self) -> Self {
        self.nested = true;
        self
    }

    pub fn at_start(mut self) -> Self {
        self.start_scope = LineScope::StartOfLine;
        self
    }

    pub fn end_at_start(mut self) -> Self {
        self.end_scope = LineScope::StartOfLine;
        self
    }

    pub fn alone_on_line(mut self) -> Self {
        self.start_scope = LineScope::AloneOnLine;
        self
    }

    pub fn end_alone_on_line(mut self) -> Self {
        self.end_scope = LineScope::AloneOnLine;
        self
    }
}

/// How escape sequences are written inside a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringEscape {
    None,
    Backslash,
    Backtick,
    /// A doubled closing delimiter escapes itself (`""`, `''`).
    DoubledQuote,
}

/// A string literal rule.
#[derive(Debug, Clone)]
pub struct StringLiteral {
    pub open: &'static str,
    pub close: &'static str,
    pub escape: StringEscape,
    pub multiline: bool,
    pub close_scope: LineScope,
    /// When true, the opening delimiter must be followed only by whitespace.
    pub open_requires_line_end: bool,
}

impl StringLiteral {
    pub fn new(delimiter: &'static str) -> Self {
        Self::with_escape(delimiter, delimiter, StringEscape::Backslash)
    }

    pub fn raw(open: &'static str, close: &'static str) -> Self {
        Self::with_escape(open, close, StringEscape::None)
    }

    pub fn doubled(open: &'static str, close: &'static str) -> Self {
        Self::with_escape(open, close, StringEscape::DoubledQuote)
    }

    pub fn with_escape(open: &'static str, close: &'static str, escape: StringEscape) -> Self {
        Self {
            open,
            close,
            escape,
            multiline: true,
            close_scope: LineScope::Anywhere,
            open_requires_line_end: false,
        }
    }

    pub fn close_at_start(mut self) -> Self {
        self.close_scope = LineScope::StartOfLine;
        self
    }

    pub fn open_at_line_end(mut self) -> Self {
        self.open_requires_line_end = true;
        self
    }

    pub fn single_line(mut self) -> Self {
        self.multiline = false;
        self
    }
}

/// A character literal (`'a'`, `'\n'`) or, for C-like languages, a multi-character literal.
#[derive(Debug, Clone, Copy)]
pub struct CharLiteral {
    pub quote: char,
    pub max_len: usize,
}

impl CharLiteral {
    pub fn new(quote: char, max_len: usize) -> Self {
        Self { quote, max_len }
    }
}

/// A comment region delimited by line-oriented markers (Ruby `=begin`/`=end`,
/// Perl POD, SAS `* ... ;`).
#[derive(Debug, Clone)]
pub struct RegionComment {
    pub start: &'static str,
    pub end: &'static str,
    pub start_scope: LineScope,
    pub end_scope: LineScope,
}

impl RegionComment {
    pub fn new(start: &'static str, end: &'static str) -> Self {
        Self {
            start,
            end,
            start_scope: LineScope::Anywhere,
            end_scope: LineScope::Anywhere,
        }
    }

    pub fn at_start(mut self) -> Self {
        self.start_scope = LineScope::StartOfLine;
        self
    }

    pub fn first_non_space(mut self) -> Self {
        self.start_scope = LineScope::FirstNonSpace;
        self
    }

    pub fn end_at_start(mut self) -> Self {
        self.end_scope = LineScope::StartOfLine;
        self
    }
}

/// A heredoc rule (`<<EOF`, `<<~SQL`, `<<<EOT`).
#[derive(Debug, Clone)]
pub struct Heredoc {
    pub marker: &'static str,
    /// Allow whitespace between the marker and the delimiter (`<< EOF`).
    pub allow_space: bool,
    /// Require an unquoted delimiter to start with an uppercase letter. This
    /// keeps `arr << x` / `$x << y` from being mistaken for a heredoc.
    pub require_uppercase: bool,
}

impl Heredoc {
    pub fn new(marker: &'static str) -> Self {
        Self {
            marker,
            allow_space: false,
            require_uppercase: false,
        }
    }

    pub fn allow_space(mut self) -> Self {
        self.allow_space = true;
        self
    }

    pub fn require_uppercase(mut self) -> Self {
        self.require_uppercase = true;
        self
    }
}

/// A language-specific raw string with a dynamic delimiter, currently C++:
/// `R"delimiter( ... )delimiter"`.
#[derive(Debug, Clone)]
pub struct RawString {
    pub prefix: &'static str,
    pub open: char,
    pub close: char,
}

impl RawString {
    pub fn new(prefix: &'static str, open: char, close: char) -> Self {
        Self {
            prefix,
            open,
            close,
        }
    }

    pub fn cpp() -> Self {
        Self::new("R\"", '(', ')')
    }
}

/// Describes the comment rules for a language.
///
/// The four legacy fields are kept for compatibility and are merged into the
/// detailed rule sets by [`crate::analyzer::CompiledSyntax`]. New definitions
/// should use the builder methods instead.
#[derive(Debug, Clone, Default)]
pub struct CommentRule {
    /// Line comment prefixes, e.g. ["//", "#"]
    pub line_comments: Vec<&'static str>,
    /// Block comment start marker, e.g. "/*"
    pub block_comment_start: Option<&'static str>,
    /// Block comment end marker, e.g. "*/"
    pub block_comment_end: Option<&'static str>,
    /// String delimiters (to avoid false positives with comment markers inside strings)
    pub string_delimiters: Vec<&'static str>,
    /// Detailed line-comment rules.
    pub line: Vec<LineComment>,
    /// Detailed block-comment rules.
    pub blocks: Vec<BlockComment>,
    /// Detailed string-literal rules.
    pub strings: Vec<StringLiteral>,
    /// Raw strings with a dynamic delimiter (C++ `R"..."`).
    pub raw_strings: Vec<RawString>,
    /// Character-literal rules.
    pub chars: Vec<CharLiteral>,
    /// Line-oriented comment regions.
    pub regions: Vec<RegionComment>,
    /// Simple heredoc markers, e.g. `<<` or `<<<` (legacy field).
    pub heredocs: Vec<&'static str>,
    /// Heredoc rules with whitespace/case options.
    pub heredoc_rules: Vec<Heredoc>,
}

impl CommentRule {
    pub fn line_rule(mut self, rule: LineComment) -> Self {
        self.line.push(rule);
        self
    }

    pub fn block_rule(mut self, rule: BlockComment) -> Self {
        self.blocks.push(rule);
        self
    }

    pub fn string_rule(mut self, rule: StringLiteral) -> Self {
        self.strings.push(rule);
        self
    }

    pub fn raw_string_rule(mut self, rule: RawString) -> Self {
        self.raw_strings.push(rule);
        self
    }

    pub fn char_rule(mut self, quote: char, max_len: usize) -> Self {
        self.chars.push(CharLiteral::new(quote, max_len));
        self
    }

    pub fn region_rule(mut self, rule: RegionComment) -> Self {
        self.regions.push(rule);
        self
    }

    pub fn heredoc(mut self, marker: &'static str) -> Self {
        self.heredocs.push(marker);
        self
    }

    pub fn heredoc_rule(mut self, rule: Heredoc) -> Self {
        self.heredoc_rules.push(rule);
        self
    }
}

/// Describes a supported language
#[derive(Debug, Clone, Default)]
pub struct Language {
    /// Display name of the language
    pub name: &'static str,
    /// Unambiguous file extensions (including dot, e.g. ".rs")
    pub extensions: Vec<&'static str>,
    /// Extensions shared with at least one other language (e.g. ".m")
    pub ambiguous_extensions: Vec<&'static str>,
    /// Exact file names (e.g. "Makefile", "Dockerfile", "CMakeLists.txt")
    pub filenames: Vec<&'static str>,
    /// Interpreter names found in `#!` lines (e.g. "python", "bash")
    pub shebangs: Vec<&'static str>,
    /// Lowercase substrings used to disambiguate shared extensions
    pub hints: Vec<&'static str>,
    /// Comment rules
    pub comment_rule: CommentRule,
}

impl Language {
    /// Returns true if the given file extension belongs to this language
    pub fn matches_extension(&self, ext: &str) -> bool {
        self.extensions
            .iter()
            .chain(self.ambiguous_extensions.iter())
            .any(|e| e.eq_ignore_ascii_case(ext))
    }

    pub fn with_ambiguous_extensions(mut self, extensions: &[&'static str]) -> Self {
        self.ambiguous_extensions.extend_from_slice(extensions);
        self
    }

    pub fn with_filenames(mut self, filenames: &[&'static str]) -> Self {
        self.filenames.extend_from_slice(filenames);
        self
    }

    pub fn with_shebangs(mut self, shebangs: &[&'static str]) -> Self {
        self.shebangs.extend_from_slice(shebangs);
        self
    }

    pub fn with_hints(mut self, hints: &[&'static str]) -> Self {
        self.hints.extend_from_slice(hints);
        self
    }
}

/// Build a comment rule from its components.
fn rule(
    line_comments: Vec<&'static str>,
    block: Option<(&'static str, &'static str)>,
    string_delimiters: Vec<&'static str>,
) -> CommentRule {
    CommentRule {
        line_comments,
        block_comment_start: block.map(|(start, _)| start),
        block_comment_end: block.map(|(_, end)| end),
        string_delimiters,
        ..Default::default()
    }
}

/// Compact constructor for language entries.
fn language(
    name: &'static str,
    extensions: Vec<&'static str>,
    comment_rule: CommentRule,
) -> Language {
    Language {
        name,
        extensions,
        comment_rule,
        ..Default::default()
    }
}

/// C-style comments (`//`, `/* */`) with `"` and `'` literals.
fn c_style() -> CommentRule {
    rule(vec!["//"], Some(("/*", "*/")), vec!["\"", "'"])
}

/// C-style comments with double-quoted strings only (no character literals).
fn c_style_double_quote() -> CommentRule {
    rule(vec!["//"], Some(("/*", "*/")), vec!["\""])
}

/// C-style comments with template literals.
fn c_style_template() -> CommentRule {
    rule(vec!["//"], Some(("/*", "*/")), vec!["\"", "'", "`"])
}

/// `#` line comments with `"` and `'` literals.
fn hash_only() -> CommentRule {
    rule(vec!["#"], None, vec!["\"", "'"])
}

/// `#` line comments with double-quoted strings only.
fn hash_double_quote() -> CommentRule {
    rule(vec!["#"], None, vec!["\""])
}

/// `//` line comments without block comments.
fn slash_no_block() -> CommentRule {
    rule(vec!["//"], None, vec!["\"", "'"])
}

/// `--` line comments with `"` and `'` literals.
fn dash_only() -> CommentRule {
    rule(vec!["--"], None, vec!["\"", "'"])
}

/// `;` line comments without block comments (Lisp-family rules).
fn semicolon_only() -> CommentRule {
    rule(vec![";"], None, vec!["\""])
}

/// `;` line comments plus one block comment pair (Lisp-family rules).
fn semicolon_block(start: &'static str, end: &'static str) -> CommentRule {
    rule(vec![";"], Some((start, end)), vec!["\""])
}

/// HTML/XML style `<!-- -->` block comments.
fn html_comment() -> CommentRule {
    rule(vec![], Some(("<!--", "-->")), vec!["\"", "'"])
}

/// Built-in language definitions. Use [`languages`] to read the cached registry.
fn language_definitions() -> Vec<Language> {
    let mut languages = vec![
        // ─── Rust ───
        Language {
            name: "Rust",
            extensions: vec![".rs"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::raw("r\"", "\""))
                .string_rule(StringLiteral::raw("r#\"", "\"#"))
                .string_rule(StringLiteral::raw("r##\"", "\"##"))
                .string_rule(StringLiteral::raw("r###\"", "\"###"))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── C / C++ ───
        Language {
            name: "C/C++",
            extensions: vec![
                ".c", ".h", ".cpp", ".hpp", ".cc", ".cxx", ".hxx", ".c++", ".h++",
            ],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .raw_string_rule(RawString::cpp())
                .char_rule('\'', 8),
            ..Default::default()
        },
        // ─── Java ───
        Language {
            name: "Java",
            extensions: vec![".java"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("\"\"\""))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── Kotlin ───
        Language {
            name: "Kotlin",
            extensions: vec![".kt", ".kts"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::raw("\"\"\"", "\"\"\""))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── Scala ───
        Language {
            name: "Scala",
            extensions: vec![".scala"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::raw("\"\"\"", "\"\"\""))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── Go ───
        Language {
            name: "Go",
            extensions: vec![".go"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::raw("`", "`"))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── Swift ───
        Language {
            name: "Swift",
            extensions: vec![".swift"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("\"\"\"")),
            ..Default::default()
        },
        // ─── Python ───
        Language {
            name: "Python",
            extensions: vec![".py", ".pyw", ".pyi"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: Some("\"\"\""),
                block_comment_end: Some("\"\"\""),
                string_delimiters: vec!["\"", "'", "\"\"\"", "'''"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── Ruby ───
        Language {
            name: "Ruby",
            extensions: vec![".rb", ".rake"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#"))
                .block_rule(
                    BlockComment::new("=begin", "=end")
                        .at_start()
                        .end_at_start(),
                )
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'"))
                .string_rule(StringLiteral::new("`"))
                .heredoc_rule(Heredoc::new("<<").require_uppercase()),
            ..Default::default()
        },
        // ─── JavaScript / TypeScript ───
        Language {
            name: "JavaScript/TypeScript",
            extensions: vec![".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'"))
                .string_rule(StringLiteral::new("`")),
            ..Default::default()
        },
        // ─── C# ───
        Language {
            name: "C#",
            extensions: vec![".cs"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::doubled("@\"", "\""))
                .string_rule(StringLiteral::doubled("$@\"", "\""))
                .string_rule(StringLiteral::doubled("@$\"", "\""))
                .string_rule(StringLiteral::raw("\"\"\"", "\"\"\""))
                .string_rule(StringLiteral::raw("$\"\"\"", "\"\"\""))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── PHP ───
        Language {
            name: "PHP",
            extensions: vec![".php"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .line_rule(LineComment::new("#"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'"))
                .heredoc_rule(Heredoc::new("<<<").allow_space()),
            ..Default::default()
        },
        // ─── Shell ───
        Language {
            name: "Shell",
            extensions: vec![".sh", ".bash", ".zsh"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#").require_whitespace_before())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::with_escape("'", "'", StringEscape::None))
                .heredoc_rule(Heredoc::new("<<").allow_space()),
            ..Default::default()
        },
        // ─── Batch ───
        language(
            "Batch",
            vec![".bat", ".cmd"],
            CommentRule::default()
                .line_rule(
                    LineComment::first_non_space("rem")
                        .case_insensitive()
                        .with_boundary_after(BoundaryAfter::Word),
                )
                .line_rule(
                    LineComment::first_non_space("@rem")
                        .case_insensitive()
                        .with_boundary_after(BoundaryAfter::Word),
                )
                .line_rule(LineComment::first_non_space("::"))
                .string_rule(StringLiteral::doubled("\"", "\"")),
        ),
        // ─── Perl ───
        Language {
            name: "Perl",
            extensions: vec![".pl", ".pm"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#"))
                .region_rule(RegionComment::new("=", "=cut").at_start().end_at_start())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'"))
                .heredoc_rule(Heredoc::new("<<").require_uppercase()),
            ..Default::default()
        },
        // ─── Lua ───
        Language {
            name: "Lua",
            extensions: vec![".lua"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("--"))
                .block_rule(BlockComment::new("--[[", "]]"))
                .block_rule(BlockComment::new("--[=[", "]=]"))
                .block_rule(BlockComment::new("--[==[", "]==]"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'"))
                .string_rule(StringLiteral::raw("[[", "]]"))
                .string_rule(StringLiteral::raw("[=[", "]=]"))
                .string_rule(StringLiteral::raw("[==[", "]==]")),
            ..Default::default()
        },
        // ─── HTML ───
        Language {
            name: "HTML",
            extensions: vec![".html", ".htm"],
            comment_rule: CommentRule {
                line_comments: vec![],
                block_comment_start: Some("<!--"),
                block_comment_end: Some("-->"),
                string_delimiters: vec!["\"", "'"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── CSS ───
        Language {
            name: "CSS",
            extensions: vec![".css", ".scss", ".less", ".sass"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── SQL ───
        Language {
            name: "SQL",
            extensions: vec![".sql"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("--"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::doubled("'", "'"))
                .string_rule(StringLiteral::doubled("\"", "\"")),
            ..Default::default()
        },
        // ─── Haskell ───
        Language {
            name: "Haskell",
            extensions: vec![".hs", ".lhs"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("--").with_boundary_after(BoundaryAfter::Symbol))
                .block_rule(BlockComment::new("{-", "-}").nested())
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── Zig ───
        language(
            "Zig",
            vec![".zig"],
            CommentRule::default()
                .line_rule(LineComment::new("//"))
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
        ),
        // ─── Vim Script ───
        Language {
            name: "Vim Script",
            extensions: vec![".vim"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("\""))
                .string_rule(StringLiteral::doubled("'", "'").single_line()),
            ..Default::default()
        },
        // ─── TOML ───
        Language {
            name: "TOML",
            extensions: vec![".toml"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'"))
                .string_rule(StringLiteral::new("\"\"\""))
                .string_rule(StringLiteral::raw("'''", "'''")),
            ..Default::default()
        },
        // ─── YAML ───
        Language {
            name: "YAML",
            extensions: vec![".yml", ".yaml"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#").require_whitespace_before())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::doubled("'", "'")),
            ..Default::default()
        },
        // ─── XML ───
        Language {
            name: "XML",
            extensions: vec![".xml", ".xsl", ".xsd", ".svg"],
            comment_rule: CommentRule {
                line_comments: vec![],
                block_comment_start: Some("<!--"),
                block_comment_end: Some("-->"),
                string_delimiters: vec!["\"", "'"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── Dart ───
        Language {
            name: "Dart",
            extensions: vec![".dart"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'"))
                .string_rule(StringLiteral::new("\"\"\""))
                .string_rule(StringLiteral::new("'''")),
            ..Default::default()
        },
        // ─── R ───
        Language {
            name: "R",
            extensions: vec![".r"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── Elixir ───
        Language {
            name: "Elixir",
            extensions: vec![".ex", ".exs"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── Erlang ───
        Language {
            name: "Erlang",
            extensions: vec![".erl", ".hrl"],
            comment_rule: CommentRule {
                line_comments: vec!["%"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── Julia ───
        Language {
            name: "Julia",
            extensions: vec![".jl"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#"))
                .block_rule(BlockComment::new("#=", "=#").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("\"\"\""))
                .char_rule('\'', 1),
            ..Default::default()
        },
        // ─── Fortran ───
        language(
            "Fortran",
            vec![".f90", ".f95", ".f03", ".f08"],
            CommentRule::default()
                .line_rule(LineComment::new("!"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::doubled("'", "'")),
        ),
        language(
            "Fortran (fixed form)",
            vec![".f", ".for", ".f77"],
            CommentRule::default()
                .line_rule(LineComment::new("!"))
                .line_rule(LineComment::at_start("c").case_insensitive())
                .line_rule(LineComment::at_start("*"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::doubled("'", "'")),
        ),
        // ─── Ada ───
        language(
            "Ada",
            vec![".adb", ".ads"],
            CommentRule::default()
                .line_rule(LineComment::new("--"))
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
        ),
        // ─── Vue ───
        Language {
            name: "Vue",
            extensions: vec![".vue"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'", "`"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── Markdown ───
        Language {
            name: "Markdown",
            extensions: vec![".md", ".markdown", ".mdown", ".mkd", ".mkdn"],
            comment_rule: CommentRule {
                line_comments: vec![],
                block_comment_start: Some("<!--"),
                block_comment_end: Some("-->"),
                string_delimiters: vec!["\"", "'", "`"],
                ..Default::default()
            },
            ..Default::default()
        },
        // ─── Additional C-family languages ───
        language("CUDA", vec![".cu", ".cuh"], c_style()),
        language("OpenCL", vec![".cl"], c_style()),
        language(
            "GLSL",
            vec![
                ".glsl", ".vert", ".frag", ".geom", ".tesc", ".tese", ".comp",
            ],
            c_style_double_quote(),
        ),
        language("HLSL", vec![".hlsl", ".hlsli"], c_style_double_quote()),
        language("Metal", vec![".metal"], c_style_double_quote()),
        language(
            "Verilog/SystemVerilog",
            vec![".v", ".vh", ".sv", ".svh"],
            c_style_double_quote(),
        ),
        language(
            "VHDL",
            vec![".vhd", ".vhdl"],
            rule(vec!["--"], Some(("/*", "*/")), vec!["\""]),
        ),
        language(
            "Groovy",
            vec![".groovy", ".gvy", ".gy", ".gsh", ".gradle"],
            rule(
                vec!["//"],
                Some(("/*", "*/")),
                vec!["\"\"\"", "'''", "\"", "'"],
            ),
        ),
        language("Solidity", vec![".sol"], c_style()),
        language("Move", vec![".move"], c_style_double_quote()),
        language("Cairo", vec![".cairo"], c_style()),
        language(
            "D",
            vec![".d"],
            CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/").nested())
                .block_rule(BlockComment::new("/+", "+/").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::raw("`", "`"))
                .string_rule(StringLiteral::raw("r\"", "\""))
                .char_rule('\'', 1),
        ),
        language("Pony", vec![".pony"], c_style()),
        language("Odin", vec![".odin"], c_style()),
        language("Chapel", vec![".chpl"], c_style()),
        language(
            "ReasonML",
            vec![".re", ".rei"],
            rule(vec!["//"], Some(("/*", "*/")), vec!["\""]),
        ),
        language(
            "ReScript",
            vec![".res", ".resi"],
            rule(vec!["//"], Some(("/*", "*/")), vec!["\""]),
        ),
        language("Haxe", vec![".hx"], c_style()),
        language("ActionScript", vec![".as"], c_style()),
        language("Vala", vec![".vala", ".vapi"], c_style()),
        language("QML", vec![".qml"], c_style_template()),
        language("Apex", vec![".cls", ".trigger"], c_style()),
        language("YANG", vec![".yang"], c_style()),
        language("Protocol Buffers", vec![".proto"], c_style()),
        language(
            "Thrift",
            vec![".thrift"],
            rule(vec!["//", "#"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language("FlatBuffers", vec![".fbs"], c_style()),
        language("Cap'n Proto", vec![".capnp"], hash_double_quote()),
        // ─── Hash-comment languages ───
        language("Vyper", vec![".vy"], hash_only()),
        language(
            "GraphQL",
            vec![".graphql", ".gql"],
            rule(vec!["#"], None, vec!["\"\"\"", "\""]),
        ),
        language("Crystal", vec![".cr"], hash_only()),
        language("Starlark", vec![".bzl"], hash_only()),
        language(
            "Rego",
            vec![".rego"],
            rule(vec!["#"], None, vec!["\"", "`"]),
        ),
        language("Roc", vec![".roc"], hash_double_quote()),
        language("Fish", vec![".fish"], hash_only()),
        language(
            "Nushell",
            vec![".nu"],
            rule(vec!["#"], None, vec!["\"", "'", "`"]),
        ),
        language("Tcl", vec![".tcl", ".tk"], hash_double_quote()),
        language("AWK", vec![".awk"], hash_double_quote()),
        language("SPARQL", vec![".rq", ".sparql"], hash_only()),
        language("Turtle", vec![".ttl"], hash_only()),
        language("QMake", vec![".pro", ".pri"], hash_only()),
        language(
            "Puppet",
            vec![".pp"],
            rule(vec!["#"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language(
            "JSONC",
            vec![".jsonc"],
            rule(vec!["//"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language(
            "JSON5",
            vec![".json5"],
            rule(vec!["//"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language(
            "Hjson",
            vec![".hjson"],
            rule(vec!["#", "//"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language(
            "Jsonnet",
            vec![".jsonnet", ".libsonnet"],
            rule(vec!["//", "#"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language(
            "Nix",
            vec![".nix"],
            rule(vec!["#"], Some(("/*", "*/")), vec!["\"", "''"]),
        ),
        language(
            "HCL/Terraform",
            vec![".tf", ".tfvars", ".hcl"],
            rule(vec!["#", "//"], Some(("/*", "*/")), vec!["\""]),
        ),
        language(
            "CMake",
            vec![".cmake"],
            rule(vec!["#"], Some(("#[[", "]]")), vec!["\""]),
        ),
        language(
            "PowerShell",
            vec![".ps1", ".psm1", ".psd1"],
            CommentRule::default()
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
                ),
        ),
        language(
            "Properties",
            vec![".properties"],
            rule(vec!["#", "!"], None, vec![]),
        ),
        language(
            "INI",
            vec![".ini", ".cfg"],
            rule(vec![";", "#"], None, vec!["\""]),
        ),
        language(
            "Nim",
            vec![".nim", ".nims"],
            CommentRule::default()
                .line_rule(LineComment::new("#"))
                .block_rule(BlockComment::new("#[", "]#").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("\"\"\""))
                .string_rule(StringLiteral::raw("r\"", "\""))
                .char_rule('\'', 1),
        ),
        language(
            "Raku",
            vec![".raku", ".rakumod", ".p6", ".pm6"],
            rule(vec!["#"], Some(("=begin", "=end")), vec!["\"", "'"]),
        ),
        language(
            "Mojo",
            vec![".mojo"],
            rule(
                vec!["#"],
                Some(("\"\"\"", "\"\"\"")),
                vec!["\"\"\"", "'''", "\"", "'"],
            ),
        ),
        // ─── Dash, semicolon, and percent comment languages ───
        language(
            "TeX/LaTeX",
            vec![".tex", ".sty", ".ltx"],
            rule(vec!["%"], None, vec!["\""]),
        ),
        language("Eiffel", vec![".e"], dash_only()),
        language(
            "Mathematica",
            vec![".wl", ".wls"],
            rule(vec![], Some(("(*", "*)")), vec!["\""]),
        ),
        language(
            "XQuery",
            vec![".xq", ".xquery"],
            rule(vec![], Some(("(:", ":)")), vec!["\"", "'"]),
        ),
        language(
            "Smalltalk",
            vec![".st"],
            rule(vec![], Some(("\"", "\"")), vec!["'"]),
        ),
        language(
            "Scheme/Racket",
            vec![".scm", ".ss", ".rkt"],
            semicolon_block("#|", "|#"),
        ),
        language(
            "Common Lisp",
            vec![".lisp", ".lsp"],
            semicolon_block("#|", "|#"),
        ),
        language("Emacs Lisp", vec![".el"], semicolon_only()),
        language(
            "Clojure",
            vec![".clj", ".cljs", ".cljc", ".edn"],
            semicolon_only(),
        ),
        language(
            "OCaml/Standard ML",
            vec![".ml", ".mli", ".sml", ".sig"],
            CommentRule::default()
                .block_rule(BlockComment::new("(*", "*)").nested())
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
        ),
        language(
            "F#",
            vec![".fs", ".fsi", ".fsx"],
            CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("(*", "*)").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("\"\"\""))
                .char_rule('\'', 1),
        ),
        language(
            "Elm",
            vec![".elm"],
            CommentRule::default()
                .line_rule(LineComment::new("--"))
                .block_rule(BlockComment::new("{-", "-}").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("\"\"\""))
                .char_rule('\'', 1),
        ),
        language(
            "PureScript",
            vec![".purs"],
            CommentRule::default()
                .line_rule(LineComment::new("--").with_boundary_after(BoundaryAfter::Symbol))
                .block_rule(BlockComment::new("{-", "-}").nested())
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
        ),
        language(
            "Idris",
            vec![".idr"],
            CommentRule::default()
                .line_rule(LineComment::new("--").with_boundary_after(BoundaryAfter::Symbol))
                .block_rule(BlockComment::new("{-", "-}").nested())
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
        ),
        language(
            "Agda",
            vec![".agda"],
            CommentRule::default()
                .line_rule(LineComment::new("--").with_boundary_after(BoundaryAfter::Symbol))
                .block_rule(BlockComment::new("{-", "-}").nested())
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
        ),
        language(
            "Dhall",
            vec![".dhall"],
            CommentRule::default()
                .line_rule(LineComment::new("--").with_boundary_after(BoundaryAfter::Symbol))
                .block_rule(BlockComment::new("{-", "-}").nested())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::raw("''", "''")),
        ),
        language(
            "Prolog",
            vec![".prolog"],
            rule(vec!["%"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language(
            "Visual Basic",
            vec![".vb", ".vbs", ".bas"],
            CommentRule::default()
                .line_rule(LineComment::new("'"))
                .line_rule(
                    LineComment::first_non_space("rem")
                        .case_insensitive()
                        .with_boundary_after(BoundaryAfter::Word),
                )
                .string_rule(StringLiteral::doubled("\"", "\"")),
        ),
        language(
            "Pascal",
            vec![".pas", ".dpr", ".lpr"],
            CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("{", "}"))
                .block_rule(BlockComment::new("(*", "*)"))
                .string_rule(StringLiteral::doubled("'", "'")),
        ),
        language(
            "Assembly",
            vec![".asm", ".nasm"],
            rule(vec![";"], None, vec!["\"", "'"]),
        ),
        language(
            "GNU Assembly",
            vec![".s"],
            rule(vec!["#"], Some(("/*", "*/")), vec!["\""]),
        ),
        language("LLVM IR", vec![".ll"], rule(vec![";"], None, vec!["\""])),
        language(
            "WebAssembly Text",
            vec![".wat", ".wast"],
            rule(vec![";;"], Some(("(;", ";)")), vec!["\""]),
        ),
        // ─── Indented and template languages ───
        language(
            "CoffeeScript",
            vec![".coffee"],
            rule(vec!["#"], Some(("###", "###")), vec!["\"", "'"]),
        ),
        language(
            "LiveScript",
            vec![".ls"],
            rule(vec!["#"], Some(("/*", "*/")), vec!["\"", "'"]),
        ),
        language("Pug", vec![".pug", ".jade"], slash_no_block()),
        language("Stylus", vec![".styl"], c_style()),
        language("XAML", vec![".xaml"], html_comment()),
        language(
            "Razor",
            vec![".cshtml", ".razor"],
            rule(vec![], Some(("@*", "*@")), vec!["\"", "'"]),
        ),
        language(
            "Jinja",
            vec![".j2", ".jinja", ".jinja2"],
            rule(vec![], Some(("{#", "#}")), vec!["\"", "'"]),
        ),
        language(
            "Handlebars",
            vec![".hbs", ".handlebars"],
            rule(vec![], Some(("{{!--", "--}}")), vec!["\"", "'"]),
        ),
        language(
            "Svelte",
            vec![".svelte"],
            rule(vec!["//"], Some(("<!--", "-->")), vec!["\"", "'", "`"]),
        ),
        language(
            "Astro",
            vec![".astro"],
            rule(vec!["//"], Some(("<!--", "-->")), vec!["\"", "'", "`"]),
        ),
        // ─── File-name driven languages ───
        language(
            "Makefile",
            vec![".mk", ".mak"],
            CommentRule::default()
                .line_rule(LineComment::new("#"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::new("'")),
        ),
        language(
            "Dockerfile",
            vec![".dockerfile"],
            CommentRule::default()
                .line_rule(LineComment::first_non_space("#"))
                .string_rule(StringLiteral::new("\"")),
        ),
        language(
            "Meson",
            vec![],
            CommentRule::default()
                .line_rule(LineComment::new("#"))
                .string_rule(StringLiteral::new("'"))
                .string_rule(StringLiteral::new("\"")),
        ),
        // ─── Ambiguous-extension languages ───
        language(
            "Objective-C",
            vec![".mm"],
            CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::doubled("@\"", "\""))
                .char_rule('\'', 1),
        ),
        language(
            "MATLAB",
            vec![],
            CommentRule::default()
                .line_rule(LineComment::new("%"))
                .block_rule(
                    BlockComment::new("%{", "%}")
                        .alone_on_line()
                        .end_alone_on_line(),
                )
                .string_rule(StringLiteral::doubled("\"", "\""))
                .char_rule('\'', 256),
        ),
        language(
            "V",
            vec![],
            CommentRule::default()
                .line_rule(LineComment::new("//"))
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::new("\""))
                .char_rule('\'', 1),
        ),
        language(
            "Coq",
            vec![],
            CommentRule::default()
                .block_rule(BlockComment::new("(*", "*)").nested())
                .string_rule(StringLiteral::new("\"")),
        ),
        language(
            "Forth",
            vec![".fth", ".forth"],
            CommentRule::default()
                .line_rule(LineComment::new("\\").require_whitespace_before())
                .block_rule(BlockComment::new("(", ")")),
        ),
        // ─── Fixed-column and statement-comment languages ───
        language(
            "COBOL",
            vec![".cob", ".cbl", ".cpy"],
            CommentRule::default()
                .line_rule(LineComment::new("*").with_scope(LineScope::Column(6)))
                .line_rule(LineComment::new("/").with_scope(LineScope::Column(6)))
                .line_rule(LineComment::new("*>"))
                .string_rule(StringLiteral::doubled("\"", "\""))
                .string_rule(StringLiteral::doubled("'", "'")),
        ),
        language(
            "SAS",
            vec![".sas"],
            CommentRule::default()
                .region_rule(RegionComment::new("*", ";").first_non_space())
                .block_rule(BlockComment::new("/*", "*/"))
                .string_rule(StringLiteral::doubled("\"", "\""))
                .string_rule(StringLiteral::doubled("'", "'")),
        ),
    ];
    apply_detection_metadata(&mut languages);
    languages
}

/// Attach file-name, shebang, ambiguity and hint metadata to the definitions.
fn apply_detection_metadata(languages: &mut [Language]) {
    for language in languages {
        match language.name {
            "Python" => {
                language.shebangs = vec!["python", "pypy", "xonsh", "uv"];
                language.filenames = vec!["SConstruct", "SConscript"];
            }
            "Shell" => {
                language.shebangs = vec!["sh", "bash", "zsh", "dash", "ksh"];
                language.filenames = vec![
                    ".bashrc",
                    ".bash_profile",
                    ".bash_login",
                    ".profile",
                    ".zshrc",
                    ".zprofile",
                    ".zshenv",
                ];
            }
            "Fish" => language.shebangs = vec!["fish"],
            "Nushell" => language.shebangs = vec!["nu"],
            "JavaScript/TypeScript" => {
                language.shebangs = vec!["node", "deno", "bun", "tsx", "ts-node"];
            }
            "Ruby" => {
                language.shebangs = vec!["ruby"];
                language.filenames = vec![
                    "Rakefile",
                    "Gemfile",
                    "Guardfile",
                    "Vagrantfile",
                    "Podfile",
                    "Brewfile",
                    "Fastfile",
                    "Appfile",
                    "Dangerfile",
                    "config.ru",
                ];
            }
            "Perl" => {
                language.shebangs = vec!["perl"];
                language.hints = vec!["use strict", "use warnings", "my $", "sub ", "package "];
            }
            "Raku" => language.shebangs = vec!["raku", "perl6"],
            "PHP" => language.shebangs = vec!["php"],
            "PowerShell" => language.shebangs = vec!["pwsh", "powershell"],
            "Lua" => language.shebangs = vec!["lua", "luajit"],
            "R" => language.shebangs = vec!["rscript"],
            "Julia" => language.shebangs = vec!["julia"],
            "Elixir" => language.shebangs = vec!["elixir"],
            "Erlang" => language.shebangs = vec!["escript"],
            "AWK" => language.shebangs = vec!["awk", "gawk", "mawk", "nawk"],
            "Tcl" => language.shebangs = vec!["tclsh", "wish"],
            "Scheme/Racket" => {
                language.shebangs = vec!["racket", "guile", "scheme", "gosh"];
            }
            "Common Lisp" => {
                language.shebangs = vec!["sbcl", "clisp", "ecl", "ccl"];
                language.ambiguous_extensions = vec![".cl"];
                language.hints = vec![
                    "(defun",
                    "(defmacro",
                    "(defvar",
                    "(in-package",
                    "(format",
                    "(let",
                ];
            }
            "Clojure" => language.shebangs = vec!["bb"],
            "Crystal" => language.shebangs = vec!["crystal"],
            "Dart" => language.shebangs = vec!["dart"],
            "Groovy" => {
                language.shebangs = vec!["groovy"];
                language.filenames = vec!["Jenkinsfile"];
            }
            "Kotlin" => language.shebangs = vec!["kotlin", "kotlinc"],
            "Scala" => language.shebangs = vec!["scala"],
            "Swift" => language.shebangs = vec!["swift"],
            "Haskell" => language.shebangs = vec!["runhaskell", "runghc"],
            "OCaml/Standard ML" => language.shebangs = vec!["ocaml"],
            "Starlark" => {
                language.filenames = vec!["BUILD", "BUILD.bazel", "WORKSPACE", "WORKSPACE.bazel"];
            }
            "CMake" => language.filenames = vec!["CMakeLists.txt"],
            "TOML" => language.filenames = vec!["Pipfile"],
            "INI" => language.filenames = vec![".editorconfig"],
            "Makefile" => {
                language.filenames = vec!["Makefile", "GNUmakefile"];
            }
            "Dockerfile" => language.filenames = vec!["Dockerfile", "Containerfile"],
            "Meson" => language.filenames = vec!["meson.build", "meson_options.txt"],
            "Verilog/SystemVerilog" => {
                language.hints = vec![
                    "module ",
                    "endmodule",
                    "always @",
                    "assign ",
                    "wire ",
                    "reg ",
                    "input ",
                    "output ",
                ];
            }
            "Objective-C" => {
                language.ambiguous_extensions = vec![".m"];
                language.hints = vec![
                    "@interface",
                    "@implementation",
                    "@end",
                    "#import",
                    "@protocol",
                    "@property",
                    "@synthesize",
                ];
            }
            "MATLAB" => {
                language.ambiguous_extensions = vec![".m"];
                language.shebangs = vec!["matlab", "octave"];
                language.hints = vec![
                    "classdef",
                    "function ",
                    "properties",
                    "methods",
                    "%{",
                    "disp(",
                    "plot(",
                    "zeros(",
                ];
            }
            "Mathematica" => {
                language.ambiguous_extensions = vec![".m"];
                language.hints = vec![
                    "(*",
                    "module[",
                    "clear[",
                    ":=",
                    "plot[",
                    "table[",
                    "simplify[",
                ];
            }
            "V" => {
                language.ambiguous_extensions = vec![".v"];
                language.hints = vec!["fn ", "pub fn", "struct ", "import ", "mut ", "println("];
            }
            "Coq" => {
                language.ambiguous_extensions = vec![".v"];
                language.hints = vec![
                    "theorem ",
                    "lemma ",
                    "proof.",
                    "qed.",
                    "require import",
                    "definition ",
                    "inductive ",
                ];
            }
            "F#" => {
                language.hints = vec!["let ", "module ", "open ", "type ", "namespace ", "[<"];
            }
            "Forth" => {
                language.ambiguous_extensions = vec![".fs"];
                language.hints = vec![": ", "variable ", "constant ", "create ", "\\ "];
            }
            "OpenCL" => {
                language.hints = vec![
                    "__kernel",
                    "__global",
                    "__local",
                    "kernel void",
                    "cl_kernel",
                    "#pragma opencl",
                ];
            }
            "Apex" => {
                language.hints = vec![
                    "@istest",
                    "global class",
                    "public class",
                    "private class",
                    "trigger ",
                    "system.debug",
                ];
            }
            "TeX/LaTeX" => {
                language.ambiguous_extensions = vec![".cls"];
                language.hints = vec![
                    "\\documentclass",
                    "\\usepackage",
                    "\\providesclass",
                    "\\loadclass",
                    "\\needstexformat",
                    "\\providecommand",
                ];
            }
            "Puppet" => {
                language.hints = vec![
                    "class ",
                    "define ",
                    "node ",
                    "include ",
                    "=>",
                    "$facts",
                    "package {",
                ];
            }
            "Pascal" => {
                language.ambiguous_extensions = vec![".pp"];
                language.hints = vec![
                    "program ",
                    "unit ",
                    "interface",
                    "implementation",
                    "begin",
                    "end.",
                ];
            }
            "QMake" => {
                language.hints = vec![
                    "template",
                    "target",
                    "sources",
                    "headers",
                    "qt +=",
                    "config +=",
                    "greaterthan(",
                ];
            }
            "Prolog" => {
                language.ambiguous_extensions = vec![".pl", ".pro"];
                language.hints = vec![":-", "?-", "module(", "use_module", ":- module"];
            }
            _ => {}
        }
    }
}

/// Preference used when content hints cannot disambiguate an extension.
const EXTENSION_PREFERENCE: &[(&str, &str)] = &[
    (".m", "Objective-C"),
    (".v", "Verilog/SystemVerilog"),
    (".pl", "Perl"),
    (".fs", "F#"),
    (".cl", "OpenCL"),
    (".cls", "Apex"),
    (".pp", "Puppet"),
    (".pro", "QMake"),
];

/// File-name prefixes recognized in addition to exact file names.
const FILENAME_PREFIXES: &[(&str, &str)] = &[
    ("dockerfile.", "Dockerfile"),
    ("containerfile.", "Dockerfile"),
    ("makefile.", "Makefile"),
];

#[derive(Debug)]
pub struct Registry {
    languages: Vec<Language>,
    by_name: HashMap<String, usize>,
    by_extension: HashMap<String, Vec<usize>>,
    by_filename: HashMap<String, usize>,
    by_shebang: HashMap<&'static str, usize>,
}

impl Registry {
    fn build() -> Self {
        let languages = language_definitions();
        let mut by_name = HashMap::new();
        let mut by_extension: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_filename = HashMap::new();
        let mut by_shebang = HashMap::new();

        for (index, language) in languages.iter().enumerate() {
            by_name.insert(language.name.to_ascii_lowercase(), index);
            for extension in language
                .extensions
                .iter()
                .chain(language.ambiguous_extensions.iter())
            {
                by_extension
                    .entry(extension.to_ascii_lowercase())
                    .or_default()
                    .push(index);
            }
            for filename in &language.filenames {
                by_filename.insert(filename.to_ascii_lowercase(), index);
            }
            for shebang in &language.shebangs {
                by_shebang.insert(*shebang, index);
            }
        }

        Self {
            languages,
            by_name,
            by_extension,
            by_filename,
            by_shebang,
        }
    }

    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    pub fn language(&self, index: usize) -> &Language {
        &self.languages[index]
    }

    pub fn language_by_name(&self, name: &str) -> Option<&Language> {
        self.by_name
            .get(&name.to_ascii_lowercase())
            .map(|&index| self.language(index))
    }

    /// Indices into [`Registry::languages`] for the given extension.
    pub fn extension_indices(&self, extension: &str) -> &[usize] {
        self.by_extension
            .get(&normalize_extension(extension))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn language_by_filename(&self, filename: &str) -> Option<&Language> {
        let lower = filename.to_ascii_lowercase();
        if let Some(&index) = self.by_filename.get(&lower) {
            return Some(self.language(index));
        }
        for (prefix, name) in FILENAME_PREFIXES {
            if !lower.starts_with(prefix) {
                continue;
            }
            if let Some(language) = self.language_by_name(name) {
                return Some(language);
            }
        }
        None
    }

    pub fn language_by_shebang(&self, name: &str) -> Option<&Language> {
        self.by_shebang.get(name).map(|&index| self.language(index))
    }
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Cached language registry, built once per process.
pub fn registry() -> &'static Registry {
    REGISTRY.get_or_init(Registry::build)
}

/// Cached language list. Prefer this over [`supported_languages`] in
/// long-running front-ends.
pub fn languages() -> &'static [Language] {
    registry().languages()
}

/// Returns a snapshot of all supported languages.
pub fn supported_languages() -> Vec<Language> {
    languages().to_vec()
}

/// Find a language by display name (case-insensitive).
pub fn find_language_by_name(name: &str) -> Option<&'static Language> {
    registry().language_by_name(name)
}

/// All languages that can claim the given extension.
pub fn candidates_for_extension(extension: &str) -> Vec<&'static Language> {
    let registry = registry();
    registry
        .extension_indices(extension)
        .iter()
        .map(|&index| registry.language(index))
        .collect()
}

/// Find the preferred language for an extension.
pub fn find_language_by_ext(ext: &str) -> Option<Language> {
    let candidates = candidates_for_extension(ext);
    preferred_candidate(ext, &candidates).cloned()
}

/// Find a language by exact file name or recognized file-name prefix.
pub fn language_for_filename(filename: &str) -> Option<&'static Language> {
    registry().language_by_filename(filename)
}

/// Find a language from a `#!` line.
pub fn language_for_shebang(line: &str) -> Option<&'static Language> {
    let registry = registry();
    let mut tokens = line.trim_start().strip_prefix("#!")?.split_whitespace();
    let mut program = tokens.next()?;
    let mut name = executable_name(program);

    if name == "env" {
        loop {
            let token = tokens.next()?;
            if token == "-S" || token == "--split-string" {
                continue;
            }
            if matches!(token, "-u" | "--unset" | "-C" | "--chdir") {
                tokens.next();
                continue;
            }
            if token.starts_with('-') {
                continue;
            }
            if token.contains('=') && !token.starts_with('/') && !token.starts_with('\\') {
                continue;
            }
            program = token;
            break;
        }
        name = executable_name(program);
    }

    if let Some(language) = registry.language_by_shebang(name.as_str()) {
        return Some(language);
    }
    let versionless = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    if versionless == name {
        return None;
    }
    registry.language_by_shebang(versionless)
}

/// Resolve a shared extension using content hints, then the preference table.
///
/// The boolean is `true` when a content hint decided the language.
pub fn resolve_ambiguous_extension(
    extension: &str,
    candidates: &[&'static Language],
    content: &str,
) -> Option<(&'static Language, bool)> {
    if candidates.is_empty() {
        return None;
    }
    let preferred = preferred_candidate(extension, candidates);
    let lower = content.to_ascii_lowercase();
    let mut best: Option<(usize, &'static Language)> = None;

    for candidate in candidates {
        let score = candidate
            .hints
            .iter()
            .filter(|hint| lower.contains(*hint))
            .count();
        if score == 0 {
            continue;
        }
        let replace = match best {
            None => true,
            Some((best_score, best_language)) => {
                score > best_score
                    || (score == best_score
                        && preferred.is_some_and(|preferred| {
                            preferred.name == candidate.name && preferred.name != best_language.name
                        }))
            }
        };
        if replace {
            best = Some((score, candidate));
        }
    }

    if let Some((_, language)) = best {
        return Some((language, true));
    }
    preferred.map(|language| (language, false))
}

/// Build extension → language name mapping table for unambiguous extensions.
pub fn extension_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    for lang in languages() {
        for ext in &lang.extensions {
            map.insert(*ext, lang.name);
        }
    }
    map
}

/// Get the list of all supported language names
pub fn list_language_names() -> Vec<&'static str> {
    languages().iter().map(|l| l.name).collect()
}

fn normalize_extension(extension: &str) -> String {
    let lower = extension.to_ascii_lowercase();
    if lower.starts_with('.') {
        lower
    } else {
        format!(".{lower}")
    }
}

fn preferred_candidate<'a>(extension: &str, candidates: &[&'a Language]) -> Option<&'a Language> {
    let preferred = EXTENSION_PREFERENCE
        .iter()
        .find(|(ext, _)| ext.eq_ignore_ascii_case(extension))
        .map(|(_, name)| *name);
    preferred
        .and_then(|name| {
            candidates
                .iter()
                .copied()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(name))
        })
        .or_else(|| candidates.first().copied())
}

fn executable_name(program: &str) -> String {
    let base = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let lower = base.to_ascii_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::analyzer::{LineStats, analyze_content};

    #[test]
    fn language_names_are_unique() {
        let mut seen = HashSet::new();
        for language in supported_languages() {
            assert!(
                seen.insert(language.name),
                "duplicate language name: {}",
                language.name
            );
        }
    }

    #[test]
    fn extensions_are_unique_case_insensitively() {
        let mut seen: HashMap<String, &'static str> = HashMap::new();
        for language in supported_languages() {
            for extension in &language.extensions {
                let key = extension.to_ascii_lowercase();
                if let Some(previous) = seen.insert(key.clone(), language.name) {
                    panic!(
                        "extension {key} is claimed by both {previous} and {}",
                        language.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_language_has_consistent_comment_markers() {
        for language in supported_languages() {
            let rule = &language.comment_rule;
            assert_eq!(
                rule.block_comment_start.is_some(),
                rule.block_comment_end.is_some(),
                "{} has an incomplete block comment rule",
                language.name
            );
            assert!(
                !rule.line_comments.is_empty()
                    || rule.block_comment_start.is_some()
                    || !rule.line.is_empty()
                    || !rule.blocks.is_empty()
                    || !rule.regions.is_empty(),
                "{} has no comment syntax",
                language.name
            );
        }
    }

    #[test]
    fn resolves_new_languages_by_extension() {
        for (extension, expected) in [
            (".cu", "CUDA"),
            (".tf", "HCL/Terraform"),
            (".nim", "Nim"),
            (".ps1", "PowerShell"),
            (".coffee", "CoffeeScript"),
            (".wat", "WebAssembly Text"),
            (".svelte", "Svelte"),
            (".bat", "Batch"),
            (".f", "Fortran (fixed form)"),
            (".cob", "COBOL"),
            (".sas", "SAS"),
        ] {
            let language = find_language_by_ext(extension)
                .unwrap_or_else(|| panic!("no language for {extension}"));
            assert_eq!(language.name, expected, "wrong language for {extension}");
        }
    }

    fn stats_for(extension: &str, content: &str) -> LineStats {
        let language = find_language_by_ext(extension).expect("language should be recognized");
        analyze_content(content, &language.comment_rule)
    }

    #[test]
    fn classifies_comment_styles_introduced_in_phase_one() {
        let powershell = stats_for(".ps1", "Write-Host 'hi'\n<# block\n#>");
        assert_eq!(powershell.code_lines, 1);
        assert_eq!(powershell.comment_lines, 2);

        let cmake = stats_for(".cmake", "add_executable(app main.cpp)\n#[[ block\n]]");
        assert_eq!(cmake.code_lines, 1);
        assert_eq!(cmake.comment_lines, 2);

        let coffee = stats_for(".coffee", "### block ###\nsquare = (x) -> x * x # inline");
        assert_eq!(coffee.comment_lines, 1);
        assert_eq!(coffee.mixed_lines, 1);

        let smalltalk = stats_for(".st", "\"a comment\"\nTranscript show: 'hello'");
        assert_eq!(smalltalk.comment_lines, 1);
        assert_eq!(smalltalk.code_lines, 1);

        let visual_basic = stats_for(".vb", "' a comment\nDim x = \"'\"");
        assert_eq!(visual_basic.comment_lines, 1);
        assert_eq!(visual_basic.code_lines, 1);

        let wat = stats_for(".wat", ";; comment\n(; block ;)\n(module)");
        assert_eq!(wat.comment_lines, 2);
        assert_eq!(wat.code_lines, 1);
    }

    #[test]
    fn phase_two_rules_are_wired_into_the_registry() {
        let rust = stats_for(
            ".rs",
            "let x: &'a str = \"v\"; // real\n/* /* nested */ still */\n",
        );
        assert_eq!(rust.mixed_lines, 1);
        assert_eq!(rust.comment_lines, 1);

        let shell = stats_for(
            ".sh",
            "echo ${#x}\necho hi # real\ncat <<EOF\n# body\nEOF\n",
        );
        assert_eq!(shell.code_lines, 4);
        assert_eq!(shell.mixed_lines, 1);
        assert_eq!(shell.comment_lines, 0);

        let batch = stats_for(".bat", "REM comment\nremove x\n:: comment\n");
        assert_eq!(batch.comment_lines, 2);
        assert_eq!(batch.code_lines, 1);

        let fortran = stats_for(".f", "C fixed comment\n      CALL FOO\n");
        assert_eq!(fortran.comment_lines, 1);
        assert_eq!(fortran.code_lines, 1);

        let vim = stats_for(".vim", "\" comment\nlet x = 'a'\n");
        assert_eq!(vim.comment_lines, 1);
        assert_eq!(vim.code_lines, 1);

        let powershell = stats_for(".ps1", "$x = @\"\n# body\n\"@\n# comment\n");
        assert_eq!(powershell.code_lines, 3);
        assert_eq!(powershell.comment_lines, 1);

        let perl = stats_for(".pl", "=pod\n# pod\n=cut\nprint 1; # real\n");
        assert_eq!(perl.comment_lines, 3);
        assert_eq!(perl.mixed_lines, 1);

        let cpp = stats_for(".cpp", "auto s = R\"x(// not a comment)x\"; // real\n");
        assert_eq!(cpp.mixed_lines, 1);

        let ruby = stats_for(".rb", "arr << x\nsql = <<~SQL\n# body\nSQL\n# real\n");
        assert_eq!(ruby.code_lines, 4);
        assert_eq!(ruby.comment_lines, 1);

        let perl_heredoc = stats_for(".pl", "$x << 1;\nmy $s = <<EOF;\n# body\nEOF\n# real\n");
        assert_eq!(perl_heredoc.code_lines, 4);
        assert_eq!(perl_heredoc.comment_lines, 1);

        let cobol = stats_for(
            ".cob",
            "      * comment\n           DISPLAY 'HI'. *> inline\n",
        );
        assert_eq!(cobol.comment_lines, 1);
        assert_eq!(cobol.mixed_lines, 1);

        let sas = stats_for(".sas", "* comment\nstill comment;\ndata x;\n");
        assert_eq!(sas.comment_lines, 2);
        assert_eq!(sas.code_lines, 1);
    }

    #[test]
    fn phase_three_metadata_is_unique_and_reachable() {
        let mut filenames = HashMap::new();
        let mut shebangs = HashMap::new();

        for language in languages() {
            assert!(
                !language.extensions.is_empty()
                    || !language.ambiguous_extensions.is_empty()
                    || !language.filenames.is_empty(),
                "{} cannot be detected",
                language.name
            );
            let mut own_extensions = HashSet::new();
            for extension in language
                .extensions
                .iter()
                .chain(language.ambiguous_extensions.iter())
            {
                assert!(
                    own_extensions.insert(extension.to_ascii_lowercase()),
                    "{} lists {extension} twice",
                    language.name
                );
            }
            for filename in &language.filenames {
                let key = filename.to_ascii_lowercase();
                assert!(
                    filenames.insert(key.clone(), language.name).is_none(),
                    "duplicate file name {key}"
                );
            }
            for shebang in &language.shebangs {
                assert!(
                    shebangs.insert(*shebang, language.name).is_none(),
                    "duplicate shebang {shebang}"
                );
            }
        }
    }

    #[test]
    fn detects_filenames_and_shebangs() {
        for (filename, expected) in [
            ("Makefile", "Makefile"),
            ("makefile", "Makefile"),
            ("Makefile.am", "Makefile"),
            ("Dockerfile", "Dockerfile"),
            ("Dockerfile.dev", "Dockerfile"),
            ("Containerfile", "Dockerfile"),
            ("CMakeLists.txt", "CMake"),
            ("BUILD.bazel", "Starlark"),
            ("WORKSPACE", "Starlark"),
            ("Rakefile", "Ruby"),
            ("Gemfile", "Ruby"),
            ("Jenkinsfile", "Groovy"),
            ("Pipfile", "TOML"),
            (".editorconfig", "INI"),
            ("meson.build", "Meson"),
        ] {
            let language = language_for_filename(filename)
                .unwrap_or_else(|| panic!("no language for {filename}"));
            assert_eq!(language.name, expected, "wrong language for {filename}");
        }
        assert!(language_for_filename("unknown.txt").is_none());

        for (line, expected) in [
            ("#!/usr/bin/env python3", "Python"),
            ("#!/usr/bin/python3.11", "Python"),
            ("#!/usr/bin/env -u NAME python3", "Python"),
            ("#!/bin/bash", "Shell"),
            (
                "#!/usr/bin/env -S node --experimental",
                "JavaScript/TypeScript",
            ),
            ("#!/usr/bin/env perl6", "Raku"),
            ("#!/usr/bin/env sbcl", "Common Lisp"),
            ("#!/usr/bin/env octave", "MATLAB"),
        ] {
            let language =
                language_for_shebang(line).unwrap_or_else(|| panic!("no language for {line}"));
            assert_eq!(language.name, expected, "wrong language for {line}");
        }
        assert!(language_for_shebang("not a shebang").is_none());
        assert!(language_for_shebang("#!/usr/bin/env").is_none());
    }

    #[test]
    fn resolves_ambiguous_extensions_by_hint_or_preference() {
        for (extension, content, expected) in [
            (".m", "@interface Foo\n@end", "Objective-C"),
            (".m", "classdef Calc\nend", "MATLAB"),
            (".m", "f[x_] := x^2", "Mathematica"),
            (".v", "module top;\nendmodule", "Verilog/SystemVerilog"),
            (".v", "fn main() {}", "V"),
            (".v", "Theorem plus_0 : forall n, n + 0 = n.", "Coq"),
            (".pl", "use strict;\nuse warnings;", "Perl"),
            (".pl", ":- module(foo).", "Prolog"),
            (".fs", "let x = 1", "F#"),
            (".fs", ": square dup * ;", "Forth"),
            (".cl", "__kernel void add()", "OpenCL"),
            (".cl", "(defun add (x y) (+ x y))", "Common Lisp"),
            (".cls", "public class Foo {", "Apex"),
            (".cls", "\\documentclass{article}", "TeX/LaTeX"),
            (".pp", "class web {", "Puppet"),
            (".pp", "program Hello;", "Pascal"),
            (".pro", "TEMPLATE = app", "QMake"),
            (".pro", ":- module(foo).", "Prolog"),
        ] {
            let candidates = candidates_for_extension(extension);
            assert!(
                candidates.len() >= 2,
                "{extension} should have multiple candidates"
            );
            let (language, hinted) =
                resolve_ambiguous_extension(extension, &candidates, content).unwrap();
            assert_eq!(
                language.name, expected,
                "wrong hint result for {extension}: {content}"
            );
            assert!(hinted, "{extension} should be resolved by a hint");
        }

        let (language, hinted) =
            resolve_ambiguous_extension(".m", &candidates_for_extension(".m"), "x = 1;").unwrap();
        assert_eq!(language.name, "Objective-C");
        assert!(!hinted, "no hint should fall back to the preference table");
    }
}
