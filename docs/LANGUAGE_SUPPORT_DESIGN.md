# CodeLines Language Support — Design

Status: **Phases 1–4 and Phase 2b implemented** (132 languages). Phase 5 is
proposed here and not yet implemented.

This document describes how CodeLines grows from “a fixed list of languages with
one hard-coded comment model” to a language platform that can absorb new
languages safely. It explains the current pipeline, the concrete gaps that block
further languages, the target rule model, and a phased rollout with acceptance
criteria.

Related files:

- [src/language.rs](../src/language.rs) — language registry and comment rules
- [src/analyzer.rs](../src/analyzer.rs) — line classification state machine
- [src/scanner.rs](../src/scanner.rs) — file discovery and extension mapping
- [src/report.rs](../src/report.rs) — shared scan → analyze pipeline
- [docs/GUI_DESIGN.md](./GUI_DESIGN.md) — desktop front-end design

---

## 1. Goals and non-goals

### Goals

| # | Goal | Measure |
|---|------|---------|
| 1 | Add a language without touching the analyzer | One declarative entry in `language.rs` |
| 2 | Never miscount a comment marker inside a literal | Fixture suite per literal form |
| 3 | Handle the common real-world comment syntaxes correctly | Nested blocks, raw strings, heredocs, column-0 rules |
| 4 | Keep the scan fast | < 5 % regression on the self-scan benchmark |
| 5 | Never break CLI/GUI compatibility | Existing language names, `--list`, CSV columns unchanged |
| 6 | Make coverage auditable | Every registered extension is unique and tested |

### Non-goals

- Full parsing or semantic analysis. CodeLines classifies lines; it does not
  build ASTs, resolve macros, or evaluate template languages.
- Exact counting of generated, minified, or embedded code.
- Counting binary notebook formats cell-by-cell (a later, opt-in extension).
- Replacing `tokei`-style language detection at any cost. Predictability and
  stable output matter more than covering every rare file name.

---

## 2. Current architecture

```
paths ──► scanner::scan_directory_with_progress ──► files_by_language
             │                                      │
             │ language::registry()                 ▼
             │  filename → extension → hints     language::find_language_by_name
             │  → shebang                        │
             │                                   ▼
             └─────────────────────► analyzer::CompiledSyntax (once per language)
                                                 │
                                                 ▼
       analyzer::analyze_content_compiled / analyze_embedded ──► LineStats + RegionInfo
                                                 │
                                                 ▼
                         output::build_summaries ──► Report ──► CLI / GUI
```

Key properties:

- `Language` holds a display name, unambiguous and shared extensions, file
  names, shebangs, disambiguation hints and a `CommentRule`.
- `Registry` is built once with `OnceLock` and provides name, extension,
  filename and shebang lookups.
- `CommentRule` keeps the legacy fields and adds detailed `line`, `blocks`,
  `strings`, `chars`, `regions` and `heredocs` rule sets.
- `CompiledSyntax` merges legacy and detailed rules, sorts delimiters longest
  first, and is built once per language by `report::analyze`.
- `analyze_content_compiled` keeps an explicit `Mode` across lines: normal,
  block (with nesting depth), string, region or heredoc.
- `analyze_embedded` splits HTML-like and Markdown hosts into regions and
  feeds each region through the host or embedded language syntax.
- The GUI calls `languages()` and no longer rebuilds the language list per
  frame.

Phase 1 extended the registry only. Phase 2 replaced the analyzer internals.
Phase 3 added cached lookup tables, filename/shebang detection and ambiguity
resolution. Phase 4 added embedded regions for HTML-like hosts and Markdown.
Phase 5 remains future work.

---

## 3. Capability gaps

The table records the gaps that motivated Phases 1–4. **Phase 2 closed gaps
1–8 and 12; Phase 3 closed gaps 9–10 and completed 14** (static registry);
**Phase 4 closed gap 11 for HTML/XML, Vue/Svelte/Astro/Razor and Markdown**.
Gap 13 (docstring policy) remains open, and other template families (Jupyter,
ERB/EJS, Haml/Slim, Blade, MDX) are future embedded-region extensions.

Each gap below is a concrete correctness bug or a hard blocker for a new
language.

