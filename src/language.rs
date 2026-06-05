/// 语言定义模块：定义各种编程语言的文件扩展名与注释规则
use std::collections::HashMap;

/// 描述一种语言的注释规则
#[derive(Debug, Clone)]
pub struct CommentRule {
    /// 单行注释前缀列表，如 ["//", "#"]
    pub line_comments: Vec<&'static str>,
    /// 多行注释起始标记，如 "/*"
    pub block_comment_start: Option<&'static str>,
    /// 多行注释结束标记，如 "*/"
    pub block_comment_end: Option<&'static str>,
    /// 字符串界定符（用于避免误判字符串中的注释标记）
    pub string_delimiters: Vec<&'static str>,
}

/// 描述一种受支持的语言
#[derive(Debug, Clone)]
pub struct Language {
    /// 语言显示名称
    pub name: &'static str,
    /// 文件扩展名列表（包含点号，如 ".rs"）
    pub extensions: Vec<&'static str>,
    /// 注释规则
    pub comment_rule: CommentRule,
}

impl Language {
    /// 判断给定文件扩展名是否属于该语言
    pub fn matches_extension(&self, ext: &str) -> bool {
        self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
    }
}

/// 获取所有支持的语言列表
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

/// 根据文件扩展名查找对应的语言
pub fn find_language_by_ext(ext: &str) -> Option<Language> {
    supported_languages()
        .into_iter()
        .find(|lang| lang.matches_extension(ext))
}

/// 获取扩展名 → 语言名称的映射表
pub fn extension_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    for lang in supported_languages() {
        for ext in &lang.extensions {
            map.insert(*ext, lang.name);
        }
    }
    map
}

/// 获取所有支持的语言名称列表
pub fn list_language_names() -> Vec<&'static str> {
    supported_languages().iter().map(|l| l.name).collect()
}
