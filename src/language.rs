/// Language definition module: defines file extensions and comment rules
/// for each supported programming language.
use std::collections::HashMap;

/// Describes the comment rules for a language
#[derive(Debug, Clone)]
pub struct CommentRule {
    /// Line comment prefixes, e.g. ["//", "#"]
    pub line_comments: Vec<&'static str>,
    /// Block comment start marker, e.g. "/*"
    pub block_comment_start: Option<&'static str>,
    /// Block comment end marker, e.g. "*/"
    pub block_comment_end: Option<&'static str>,
    /// String delimiters (to avoid false positives with comment markers inside strings)
    pub string_delimiters: Vec<&'static str>,
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

/// Returns the list of all supported languages
pub fn supported_languages() -> Vec<Language> {
    vec![
        // ─── Rust ───
        Language {
            name: "Rust",
            extensions: vec![".rs"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── C / C++ ───
        Language {
            name: "C/C++",
            extensions: vec![".c", ".h", ".cpp", ".hpp", ".cc", ".cxx", ".hxx", ".c++", ".h++"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Java ───
        Language {
            name: "Java",
            extensions: vec![".java"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Kotlin ───
        Language {
            name: "Kotlin",
            extensions: vec![".kt", ".kts"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Scala ───
        Language {
            name: "Scala",
            extensions: vec![".scala"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Go ───
        Language {
            name: "Go",
            extensions: vec![".go"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'", "`"],
            },
        },
        // ─── Swift ───
        Language {
            name: "Swift",
            extensions: vec![".swift"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\""],
            },
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
            },
        },
        // ─── Ruby ───
        Language {
            name: "Ruby",
            extensions: vec![".rb", ".rake"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: Some("=begin"),
                block_comment_end: Some("=end"),
                string_delimiters: vec!["\"", "'", "`"],
            },
        },
        // ─── JavaScript / TypeScript ───
        Language {
            name: "JavaScript/TypeScript",
            extensions: vec![".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'", "`"],
            },
        },
        // ─── C# ───
        Language {
            name: "C#",
            extensions: vec![".cs"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── PHP ───
        Language {
            name: "PHP",
            extensions: vec![".php"],
            comment_rule: CommentRule {
                line_comments: vec!["//", "#"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Shell ───
        Language {
            name: "Shell",
            extensions: vec![".sh", ".bash", ".zsh"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Perl ───
        Language {
            name: "Perl",
            extensions: vec![".pl", ".pm"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Lua ───
        Language {
            name: "Lua",
            extensions: vec![".lua"],
            comment_rule: CommentRule {
                line_comments: vec!["--"],
                block_comment_start: Some("--[["),
                block_comment_end: Some("]]"),
                string_delimiters: vec!["\"", "'"],
            },
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
            },
        },
        // ─── SQL ───
        Language {
            name: "SQL",
            extensions: vec![".sql"],
            comment_rule: CommentRule {
                line_comments: vec!["--"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["'", "\""],
            },
        },
        // ─── Haskell ───
        Language {
            name: "Haskell",
            extensions: vec![".hs", ".lhs"],
            comment_rule: CommentRule {
                line_comments: vec!["--"],
                block_comment_start: Some("{-"),
                block_comment_end: Some("-}"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Zig ───
        Language {
            name: "Zig",
            extensions: vec![".zig"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Vim Script ───
        Language {
            name: "Vim Script",
            extensions: vec![".vim"],
            comment_rule: CommentRule {
                line_comments: vec!["\""],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["'", "\""],
            },
        },
        // ─── TOML ───
        Language {
            name: "TOML",
            extensions: vec![".toml"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── YAML ───
        Language {
            name: "YAML",
            extensions: vec![".yml", ".yaml"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
            },
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
            },
        },
        // ─── Dart ───
        Language {
            name: "Dart",
            extensions: vec![".dart"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── R ───
        Language {
            name: "R",
            extensions: vec![".r", ".R"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
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
            },
        },
        // ─── Julia ───
        Language {
            name: "Julia",
            extensions: vec![".jl"],
            comment_rule: CommentRule {
                line_comments: vec!["#"],
                block_comment_start: Some("#="),
                block_comment_end: Some("=#"),
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Fortran ───
        Language {
            name: "Fortran",
            extensions: vec![".f", ".f90", ".f95", ".f03", ".f08"],
            comment_rule: CommentRule {
                line_comments: vec!["!"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Ada ───
        Language {
            name: "Ada",
            extensions: vec![".adb", ".ads"],
            comment_rule: CommentRule {
                line_comments: vec!["--"],
                block_comment_start: None,
                block_comment_end: None,
                string_delimiters: vec!["\"", "'"],
            },
        },
        // ─── Vue ───
        Language {
            name: "Vue",
            extensions: vec![".vue"],
            comment_rule: CommentRule {
                line_comments: vec!["//"],
                block_comment_start: Some("/*"),
                block_comment_end: Some("*/"),
                string_delimiters: vec!["\"", "'", "`"],
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
            },
        },
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