| # | Gap | Current behavior | Example |
|---|-----|------------------|---------|
| 1 | One block-comment pair only | Second pair never recognized | Pascal `{ }` **and** `(* *)`; D `/* */` **and** `/+ +/`; Lua `--[[ ]]` and `--[==[ ]==]` |
| 2 | No nested block comments | Inner `/*` is ignored, first `*/` closes | Rust, Swift, Kotlin, Scala, D, Nim, OCaml, F#, Racket, Chapel, Odin, Pony |
| 3 | No raw/verbatim/text-block strings | Quote ends at the next quote | Rust `r#"..."#`, C++ `R"(...)"`, C# `@"..."`, Java/Kotlin `"""` |
| 4 | `'` is always a string opener | Lifetimes/attributes swallow later markers | Rust `&'a str`, Ada/VHDL attribute `'event`, Verilog `4'b1010` |
| 5 | No column/token constraints | Any occurrence counts | Ruby `=begin` (column 0), MATLAB `%{` (alone on line), Haskell `-->` (operator, not comment) |
| 6 | No case-insensitive prefixes | `REM` not a comment | Batch `REM ...`, VB `Rem ...` |
| 7 | No line comment requires whitespace/EOL | `#` mid-token counts | Shell `${#var}`, Rust `r#ident`, C preprocessor |
| 8 | No heredoc/line-terminated regions | Body lines counted as code | Shell `<<EOF`, PowerShell here-strings, Perl POD |
| 9 | Extension-only detection | Extensionless files ignored | `Makefile`, `Dockerfile`, `BUILD`, `CMakeLists.txt` |
| 10 | One extension → one language | Ambiguity silently resolved by order | `.m` (Obj-C/MATLAB), `.pl` (Perl/Prolog), `.v` (Verilog/V/Coq), `.fs` (F#/Forth), `.cl` (OpenCL/Common Lisp) |
| 11 | No embedded regions | Outer syntax applied to every line | HTML `<script>`/`<style>`, Vue/Svelte/Astro, JSX, Markdown fences |
| 12 | One precedence order | String check always wins | Vim `"` comment vs string; Smalltalk `"..."` comment vs `'...'` string |
| 13 | Docstrings counted as code | Policy is implicit | Python/Elixir/Crystal triple-quoted docstrings |
| 14 | Registry rebuilt per call | Allocations in hot paths | `supported_languages()` built 132 entries per call; now cached in `OnceLock` |

Two smaller issues already affect existing languages:

- `.R` was registered next to `.r`; because the scanner lowercases extensions,
  only `.r` was reachable. Phase 1 removes the redundant entry and adds a test
  that rejects case-insensitive duplicate extensions.
- `extension_map()` and the scanner’s old `ext_to_lang` resolved duplicates
  differently (first match vs last insert). Phase 3 replaces this with explicit
  candidate lists and a uniqueness guard for unambiguous extensions.

---

## 4. Design principles

1. **Declarative rules, one state machine.** Languages differ only in data; the
   analyzer never contains language-specific branches.
2. **Conservative classification.** When unsure, prefer “code”. False “comment”
   results hide code; false “code” results are visible and easy to fix.
3. **Fail closed at registration.** Duplicate extensions, incomplete block
   pairs, and unknown rule fields fail tests, not production scans.
4. **Stable identity.** Display names and CSV columns never change silently;
   new aliases are additive.
5. **Measure first.** Every rule-model addition ships with fixtures and a
   self-scan benchmark.

---

## 5. Rule and detection model (Phases 2–3 implemented)

### 5.1 Registry (implemented)

```rust
pub struct Language {
    pub name: &'static str,
    /// Unambiguous extensions.
    pub extensions: Vec<&'static str>,
    /// Shared extensions (e.g. ".m"); resolved by hints/preference.
    pub ambiguous_extensions: Vec<&'static str>,
    pub filenames: Vec<&'static str>,
    pub shebangs: Vec<&'static str>,
    /// Lowercase substrings used to disambiguate shared extensions.
    pub hints: Vec<&'static str>,
    pub comment_rule: CommentRule,
}

pub struct Registry {
    languages: Vec<Language>,
    by_name: HashMap<String, usize>,
    by_extension: HashMap<String, Vec<usize>>,
    by_filename: HashMap<String, usize>,
    by_shebang: HashMap<&'static str, usize>,
}
```

- `registry()` builds once with `OnceLock`; `languages()` returns a
  `&'static [Language]`.
- `supported_languages() -> Vec<Language>` remains as a compatibility snapshot.
- Detection metadata is attached in `apply_detection_metadata`, keeping syntax
  definitions and discovery metadata separate.
- The GUI now calls `languages()` and no longer rebuilds 132 definitions per
  frame.
- `CompiledSyntax` is still built once per language by `report::analyze`.
- Command-line aliases (`--lang js`) remain a future addition.

### 5.2 Rule schema (implemented)

```rust
pub struct CommentRule {
    // Legacy fields, still merged by CompiledSyntax:
    pub line_comments: Vec<&'static str>,
    pub block_comment_start: Option<&'static str>,
    pub block_comment_end: Option<&'static str>,
    pub string_delimiters: Vec<&'static str>,

    // Phase 2 rule sets:
    pub line: Vec<LineComment>,
    pub blocks: Vec<BlockComment>,
    pub strings: Vec<StringLiteral>,
    pub chars: Vec<CharLiteral>,
    pub regions: Vec<RegionComment>,
    pub heredocs: Vec<&'static str>,
}

pub struct LineComment {
    pub prefix: &'static str,
    pub scope: LineScope,
    pub case_insensitive: bool,
    pub boundary_after: Option<BoundaryAfter>,
    pub require_whitespace_before: bool,
}

pub struct BlockComment {
    pub start: &'static str,
    pub end: &'static str,
    pub nested: bool,
    pub start_scope: LineScope,
    pub end_scope: LineScope,
}

pub struct StringLiteral {
    pub open: &'static str,
    pub close: &'static str,
    pub escape: StringEscape,
    pub multiline: bool,
    pub close_scope: LineScope,
    pub open_requires_line_end: bool,
}

pub enum StringEscape { None, Backslash, Backtick, DoubledQuote }
pub enum LineScope { Anywhere, StartOfLine, FirstNonSpace, AloneOnLine }
pub enum BoundaryAfter { Word, Symbol }
```

Definitions use builder methods (`line_rule`, `block_rule`, `string_rule`,
`char_rule`, `region_rule`, `heredoc`). Legacy fields remain so existing
definitions and external `CommentRule` literals keep working; they are merged
into the same compiled syntax.

A language may have **many** rules, in any order. Precedence is: longest
delimiter first, then rule kind (character literal → string → block → region →
line comment). This supports:

- Pascal: `//`, `{ }`, `(* *)`
- Lua: `--`, `--[[ ]]`, `--[==[ ]==]`
- Rust: `//`, `///`, `//!`, `/* */` (nested), `"`, `'` (char only), `r#"..."#`
- Vim: `"` comment first, `'` string
- Smalltalk: `"..."` block comment, `'...'` string
- Shell: `#` with boundary, `<<EOF` heredoc
- Batch/VB: case-insensitive `REM`/`Rem` with a word boundary
- Ruby: `#`, `=begin`/`=end` at column 0 (`%q` literals are not modeled)
- PowerShell: `@"..."@`/`@'...'@` here-strings, backtick escapes
- Verilog: `//`, `/* */`, `"` strings, `'` allowed in numeric literals

### 5.3 Analyzer v2 (implemented)

The two booleans were replaced by an explicit mode and a compiled matcher:

```rust
enum Mode {
    Normal,
    Block { rule: usize, depth: usize },
    String { rule: usize },
    Region { rule: usize },
    Heredoc { terminator: String, strip: HeredocStrip },
}

pub struct CompiledSyntax {
    strings: Vec<StringLiteral>,  // longest open first
    blocks: Vec<BlockComment>,    // longest start first
    lines: Vec<LineComment>,      // longest prefix first
    chars: Vec<CharLiteral>,
    regions: Vec<RegionComment>,
    heredocs: Vec<&'static str>,
}
```

Loop per line:

1. If in a block/string/region/heredoc mode, scan for that mode’s end (and
   nested starts when the block is nested), then return to `Normal`.
2. In `Normal`, scan for the next match among compiled rules.
3. When a rule matches, record `has_comment` or `has_code` and push/pop modes.
4. Unmatched non-whitespace characters set `has_code`; whitespace never does
   (otherwise indentation before a comment would produce a mixed line).

Implemented properties:

- **Compiled once per language** by `report::analyze`, not per file.
- **Column tracking** on the raw line enables `StartOfLine`, `FirstNonSpace`,
  `AloneOnLine` and `Column(n)` rules (COBOL column 7).
- **Token boundaries** (`BoundaryAfter::Word`/`Symbol`, optional
  `require_whitespace_before`) fix Shell `${#var}`, Haskell `-->`, Batch `REM`
  and Vim `"` false positives.
- **Multiple block rules** per language (Pascal, D, Lua, Ruby).
- **Nested block comments** for Rust, Kotlin, Scala, Swift, D, Nim, OCaml,
  F#, Haskell-family languages and others.
- **Raw/verbatim strings**: Rust `r#"..."#`, C++ `R"delim(...)delim"` with
  dynamic delimiters, C# `@"..."`/`"""`, Go backticks, Nix `''...''`, Lua long
  strings.
- **Character literals** distinct from strings, with a look-ahead length limit,
  so Rust/OCaml/F# type variables and Ada/VHDL/Verilog attributes stay code.
- **Doubled-quote escapes** for SQL, Smalltalk, Vim, Pascal, VB, PowerShell.
- **Heredocs** for Shell (`<<`, `<<-`, `<< EOF`), PHP (`<<<`), Ruby
  (`<<`, `<<-`, `<<~`) and Perl; the body is code.
- **PowerShell here-strings** `@"..."@` / `@'...'@` with start-of-line closers.
- **Line-oriented regions** for Ruby `=begin`/`=end`, Perl POD and SAS
  `* ... ;`.
- **Byte-based scanner**: the parser reads `&[u8]`; UTF-8 continuation bytes
  can never match an ASCII delimiter, so non-ASCII text is safe without a
  per-line `Vec<char>`.

Implementation notes:

- Ruby/Perl unquoted heredoc delimiters must start with an uppercase letter;
  quoted lowercase delimiters (`<<~'sql'`) are accepted. This keeps `arr << x`
  and `$x << y` from being mistaken for heredocs.
- C++ raw-string delimiters are limited to 16 characters, matching the
  standard.
- A line that continues a string/region counts as code, matching earlier
  behavior.

### 5.4 Detection pipeline (implemented)

```
path
 ├─ 1. exact filename / prefix    → Makefile, Dockerfile, CMakeLists.txt, BUILD
 ├─ 2. extension candidates       → unambiguous: take it; shared: continue
 ├─ 3. content hints              → lowercase substring score (8 KiB sample)
 ├─ 4. preference table           → documented default for the extension
 └─ 5. shebang (no extension)     → `#!/usr/bin/env python3`, `#!/bin/bash`
```

- The language filter (`--lang`) is applied **before** ambiguity resolution, so
  `--lang MATLAB` selects `.m` files as MATLAB even when their content would
  hint Objective-C.
- Shebang parsing handles `env`, `-S`, option arguments (`-u NAME`), Windows
  `.exe` suffixes and version suffixes (`python3.11` → `python`).
- An 8 KiB sample is read only for shared extensions and extensionless files;
  unique extensions and filename matches never read a sample.
- When no hint is conclusive, the preference table decides and the scanner
  aggregates one diagnostic per `extension -> language` pair:
  `Detection: N file(s) used the default language for ambiguous extension .m -> Objective-C`.
- Unrecognized extensions and unresolved extensionless files still appear in
  `unknown_extensions`.

Shared extensions get candidate lists plus cheap hints:

| Extension | Candidates | Hint |
|-----------|------------|------|
| `.m` | Objective-C, MATLAB, Mathematica | `@interface` / `classdef` / `(*` / `:=` |
| `.v` | Verilog, V, Coq | `module`/`endmodule`, `fn `, `theorem ` |
| `.pl` | Perl, Prolog | `use strict`, `:-` |
| `.fs` | F#, Forth | `let `, `: ` |
| `.cl` | OpenCL, Common Lisp | `__kernel`, `(defun` |
| `.cls` | Apex, TeX/LaTeX | `public class`, `\documentclass` |
| `.pp` | Puppet, Free Pascal | `class `, `program ` |
| `.pro` | QMake, Prolog | `TEMPLATE`, `:-` |

Hinting stays intentionally small: a few substring checks, no parsing.

### 5.5 Embedded regions

Phase 4 handles mixed-content files by splitting a file into regions and
classifying each region with its own rule set:

| Host | Regions |
|------|---------|
| HTML/XML | markup + `<!-- -->`; `<script>` → JS/TS; `<style>` → CSS |
| Vue/Svelte/Astro | `<template>` → markup; `<script>` → JS/TS; `<style>` → CSS |
| Razor/Blazor | markup + `@* *@`; `<script>`/`<style>` regions |
| Markdown | prose; fenced blocks → language by info string; inline code ignored |
| JSX/TSX | JS/TS rules already cover comments; no additional splitter needed |

`Language.embedded` selects the strategy: `EmbeddedKind::Html` for
HTML/XML/Vue/Svelte/Astro/Razor, `EmbeddedKind::Markdown` for Markdown. The
splitter is a deliberately small scanner that recognizes host delimiters only;
it does not validate markup nesting or parse the host language.

HTML-like rules:

- `<script>` defaults to JavaScript/TypeScript. `lang="ts|js|tsx|jsx"` and
  `type="module|text/javascript|application/javascript|text/typescript|…"` are
  accepted; other script types (for example `application/json`) are **not**
  treated as embedded code.
- `<style>` defaults to CSS and accepts `lang="stylus"` or
  `type="text/stylus"`.
- Self-closing tags and `<script src="…"></script>` do not create a region.
- An opening tag-only line (`<script>`, `<style>`) is classified with the host
  syntax; content and closing-tag lines are classified with the embedded
  syntax. A line containing inline content is classified with the embedded
  syntax.

Markdown rules:

- Fences are ``` or `~~~` with up to three leading spaces and at least three
  markers. A closing fence must use the same marker, at least the same length,
  and may only be followed by whitespace.
- The info string is resolved through an alias table (`rust`/`rs`, `py`,
  `js`/`ts`, `c++`, `sh`, `yml`, `proto`, …), including `{.python}` forms.
  Unknown or plain-text info strings use a neutral “Text” syntax where every
  line is code.
- Fence lines count as code; the region spans the opening through closing fence.
  An unclosed fence runs to EOF.
- Inline backticks remain ordinary host text and are not parsed as fences.

Region state is reset between regions, so an unterminated comment/string in one
`<script>` or fence cannot leak into the next one. Region boundaries are
recorded per file (`RegionInfo { kind, language, start_line, end_line }`),
printed under `--files`/`--verbose`, and included in the GUI JSON export.
Embedded lines remain aggregated under the host file; they are not added to the
embedded language’s summary row.

### 5.6 Statistics policy

- Default: string literals (including docstrings) count as **code**, preserving
  current output.
- Optional `--docstrings comments` (CLI) / setting (GUI) treats docstrings as
  comments for Python, Elixir, Crystal, Julia and Mojo.
- Optional `--directives` separates preprocessor lines (`#include`, `using`,
  `import`) from code and comments. Off by default.
- Comment ratios keep their current definition (`pure comment + mixed`).

---

## 6. Phased rollout

### Phase 1 — Static coverage (implemented)

Scope: only languages that map cleanly onto the existing rule model; no analyzer
changes. This is the phase delivered with this document.

Delivered:

- 87 new language definitions, bringing the registry to **120 languages**.
- Shared private constructors (`c_style`, `hash_only`, `dash_block`, …) to keep
  registry entries one line each and make future additions cheap.
- Removal of the unreachable `.R` extension.
- Registry tests:
  - unique language names,
  - unique extensions (case-insensitive),
  - block start/end always paired,
  - at least one comment syntax per language,
  - representative classification tests for the new rule shapes.
- README, CLI help and GUI empty-state text now reflect the real count.

Coverage added in Phase 1, by family:

| Family | Languages |
|--------|-----------|
| C-style (`//`, `/* */`) | CUDA, OpenCL, GLSL, HLSL, Metal, Verilog/SystemVerilog, Groovy, Solidity, Move, Cairo, D, Pony, Odin, Chapel, ReasonML, ReScript, Haxe, ActionScript, Vala, QML, Apex, YANG, Protocol Buffers, Thrift, FlatBuffers |
| `#`-comment | Vyper, GraphQL, Crystal, Starlark, Rego, Roc, Fish, Nushell, Tcl, AWK, SPARQL, Turtle, Cap’n Proto, QMake, Puppet, JSONC, JSON5, Hjson, Jsonnet, Nix, HCL/Terraform, CMake, PowerShell, Properties, INI, Nim, Raku, Mojo |
| Dash / semicolon / percent | VHDL, TeX/LaTeX, Eiffel, Mathematica, XQuery, Smalltalk, Scheme/Racket, Common Lisp, Emacs Lisp, Clojure, OCaml/Standard ML, F#, Elm, PureScript, Idris, Agda, Dhall, Prolog, Visual Basic, Pascal, Assembly, GNU Assembly, LLVM IR, WebAssembly Text |
| Template / markup | CoffeeScript, LiveScript, Pug, Stylus, XAML, Razor, Jinja, Handlebars, Svelte, Astro |

Known Phase 1 limitations (most were closed by Phase 2):

- ~~Nested block comments close at the first end marker.~~ Closed in Phase 2.
- ~~Raw/verbatim strings are not modeled.~~ Closed for the main forms in Phase 2.
- ~~Two block syntaxes use the dominant one.~~ Closed in Phase 2 (Pascal, D, Lua).
- ~~`REM` in Visual Basic/Batch is not recognized.~~ Closed in Phase 2.
- ~~Filename-only languages (`Makefile`, `Dockerfile`) are still unknown.~~
  Closed in Phase 3.

Acceptance criteria met (at the end of Phase 1):

- `cargo test -p codelines --lib` passes.
- `cargo run -- --list` showed 120 entries (122 after Phase 2, 130 after Phase 3).
- No duplicate extension in the registry.

### Phase 2 — Rule engine correctness (implemented)

Delivered:

- Detailed rule model: `LineComment`, `BlockComment`, `StringLiteral`,
  `CharLiteral`, `RegionComment`, plus heredoc markers. Legacy `CommentRule`
  fields are retained and merged into the same compiled syntax.
- `CompiledSyntax` built once per language by `report::analyze`; strings, blocks
  and line comments are pre-sorted by delimiter length.
- Nested block comments, multiple block rules per language, column scopes
  (`StartOfLine`, `FirstNonSpace`, `AloneOnLine`), token boundaries and
  case-insensitive prefixes.
- Raw/verbatim strings for Rust, C++ (empty delimiter), C#, Go, Nix, Lua and
  others; doubled-quote escapes for SQL, Smalltalk, Vim, Pascal, VB and
  PowerShell.
- Character literals with look-ahead, so Rust lifetimes, OCaml/F#/Idris type
  variables and Ada/VHDL/Verilog attributes are not strings.
- Heredocs for Shell (`<<`, `<<-`) and PHP (`<<<`), including quoted
  delimiters; PowerShell here-strings; Ruby `=begin`/`=end` and Perl POD regions.
- Language upgrades: Rust, C/C++, Java, Kotlin, Scala, Swift, Go, C#, PHP, Shell,
  Ruby, Perl, Lua, SQL, Haskell, Zig, Vim, TOML, YAML, Dart, Julia, Fortran
  (split into free and fixed form), Ada, D, PowerShell, Nim, OCaml/Standard ML,
  F#, Idris, Agda, PureScript, Elm, Dhall, Visual Basic and Pascal.
- Added **Batch** and **Fortran (fixed form)**, bringing the registry to
  **122 languages**.
- 15 new analyzer tests plus registry wiring tests (39 library tests at the end
  of Phase 2; 44 after Phase 3).

Acceptance criteria met:

- Rust `&'a str` no longer opens a string.
- Vim `" comment` is a comment; Smalltalk `"comment"` is a comment.
- Shell `${#var}`, Haskell `-->` and Batch `REM` classify correctly.
- Nested `/* /* */ */` in Rust/Kotlin/Swift is fully covered.
- Self-scan and workspace tests pass.
- Benchmark on a 1,900-file corpus (debug builds, median of 5 runs):
  **15.1 s before → 10.3 s after**. No regression; removing the per-comparison
  `Vec<char>` allocation more than offsets the extra rules.

The Phase 2b items left after Phase 2 (Ruby/Perl heredocs, C++ raw delimiters,
the byte matcher, COBOL and SAS) were completed in the Phase 2b cleanup pass
below.

### Phase 3 — File-name and ambiguity detection (implemented)

Delivered:

- `OnceLock<Registry>` with name, extension, filename and shebang lookup tables.
- `languages()` returns a cached `&'static [Language]`; the GUI uses it instead
  of rebuilding the language list every frame. `supported_languages()` remains
  as a snapshot API.
- Filename detection for `Makefile`, `GNUmakefile`, `Makefile.*`,
  `Dockerfile`/`Dockerfile.*`, `Containerfile`/`Containerfile.*`,
  `CMakeLists.txt`, `BUILD`/`BUILD.bazel`, `WORKSPACE`/`WORKSPACE.bazel`,
  `Rakefile`, `Gemfile`, `Jenkinsfile`, `Pipfile`, `.editorconfig`,
  `meson.build` and other common names.
- Shebang detection for extensionless files, including `env`, `-S`, `-u NAME`,
  `.exe` suffixes and version suffixes.
- Candidate lists, content hints, preference table and aggregated diagnostics
  for `.m`, `.v`, `.pl`, `.fs`, `.cl`, `.cls`, `.pp` and `.pro`.
- Added **Makefile**, **Dockerfile**, **Meson**, **Objective-C**, **MATLAB**,
  **V**, **Coq** and **Forth**, bringing the registry to **130 languages**.
- Phase 3 tests: filename/shebang resolution, ambiguity hints, preference
  fallback, metadata uniqueness, end-to-end detection and language-filter
  interaction (44 library tests total).

Acceptance criteria met:

- `Makefile`, `Dockerfile`, `CMakeLists.txt`, `BUILD` and `WORKSPACE` are
  recognized.
- `#!/usr/bin/env python3` and `#!/bin/bash` files without extensions count.
- `.m`/`.v`/`.pl`/`.fs`/`.cl`/`.cls`/`.pp`/`.pro` resolve by hint or the
  documented preference table.
- Unknown extensions still appear in diagnostics; defaulted ambiguity
  resolutions produce an aggregated detection note.
- Benchmark on the same 1,900-file corpus (debug builds, 9 runs):
  Phase 2 median **16.1 s** → Phase 3 median **15.6 s** (~3 % faster). No
  regression.

### Phase 2b — Deferred correctness and performance (implemented)

Delivered:

- **Byte-based scanner.** `parse_line` now scans `&[u8]` directly and no longer
  allocates a `Vec<char>` per line. Delimiters are ASCII, and UTF-8
  continuation bytes cannot match them.
- **C++ raw strings with dynamic delimiters.** `R"delim(... )delim"` is parsed
  into a `Mode::RawString` whose close marker is built from the delimiter
  (maximum 16 characters).
- **Heredoc rules with options.** `Heredoc { allow_space, require_uppercase }`
  enables Shell `<< EOF`, PHP `<<< EOT`, Ruby `<<~SQL` and Perl `<<EOF` while
  keeping `arr << x` and `$x << 1` as operators.
- **COBOL fixed-column comments.** New `LineScope::Column(n)` handles the
  column-7 `*`/`/` indicator; `*>` is recognized anywhere.
- **SAS statement comments.** `* ... ;` is modeled as a region comment, and
  `/* ... */` remains a block comment.
- Added **COBOL** and **SAS**, bringing the registry to **132 languages**.
- 7 new Phase 2b tests (51 library tests total: byte parsing, C++ raw strings,
  Ruby/Perl heredocs, Shell spaced heredoc, COBOL, SAS, Unicode safety).

Acceptance criteria met:

- `R"delim(// not a comment \" still)delim"` is code, including multi-line.
- Ruby `arr << x` and Perl `$x << 1` are not heredocs; `<<~SQL`/`<<EOF` are.
- Shell `cat << EOF` starts a heredoc.
- COBOL column-7 comments and `*>` inline comments classify correctly.
- SAS `* ... ;` comments span lines and end at `;`.
- Unicode content is classified without panics or delimiter false positives.
- Benchmark on the 1,900-file corpus (debug, 9 runs): Phase 2 median
  **12.9 s** → current median **9.85 s** (~24 % faster).

Known limitation:

- Ruby/Perl unquoted lowercase heredoc delimiters are not guessed; use an
  uppercase or quoted delimiter (`<<~'sql'`).

### Phase 4 — Embedded and template languages (implemented)

Delivered:

- **`EmbeddedKind` metadata.** HTML, XML, Vue, Svelte, Astro and Razor use the
  HTML splitter; Markdown uses the fence splitter.
- **HTML-like script/style splitting.** `<script>`/`<style>` regions use the
  embedded language’s comment and literal rules; tag-only lines use the host
  syntax. `lang`, `type`, self-closing tags and non-JavaScript script types are
  handled conservatively.
- **Markdown fenced code.** Backtick and tilde fences, up to three leading
  spaces, alias-based info strings and a neutral `Text` fallback.
- **Per-file regions.** `FileEntry.regions` carries `kind`, `language`,
  `start_line` and `end_line`; the CLI prints them under `--files` (per file)
  and `--verbose` (aggregated), and the GUI JSON export includes them.
- **`LineKind` / `LineStats::record`.** Shared line classification used by both
  the plain and embedded analyzers.
- **State isolation.** Embedded parser state is reset at every region boundary;
  no cross-region leakage of block comments/strings.

Acceptance criteria met:

- `<script>` blocks use JS/TS comment rules; `<style>` uses CSS.
- Vue `<script lang="ts">`, Stylus `<style lang="stylus">` and single-line
  `<script>code</script>` are classified correctly.
- Markdown fenced code is classified by its info string; unknown fences use
  the neutral syntax.
- Region boundaries are visible in `--files` and `--verbose` output and in the
  GUI JSON export.
- 14 new Phase 4 tests (65 library tests total), plus GUI JSON/region coverage.

Not in this phase: Jupyter `.ipynb` cell extraction, ERB/EJS, Haml/Slim, Blade,
MDX and other template-specific splitters. They remain future embedded-region
work.

### Phase 5 — User-defined languages

Scope: `codelines.toml` / `--languages FILE` schema, validation, GUI import and
“custom” badge. Built-ins remain immutable and take precedence.

Acceptance criteria:

- A user can add a language with extensions, filenames and rules without
  recompiling.
- Invalid rules fail with a path/field diagnostic.
- Export formats include custom languages unchanged.

---

## 7. Language matrix

Current registry (132). Phase 1–4 and Phase 2b groups are listed in section 6;
the table below focuses on languages that are **not** covered and what blocks
them. These are future embedded-region extensions, not part of the implemented
Phase 4 scope.

| Language | Extensions | Blocker | Target |
|----------|------------|---------|--------|
| Jupyter | `.ipynb` | JSON container, per-cell extraction | future Phase 4 extension |
| ERB/EJS | `.erb`, `.ejs` | `<%# %>` regions + embedded Ruby/JS | future Phase 4 extension |
| Haml/Slim | `.haml`, `.slim` | indentation-based templates | future Phase 4 extension |
| Blade | `*.blade.php` | nested extension + PHP/HTML regions | future Phase 4 extension |
| MDX | `.mdx` | Markdown + JSX regions | future Phase 4 extension |

Rules of thumb for future additions:

- One comment syntax and one string syntax → Phase 1 style entry.
- Multiple/nested comments or unusual literals → Phase 2 style entry.
- Needs a filename, shebang, or ambiguity hint → Phase 3.
- Contains another language → extend the Phase 4 region splitter.
- User-specific or proprietary → Phase 5.

---

## 8. Testing strategy

1. **Registry invariants** (implemented): unique names, unique extensions,
   complete block pairs, at least one comment syntax (legacy or detailed rules).
2. **Rule-shape fixtures** (implemented): PowerShell `<# #>`, CMake `#[[ ]]`,
   CoffeeScript `###`, Smalltalk `" "`, VB `'`, WAT `(; ;)`.
3. **Phase 2 analyzer fixtures** (implemented): Rust lifetimes/raw strings/nested
   blocks, Vim `"`, Smalltalk `"..."`, Shell `${#var}`/heredoc, Haskell `-->`,
   Batch `REM`, SQL strings, char literals, multiple block syntaxes, Lua long
   comments/strings, PowerShell here-strings and Perl POD.
4. **Phase 2b fixtures** (implemented): C++ raw strings with custom delimiters,
   Ruby/Perl heredocs vs `<<`, Shell `<< EOF`, COBOL column 7, SAS `* ... ;`
   and Unicode safety.
5. **Phase 3 detection fixtures** (implemented): filename matches and prefixes,
   shebang variants (`env`, `-S`, `-u NAME`, version suffixes), every ambiguous
   extension hint, preference fallback, metadata uniqueness, end-to-end
   detection and `--lang` interaction.
6. **Per-language golden fixtures** (Phase 3b): one small file per language with
   a comment-only line, a code-only line, a mixed line, a blank line, and a
   literal containing a comment marker. Table-driven, so a new language without
   fixtures fails CI.
7. **Embedded fixtures** (Phase 4, implemented): HTML with
   `<script>`/`<style>`, Vue SFC with `lang="ts"`, single-line script, JSON
   script exclusion, scripts inside HTML comments, Markdown backtick/tilde
   fences, unknown fences, marker mismatch, unclosed fences and state reset
   between regions.
8. **Invariants**: `code + comment + mixed + blank == total` for every analyzed
   file in the fixture corpus.
9. **Performance**: scan a fixed corpus before/after; fail on > 5 % regression.
   Keep `cargo clippy --workspace --all-targets` clean.

---

## 9. Performance notes

- Phase 2 compiles rules once per language in `report::analyze`; files no longer
  rebuild matcher tables.
- `starts_with` no longer allocates a `Vec<char>` per comparison. On a
  1,900-file corpus this reduced a debug-build scan from 15.1 s to 10.3 s.
- Phase 3 builds the registry once with `OnceLock`. `languages()` is a static
  slice, and the GUI no longer allocates 132 definitions per frame.
- Per-file detection uses registry index slices; only shared extensions and
  extensionless files allocate (a candidate `Vec` and an 8 KiB sample).
- Phase 2b scans `&[u8]` and no longer allocates a `Vec<char>` per line.
- Phase 4 only adds the region splitter for files whose `Language.embedded`
  is set; ordinary languages take the existing path. Embedded syntaxes and
  parser states are cached per file, and fence/tag scanning is byte-local.
- Benchmark on the 1,900-file corpus (debug builds): Phase 2 median
  **12.9 s** → Phase 2b median **9.85 s** (~24 % faster). Phase 4 vs Phase 3
  (5 alternating runs each): median **16.46 s → 16.25 s** (~1 % faster, within
  noise; no regression).
- The analyzer remains single-threaded per file; the GUI already runs it on a
  worker thread. File-level parallelism is out of scope for now.

---

## 10. Backward compatibility

- Existing display names and CLI `--lang` values are stable.
- `supported_languages()` keeps its signature; `languages()` is the new cached
  slice API.
- `CommentRule` keeps its legacy fields; the Phase 2/2b rule sets default to
  empty and are merged by `CompiledSyntax`. `heredocs` remains
  `Vec<&'static str>`; richer heredocs use `heredoc_rules`, and C++ raw strings
  use the additive `raw_strings` field. Existing literals only need
  `..Default::default()`.
- CSV export columns are unchanged. The GUI JSON export adds an additive
  `regions` array per file; existing fields keep their names and meanings.
- Statistics definitions (`effective = code + mixed`, `comment = comment +
  mixed`) do not change.
- New languages are additive; the only removal is the unreachable `.R`
  duplicate, which no user could observe because extensions are lowercased.

---

## 11. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Rule schema becomes a regex engine | Keep rule kinds closed and enumerable; no user regexes in built-ins |
| More languages, more false positives | Conservative classification; per-language fixtures; diagnostics |
| Embedded parsing balloons | Region splitting only, no host-language parsing; opt-in verbose diagnostics |
| User config breaks built-ins | Built-ins take precedence; config validated before merge |
| Performance regression | Static registry, compiled matchers, benchmark gate |
| Ambiguous extensions surprise users | Preference table + diagnostics + `--lang` override |

---

## 12. Open questions

1. Should docstrings count as comments by default for Python-like languages?
   Current default is “code”; keeping it preserves comparability with older
   reports.
2. Should `--list` group languages by family instead of one alphabetical block?
3. Do we need a `codelines.toml` in the scanned project (auto-discovered), or
   only an explicit `--languages FILE` flag?
4. ~~Should embedded-region lines appear as separate languages in the summary,
   or stay aggregated under the host file?~~ Resolved: embedded lines stay
   aggregated under the host file; regions are exposed per file instead.
5. Is a content-hint disambiguation for `.m`/`.v` acceptable, or should those
   extensions require `--lang` when ambiguous?
