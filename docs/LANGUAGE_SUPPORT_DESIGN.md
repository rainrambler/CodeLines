# CodeLines Language Support — Design

Status: **Phases 1–2 implemented** (122 languages). Phases 3–5 are proposed here
and not yet implemented.

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
                                                       │
                                                       ▼
                        language::supported_languages() ──► CommentRule
                                                       │
                                                       ▼
                                    analyzer::CompiledSyntax (once per language)
                                                       │
                                                       ▼
              analyzer::analyze_content_compiled(&str, &CompiledSyntax) ──► LineStats
                                                       │
                                                       ▼
                                   output::build_summaries ──► Report ──► CLI / GUI
```

Key properties:

- `Language` holds a display name, extensions and a `CommentRule`.
- `CommentRule` keeps the legacy fields (one line-prefix list, one block pair,
  string delimiters) and adds detailed `line`, `blocks`, `strings`, `chars`,
  `regions` and `heredocs` rule sets.
- `CompiledSyntax` merges legacy and detailed rules, sorts delimiters longest
  first, and is built once per language by `report::analyze`.
- `analyze_content_compiled` keeps an explicit `Mode` across lines: normal,
  block (with nesting depth), string, region or heredoc.
- `scanner` builds `extension → language name` once per scan; the GUI calls
  `supported_languages()` again every frame to render the language filter.

Phase 1 extended the registry only. Phase 2 keeps the same pipeline but replaces
the analyzer internals (compiled rules, explicit mode, detailed rule sets). The
remaining sections describe the target architecture that later phases need.

---

## 3. Capability gaps

The table records the gaps that motivated Phases 1–3. **Phase 2 closed gaps
1–8, 12 and the analyzer half of 14**; gaps 9–11 and 13 remain open.

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
| 14 | Registry rebuilt per call | Allocations in hot paths | `supported_languages()` builds 122 entries each call; GUI calls it per frame |

Two smaller issues already affect existing languages:

- `.R` was registered next to `.r`; because the scanner lowercases extensions,
  only `.r` was reachable. Phase 1 removes the redundant entry and adds a test
  that rejects case-insensitive duplicate extensions.
- `extension_map()` and the scanner’s `ext_to_lang` resolve duplicates
  differently (first match vs last insert). A uniqueness guard removes the
  ambiguity.

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

## 5. Rule model (Phase 2 implemented)

### 5.1 Registry

The block below is the Phase 3 target. The current `Language` struct still has
only `name`, `extensions` and `comment_rule`.

```rust
pub struct Language {
    pub name: &'static str,
    /// Command-line / GUI aliases (case-insensitive): "js", "ts", "c++", ...
    pub aliases: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub filenames: &'static [&'static str],
    pub shebangs: &'static [&'static str],
    pub rules: &'static [Rule],
    pub options: LanguageOptions,
}
```

The `OnceLock` registry and the filename/shebang fields are still Phase 3 work.
Phase 2 keeps the existing `supported_languages() -> Vec<Language>` API, but
`CompiledSyntax` is built once per language by `report::analyze`, so rules are
not recompiled for every file.

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
- Ruby: `#`, `=begin`/`=end` at column 0 (`%q` literals remain Phase 2b)
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
- **Column tracking** on the raw line enables `StartOfLine`, `FirstNonSpace`
  and `AloneOnLine` rules.
- **Token boundaries** (`BoundaryAfter::Word`/`Symbol`, optional
  `require_whitespace_before`) fix Shell `${#var}`, Haskell `-->`, Batch `REM`
  and Vim `"` false positives.
- **Multiple block rules** per language (Pascal, D, Lua, Ruby).
- **Nested block comments** for Rust, Kotlin, Scala, Swift, D, Nim, OCaml,
  F#, Haskell-family languages and others.
- **Raw/verbatim strings**: Rust `r#"..."#`, C++ `R"(...)"` (empty delimiter),
  C# `@"..."`/`"""`, Go backticks, Nix `''...''`, Lua long strings.
- **Character literals** distinct from strings, with a look-ahead length limit,
  so Rust/OCaml/F# type variables and Ada/VHDL/Verilog attributes stay code.
- **Doubled-quote escapes** for SQL, Smalltalk, Vim, Pascal, VB, PowerShell.
- **Heredocs** (`<<`, `<<-`, `<<~`, `<<<`) for Shell and PHP; the body is code.
- **PowerShell here-strings** `@"..."@` / `@'...'@` with start-of-line closers.
- **Line-oriented regions** for Ruby `=begin`/`=end` and Perl POD.

Still deferred:

