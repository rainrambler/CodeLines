/// Language definition module: defines file extensions and comment rules
/// for each supported programming language.
use std::collections::HashMap;

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

/// A comment region delimited by line-oriented markers (Ruby `=begin`/`=end`, Perl POD).
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

    pub fn end_at_start(mut self) -> Self {
        self.end_scope = LineScope::StartOfLine;
        self
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
    /// Character-literal rules.
    pub chars: Vec<CharLiteral>,
    /// Line-oriented comment regions.
    pub regions: Vec<RegionComment>,
    /// Heredoc markers, e.g. `<<` or `<<<`.
    pub heredocs: Vec<&'static str>,
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
}

/// Describes a supported language
#[derive(Debug, Clone)]
pub struct Language {
    /// Display name of the language
    pub name: &'static str,
    /// File extensions (including dot, e.g. ".rs")
    pub extensions: Vec<&'static str>,
    /// Comment rules
    pub comment_rule: CommentRule,
}

impl Language {
    /// Returns true if the given file extension belongs to this language
    pub fn matches_extension(&self, ext: &str) -> bool {
        self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
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

/// Returns the list of all supported languages
pub fn supported_languages() -> Vec<Language> {
    vec![
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
                .string_rule(StringLiteral::raw("R\"(", ")\""))
                .char_rule('\'', 8),
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
                .string_rule(StringLiteral::new("`")),
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
                .heredoc("<<<"),
        },
        // ─── Shell ───
        Language {
            name: "Shell",
            extensions: vec![".sh", ".bash", ".zsh"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#").require_whitespace_before())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::with_escape("'", "'", StringEscape::None))
                .heredoc("<<"),
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
                .string_rule(StringLiteral::new("'")),
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
        },
        // ─── YAML ───
        Language {
            name: "YAML",
            extensions: vec![".yml", ".yaml"],
            comment_rule: CommentRule::default()
                .line_rule(LineComment::new("#").require_whitespace_before())
                .string_rule(StringLiteral::new("\""))
                .string_rule(StringLiteral::doubled("'", "'")),
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
    ]
}

/// Find the language definition matching a file extension
pub fn find_language_by_ext(ext: &str) -> Option<Language> {
    supported_languages()
        .into_iter()
        .find(|lang| lang.matches_extension(ext))
}

/// Build extension → language name mapping table
pub fn extension_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    for lang in supported_languages() {
        for ext in &lang.extensions {
            map.insert(*ext, lang.name);
        }
    }
    map
}

/// Get the list of all supported language names
pub fn list_language_names() -> Vec<&'static str> {
    supported_languages().iter().map(|l| l.name).collect()
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
    }
}
