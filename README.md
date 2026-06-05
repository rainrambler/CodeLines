# CodeLines

A fast, multi-language code line statistics tool written in Rust.

Count lines of code, comments, and blanks across your entire project — intelligently.

## Features

- **35+ Languages Supported** — Rust, C/C++, Java, Kotlin, Scala, Go, Swift, Python, Ruby, JavaScript/TypeScript, C#, PHP, Shell, Perl, Lua, HTML, CSS/SCSS/Less, SQL, Haskell, Zig, Vim Script, TOML, YAML, XML, Dart, R, Elixir, Erlang, Julia, Fortran, Ada, Vue, Markdown, and more.
- **Smart Comment Detection** — Distinguishes line comments (`//`, `#`, `--`, `%`, etc.) and block comments (`/* */`, `""" """`, `<!-- -->`, `{- -}`, etc.) per language.
- **String-Aware Parsing** — Correctly ignores comment-like markers inside string literals to avoid false positives.
- **Line Classification** — Each line is classified as: **code**, **comment**, **mixed** (code + comment), or **blank**.
- **Recursive Directory Scanning** — Walks entire directory trees with sensible defaults that skip VCS directories, build artifacts, and dependency folders.
- **Flexible Filtering** — Filter by language, exclude specific directories or files.
- **Multiple Output Formats** — Colored terminal output, plain text (pipe-friendly), or CSV.
- **Verbose & Per-File Modes** — Drill down into detailed per-file statistics or see a compact summary.
- **Ratio Calculations** — Code% and comment% for each language and overall totals.

## Installation

### From Source

```bash
cargo build --release
```

The binary will be at `target/release/codelines`.

### Requirements

- Rust 1.80+ (edition 2024)

## Usage

```bash
# Count lines in the current directory
codelines

# Count lines in specific directories
codelines src/ tests/

# Filter by language
codelines --lang Rust --lang Python

# Exclude directories
codelines --exclude vendor --exclude generated

# Exclude specific files
codelines --exclude-file Cargo.lock

# Verbose output (separate code/comment/mixed columns)
codelines --verbose

# Show per-file statistics
codelines --files

# Plain text output (no ANSI colors)
codelines --plain

# CSV output
codelines --csv > stats.csv

# List all supported languages
codelines --list
```

## Example Output

```
Language                 Files      Total   Eff.Code   Comments      Blank   Code%  Comment%
───────────────────────────────────────────────────────────────────────────────────────────
Rust                        8       1200        850        200        150    70.8%    16.7%
Python                      5        600        450         80         70    75.0%    13.3%
JavaScript/TypeScript       3        300        200         50         50    66.7%    16.7%
───────────────────────────────────────────────────────────────────────────────────────────
Total                      16       2100       1500        330        270    71.4%    15.7%
```

## How It Works

1. **Scan** — Recursively walks directories, identifying files by extension using built-in language definitions.
2. **Read** — Loads file contents from disk.
3. **Analyze** — Parses each line with language-specific comment rules, tracking block comment and multi-line string state across lines.
4. **Aggregate** — Sums statistics per language and globally.
5. **Output** — Renders results in the chosen format (colored table, plain text, or CSV).

## License

MIT