- Byte-based fast path for ASCII delimiters (the parser still builds a
  `Vec<char>` per line).
- Ruby/Perl heredocs (ambiguous with the `<<` operator) and C++ raw strings
  with custom delimiters.
- `AloneOnLine` is implemented but only used by MATLAB, which is still blocked
  on the `.m` ambiguity.
- A line that continues a string/region counts as code, matching earlier
  behavior.

### 5.4 Detection pipeline

```
path
 ├─ 1. exact filename match        → Makefile, Dockerfile, CMakeLists.txt, BUILD
 ├─ 2. extension match (lowercase) → primary lookup
 ├─ 3. ambiguity resolution       → content hints, then preference table
 └─ 4. shebang match              → `#!/usr/bin/env python3`, `#!/bin/bash`
```

Ambiguous extensions get an ordered candidate list plus cheap hints:

| Extension | Candidates | Hint |
|-----------|------------|------|
| `.m` | Objective-C, MATLAB, Mathematica | `@interface` / `function`+`%` / `(*` |
| `.v` | Verilog, V, Coq | `module`/`endmodule`, `fn `, `Theorem` |
| `.pl` | Perl, Prolog | `:-`, `use strict` |
| `.fs` | F#, Forth | `let `, `: ` |
| `.cl` | OpenCL, Common Lisp | `__kernel`, `(defun` |
| `.cls` | Apex, TeX/LaTeX | `@isTest`/`global class`, `\NeedsTeXFormat` |
| `.pp` | Puppet, Free Pascal | `class `, `program ` |
| `.pro` | QMake, Prolog | `TEMPLATE`, `:-` |

Hinting stays intentionally small: a few substring checks, no parsing. When no
hint is conclusive, use the preference table and record a diagnostic so users
can see which language won.

### 5.5 Embedded regions

Phase 4 handles mixed-content files by splitting a file into regions and
classifying each region with its own rule set:

| Host | Regions |
|------|---------|
| HTML/XML | markup + `<!-- -->`; `<script>` → JS/TS; `<style>` → CSS |
| Vue/Svelte/Astro | `<template>` → markup; `<script>` → JS/TS; `<style>` → CSS |
| JSX/TSX | JS/TS rules already cover comments; tag markup uses JS literals |
| Markdown | prose; fenced blocks → language by info string; inline code ignored |
| Razor/Blazor | markup + `@* *@` + C# blocks |

The region splitter is a small scanner that recognizes host delimiters only; it
does not validate nesting. A line that straddles a region boundary is counted
with the region that starts it, and the split is reported in verbose diagnostics.

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
- Filename-only languages (`Makefile`, `Dockerfile`) are still unknown (Phase 3).

Acceptance criteria met (at the end of Phase 1):

- `cargo test -p codelines --lib` passes.
- `cargo run -- --list` shows 120 entries (122 after Phase 2).
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
- 15 new analyzer tests plus registry wiring tests (39 library tests total)
  covering every acceptance case.

Acceptance criteria met:

- Rust `&'a str` no longer opens a string.
- Vim `" comment` is a comment; Smalltalk `"comment"` is a comment.
- Shell `${#var}`, Haskell `-->` and Batch `REM` classify correctly.
- Nested `/* /* */ */` in Rust/Kotlin/Swift is fully covered.
- Self-scan and workspace tests pass.
- Benchmark on a 1,900-file corpus (debug builds, median of 5 runs):
  **15.1 s before → 10.3 s after**. No regression; removing the per-comparison
  `Vec<char>` allocation more than offsets the extra rules.

Remaining Phase 2b work:

- Ruby/Perl heredocs (ambiguous with `<<`) and C++ raw strings with custom
  delimiters.
- Heredoc delimiters must follow the marker directly and start with a letter or
  underscore (`<<EOF`, not `<< EOF`); this avoids false positives on the `<<`
  operator.
- Byte-based matcher and a `OnceLock` registry to remove per-call allocation.
- MATLAB `%{`/`%}` (blocked on `.m` ambiguity) and COBOL fixed-column rules.

### Phase 3 — File-name and ambiguity detection

Scope: filename table, shebang parsing, candidate lists for ambiguous
extensions, diagnostics for unresolved ambiguity.

Acceptance criteria:

- `Makefile`, `Dockerfile` and `CMakeLists.txt` are recognized.
- `#!/usr/bin/env python3` and `#!/bin/bash` files without extensions count.
- `.m`/`.v`/`.pl` files resolve by hint or documented preference.
- Unknown extensions still appear in diagnostics.

### Phase 4 — Embedded and template languages

Scope: region splitting for HTML/XML, Vue/Svelte/Astro, Razor, Markdown fences.

Acceptance criteria:

- `<script>` blocks use JS/TS comment rules; `<style>` uses CSS.
- Markdown fenced code is classified by its info string.
- Region boundaries are visible in verbose output.

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

Current registry (122). Phase 1 and Phase 2 groups are listed in section 6; the
table below focuses on languages that are **not** covered and what blocks them.

| Language | Extensions | Blocker | Target phase |
|----------|------------|---------|--------------|
| Makefile | `Makefile`, `*.mk` | filename + tab-sensitive recipes | 3 (rule ready) |
| Dockerfile | `Dockerfile`, `*.dockerfile` | filename | 3 |
| Objective-C | `.m`, `.mm` | `.m` ambiguity | 3 |
| MATLAB | `.m` | `.m` ambiguity, `%{ %}` alone-on-line blocks | 3 |
| COBOL | `.cob`, `.cbl`, `.cpy` | fixed-column `*` indicator | 2b |
| SAS | `.sas` | `* ... ;` region comment | 2b |
| Ruby/Perl heredoc | `<<EOF` | ambiguous with the `<<` operator | 2b |
| C++ delimited raw strings | `R"delim(...)delim"` | only the empty delimiter is modeled | 2b |
| Common Lisp (`*.cl`) | `.cl` | `.cl` assigned to OpenCL; use `.lisp`/`.lsp` | 3 (hint) |
| Prolog | `.pl`, `.pro` | extension conflicts (Perl, QMake); `.prolog` is covered | 3 |
| LaTeX class files | `.cls` | `.cls` assigned to Apex | 3 (hint) |
| V language | `.v` | `.v` assigned to Verilog | 3 (hint) |
| Coq | `.v` | `.v` assigned to Verilog | 3 (hint) |
| Forth | `.fs`, `.fth` | `.fs` assigned to F# | 3 (hint) |
| Jupyter | `.ipynb` | JSON container, per-cell extraction | 4 |
| ERB/EJS | `.erb`, `.ejs` | `<%# %>` regions + embedded Ruby/JS | 4 |
| Haml/Slim | `.haml`, `.slim` | indentation-based templates | 4 |
| Blade | `*.blade.php` | nested extension + PHP/HTML regions | 4 |
| MDX | `.mdx` | Markdown + JSX regions | 4 |

Rules of thumb for future additions:

- One comment syntax and one string syntax → Phase 1 style entry.
- Multiple/nested comments or unusual literals → Phase 2 style entry.
- Needs a filename, shebang, or ambiguity hint → Phase 3.
- Contains another language → Phase 4.
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
4. **Per-language golden fixtures** (Phase 3): one small file per language with
   a comment-only line, a code-only line, a mixed line, a blank line, and a
   literal containing a comment marker. Table-driven, so a new language without
   fixtures fails CI.
5. **Ambiguity fixtures** (Phase 3): `.m`, `.v`, `.pl`, `.fs`, extensionless
   files.
6. **Embedded fixtures** (Phase 4): HTML with `<script>`/`<style>`, Vue SFC,
   Markdown fences.
7. **Invariants**: `code + comment + mixed + blank == total` for every analyzed
   file in the fixture corpus.
8. **Performance**: scan a fixed corpus before/after; fail on > 5 % regression.
   Keep `cargo clippy --workspace --all-targets` clean.

---

## 9. Performance notes

- Phase 2 compiles rules once per language in `report::analyze`; files no longer
  rebuild matcher tables.
- `starts_with` no longer allocates a `Vec<char>` per comparison. On a
  1,900-file corpus this reduced a debug-build scan from 15.1 s to 10.3 s.
- The registry is still rebuilt by `supported_languages()`, and the GUI still
  calls it per frame. Moving it to `OnceLock` is Phase 2b and removes that cost.
- The parser still builds one `Vec<char>` per line. A byte-based ASCII fast path
  is Phase 2b.
- The analyzer remains single-threaded per file; the GUI already runs it on a
  worker thread. File-level parallelism is out of scope until the registry is
  static and allocation-free.

---

## 10. Backward compatibility

- Existing display names and CLI `--lang` values are stable.
- `supported_languages()` keeps its signature.
- `CommentRule` keeps its legacy fields; the Phase 2 rule sets default to empty
  and are merged by `CompiledSyntax`. Existing literals only need
  `..Default::default()`.
- CSV/JSON export schemas do not change.
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
4. Should embedded-region lines appear as separate languages in the summary, or
   stay aggregated under the host file?
5. Is a content-hint disambiguation for `.m`/`.v` acceptable, or should those
   extensions require `--lang` when ambiguous?
