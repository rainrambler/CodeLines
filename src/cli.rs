/// CLI argument definition module
use clap::Parser;

/// codelines - Code line statistics tool with per-language breakdown and comment analysis
#[derive(Parser, Debug)]
#[command(
    name = "codelines",
    version,
    about = "Code line statistics tool - per-language breakdown with comment analysis",
    long_about = "codelines is a high-performance code line statistics tool that recursively\n\
                  scans project directories, counting total lines, code lines, comment lines,\n\
                  mixed lines, and blank lines per programming language.\n\
                  Supports 30+ programming languages, intelligently detects comments\n\
                  (both block and line comments), and correctly handles comment-like\n\
                  markers inside strings to avoid false positives."
)]
pub struct Args {
    /// Directories or file paths to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// Only count the specified language(s) (repeatable), e.g. --lang Rust --lang Python
    #[arg(short, long = "lang", value_name = "LANGUAGE")]
    pub languages: Vec<String>,

    /// Exclude the specified directory (repeatable), e.g. --exclude vendor --exclude generated
    #[arg(short, long = "exclude", value_name = "DIR")]
    pub exclude_dirs: Vec<String>,

    /// Exclude the specified file (repeatable), e.g. --exclude-file lock.json
    #[arg(long = "exclude-file", value_name = "FILE")]
    pub exclude_files: Vec<String>,

    /// Show detailed statistics (separate pure code/comment/mixed lines)
    #[arg(short, long)]
    pub verbose: bool,

    /// Plain text output (no colors, suitable for piping to a file)
    #[arg(short, long)]
    pub plain: bool,

    /// Output in CSV format
    #[arg(long)]
    pub csv: bool,

    /// List all supported languages and their file extensions
    #[arg(long)]
    pub list: bool,

    /// Show per-file detailed statistics
    #[arg(short = 'f', long)]
    pub files: bool,
}
