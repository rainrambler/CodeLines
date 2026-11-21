# CodeLines

A fast, multi-language code line statistics tool written in Rust.

Count lines of code, comments, and blanks across your entire project — intelligently.

## Features

- **122 Languages Supported** — Rust, C/C++, C#, Java, Kotlin, Scala, Go, Swift, Python, Ruby, JavaScript/TypeScript, PHP, Lua, Haskell, SQL, HTML/CSS, Markdown, plus CUDA, OpenCL, GLSL/HLSL/Metal, Verilog/VHDL, Solidity, Move, Zig, Nim, Nix, HCL/Terraform, PowerShell, Dart, Elixir, Erlang, Julia, Fortran, Ada, Vue, Svelte and many more. Run `codelines --list` for the full list.
- **Smart Comment Detection** — Distinguishes line comments (`//`, `#`, `--`, `%`, etc.) and block comments (`/* */`, `""" """`, `<!-- -->`, `{- -}`, etc.) per language, including **nested blocks** (Rust, Kotlin, Scala, Swift, D, OCaml, Haskell family) and multiple block syntaxes (Pascal, D, Lua).
- **String-Aware Parsing** — Correctly ignores comment-like markers inside raw/verbatim strings (`r#"..."#`, `@"..."`, `"""..."""`), character literals, heredocs and PowerShell here-strings. Rust lifetimes (`&'a str`), Verilog literals (`4'b1010`) and Ada/VHDL attributes are not treated as strings.
- **Line Classification** — Each line is classified as: **code**, **comment**, **mixed** (code + comment), or **blank**.
- **Recursive Directory Scanning** — Walks entire directory trees with sensible defaults that skip VCS directories, build artifacts, and dependency folders.
- **`.gitignore` Aware** — Honours `.gitignore` files (including nested ones and those in parent directories); disable with `--no-gitignore` or the GUI checkbox.
- **Flexible Filtering** — Filter by language, exclude specific directories or files.
- **Multiple Output Formats** — Colored terminal output, plain text (pipe-friendly), or CSV.
- **Verbose & Per-File Modes** — Drill down into detailed per-file statistics or see a compact summary.
- **Ratio Calculations** — Code% and comment% for each language and overall totals.
- **Desktop GUI** — a separate crate (`codelines-gui`) with live progress, charts, sortable tables and CSV/JSON export.

See [docs/LANGUAGE_SUPPORT_DESIGN.md](docs/LANGUAGE_SUPPORT_DESIGN.md) for the language
registry, comment-rule model and roadmap for additional languages.

## Installation

### From Source

```bash
cargo build --release
```

The binary will be at `target/release/codelines`. The GUI is a separate crate:

```bash
cargo install --path gui
```

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

# .gitignore files are honoured by default; count everything instead
codelines --no-gitignore

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

## Desktop GUI

CodeLines also ships a native desktop interface built with [egui](https://github.com/emilk/egui).
It lives in its own workspace crate (`gui/`) and reuses the exact same analysis
engine as the CLI, so the CLI binary stays dependency-light.

```bash
# Build and run the GUI (optionally pointing it at a project folder)
cargo run --release -p codelines-gui
cargo run --release -p codelines-gui -- /path/to/project
```

Release builds are linked with the Windows GUI subsystem, so double-clicking
`codelines-gui.exe` opens just the app window without a console flashing up.
Debug builds keep the console window for log output.

The GUI offers:

- A project picker with drag & drop, plus include/exclude filters, a `.gitignore` toggle and a searchable language selector.
- Live progress while scanning and analyzing (Esc cancels a running analysis).
- An **Overview** tab with KPI cards, a stacked bar chart of lines by language and a donut chart of each language's share.
- A **Files** tab with a filterable, sortable per-file table (double-click a row to reveal the file in your file manager).
- A **Diagnostics** tab listing skipped file types and read warnings.
- Export to language-summary CSV, per-file CSV, JSON or a plain-text report.
- Light and dark themes.

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + O` | Open a project folder |
| `Cmd/Ctrl + R` / `F5` | Analyze again |
| `Cmd/Ctrl + E` | Export the language summary (CSV) |
| `Esc` | Cancel a running analysis |

See [docs/GUI_DESIGN.md](docs/GUI_DESIGN.md) for the full design: architecture, screen
specifications, visual system, data model and performance notes.

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

1. **Scan** — Recursively walks directories, identifying files by extension using built-in language definitions and skipping paths matched by `.gitignore` rules.
2. **Read** — Loads file contents from disk.
3. **Analyze** — Parses each line with language-specific comment rules, tracking block comment and multi-line string state across lines.
4. **Aggregate** — Sums statistics per language and globally.
5. **Output** — Renders results in the chosen format (colored table, plain text, or CSV).

Steps 1–4 live in the `codelines` library (`report::analyze`) and are shared by the CLI and the GUI;
step 5 is implemented by each front-end crate (`codelines` CLI and `codelines-gui`).

## Development

```bash
cargo test                       # core analysis pipeline (default crate)
cargo test -p codelines-gui      # GUI crate + export formats
cargo clippy --workspace --all-targets
```

## License

MIT
