# CodeLines Language Support — Design

Status: **Phase 1 implemented** (120 languages). Phases 2–5 are proposed here and
not yet implemented.

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
                        analyzer::analyze_content(&str, &CommentRule) ──► LineStats
                                                       │
                                                       ▼
                                   output::build_summaries ──► Report ──► CLI / GUI
```

Key properties:

- `Language` holds a display name, extensions and a `CommentRule`.
- `CommentRule` holds **one** list of line-comment prefixes, **at most one**
  block-comment pair, and a list of string delimiters.
- `analyze_content` keeps only two pieces of cross-line state: “inside a block
  comment” and “inside a multi-line string”.
- `scanner` builds `extension → language name` once per scan; the GUI calls
  `supported_languages()` again every frame to render the language filter.

Phase 1 preserves this architecture and extends the registry only. The
remaining sections describe the target architecture that later phases need.

---

## 3. Capability gaps

The current rule model cannot express several common language features. Each gap
below is a concrete correctness bug or a hard blocker for a new language.

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
| 14 | Registry rebuilt per call | Allocations in hot paths | `supported_languages()` builds 120 entries each call; GUI calls it per frame |

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

## 5. Target rule model

### 5.1 Registry

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

- Built-ins live in `language.rs` as `const` data (no per-call allocation).
- `registry()` returns `&'static Registry` built once with `OnceLock`, including
  the extension/filename/language lookup maps.
- `supported_languages()` remains as a compatibility shim over `registry()`.
- User-defined languages (Phase 5) are merged into an owned registry at startup;
  built-in names win on conflict.

### 5.2 Rule schema

```rust
pub struct Rule {
    pub kind: RuleKind,
    pub nesting: Nesting,
    pub scope: Scope,
}

pub enum RuleKind {
    Line { prefix: &'static str, case_insensitive: bool, boundary: Boundary },
    Block { start: &'static str, end: &'static str },
    Region { open: RegionOpen, close: RegionClose }, // heredocs, POD, templates
    String(StringRule),
}

pub struct StringRule {
    pub open: &'static str,
    pub close: &'static str,
    pub multiline: bool,
    pub escape: Escape,
    pub raw_prefixes: &'static [&'static str],
}

pub enum Escape { Backslash, Doubled, None }
pub enum Nesting { Flat, Nested }
pub enum Scope { Anywhere, LineStart, FirstNonSpace, AloneOnLine }
pub enum Boundary { None, NotFollowedBySymbol, PrecededByWhitespaceOrStart }
```

A language may have **many** rules in any order; precedence is explicit:
longest match first, then the order in the array. This replaces the current
fixed “strings → block → line” precedence and supports:

- Pascal: `//`, `{ }`, `(* *)`
- Lua: `--`, `--[[ ]]`, `--[==[ ]==]`
- Rust: `//`, `///`, `//!`, `/* */` (nested), `"`, `'` (char only), `r#"..."#`
- Vim: `"` comment first, `'` string
- Smalltalk: `"..."` block comment, `'...'` string
- Shell: `#` with boundary, `<<EOF` region
- Ruby: `#`, `=begin`/`=end` at column 0, `%q`-style literals
- Verilog: `//`, `/* */`, `"` strings, `'` allowed in numeric literals

### 5.3 Analyzer v2

Replace the two booleans with an explicit mode stack and a compiled matcher:

```rust
enum Mode<'a> {
    Normal,
    Block { end: &'a str, depth: usize },
    String { rule: &'a StringRule, hashed_delimiter: bool },
    Region { end: RegionEnd },
}

struct CompiledSyntax {
    rules: Vec<CompiledRule>,      // longest delimiter first
    line_starts: Vec<CompiledLine>, // column/token constraints
}
```

Loop per line:

1. If in a block/string/region mode, scan for that mode’s end (and nested
   starts when `Nesting::Nested`), then return to `Normal`.
2. In `Normal`, scan for the next match among compiled rules.
3. When a rule matches, record `has_comment` or `has_code` and push/pop modes.
4. Unmatched characters set `has_code`.

Important properties:

- **Compiled once per language**, not per file or per line. Delimiters are
  stored as `Vec<char>`/`Vec<u8>` and never re-collected in `starts_with`.
- **Byte-based fast path** for ASCII delimiters; fall back to `char` iteration
  only for non-ASCII text.
- **Column tracking** enables `LineStart`/`AloneOnLine` rules without
  post-processing.
- **Token boundaries** are evaluated on the raw slice (`#` not followed by `{`,
  `--` not followed by `>`), fixing shell/Vim/Haskell false positives.
- **Region mode** consumes whole lines until a terminator, which makes heredocs,
  PowerShell here-strings, Perl POD and template blocks cheap to support.
- A line that only contains a continuation of a string/region continues to
  count as code (current behavior, preserved deliberately).

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

Known Phase 1 limitations (accepted, documented):

- Nested block comments close at the first end marker.
- Raw/verbatim strings are not modeled.
- Languages with two block syntaxes use the dominant one (Pascal `{ }`, D `/* */`).
- `REM` in Visual Basic/Batch is not recognized.
- Filename-only languages (`Makefile`, `Dockerfile`) are still unknown.

Acceptance criteria met:

- `cargo test -p codelines --lib` passes.
- `cargo run -- --list` shows 120 entries.
- No duplicate extension in the registry.

### Phase 2 — Rule engine correctness

Scope: nested blocks, multiple rules per language, raw strings, escaped/doubled
quotes, character literals, token boundaries, column scopes, case-insensitive
prefixes, heredocs and POD.

Acceptance criteria:

- Rust `&'a str` no longer opens a string.
- Vim `" comment` is a comment; Smalltalk `"comment"` is a comment.
- Shell `${#var}`, Haskell `-->` and Batch `REM` classify correctly.
- Nested `/* /* */ */` in Rust/Kotlin/Swift is fully covered.
- No more than 5 % regression on the self-scan benchmark.

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

Current registry (120). Phase 1 groups are listed in section 6; the table below
focuses on languages that are **not** covered and what blocks them.

| Language | Extensions | Blocker | Target phase |
|----------|------------|---------|--------------|
| Makefile | `Makefile`, `*.mk` | filename + tab-sensitive recipes | 3 (filename), 2 (`#`) |
| Dockerfile | `Dockerfile`, `*.dockerfile` | filename | 3 |
| Batch | `.bat`, `.cmd` | case-insensitive `REM`, `::` label comment | 2 |
| Objective-C | `.m`, `.mm` | `.m` ambiguity | 2 + 3 |
| MATLAB | `.m` | `.m` ambiguity, `%{ %}` alone-on-line blocks | 2 + 3 |
| COBOL | `.cob`, `.cbl`, `.cpy` | fixed-column `*` indicator | 2 |
| Fortran fixed form | `.f`, `.for` | column-1 `C`/`*` indicator | 2 |
| SAS | `.sas` | `* ... ;` region comment | 2 |
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
- Multiple/nested comments or unusual literals → Phase 2.
- Needs a filename, shebang, or ambiguity hint → Phase 3.
- Contains another language → Phase 4.
- User-specific or proprietary → Phase 5.

---

## 8. Testing strategy

1. **Registry invariants** (implemented): unique names, unique extensions,
   complete block pairs, at least one comment syntax.
2. **Rule-shape fixtures** (implemented): PowerShell `<# #>`, CMake `#[[ ]]`,
   CoffeeScript `###`, Smalltalk `" "`, VB `'`, WAT `(; ;)`.
3. **Per-language golden fixtures** (Phase 2): one small file per language with
   a comment-only line, a code-only line, a mixed line, a blank line, and a
   literal containing a comment marker. Table-driven, so a new language without
   fixtures fails CI.
4. **Ambiguity fixtures** (Phase 3): `.m`, `.v`, `.pl`, `.fs`, extensionless
   files.
5. **Embedded fixtures** (Phase 4): HTML with `<script>`/`<style>`, Vue SFC,
   Markdown fences.
6. **Invariants**: `code + comment + mixed + blank == total` for every analyzed
   file in the fixture corpus.
7. **Performance**: scan the repository itself before/after; fail on > 5 %
   regression. Keep `cargo clippy --workspace --all-targets` clean.

---

## 9. Performance notes

- Phase 1 changes are data-only. The registry is still rebuilt per call; that is
  a known cost, not a regression.
- Phase 2 must move `supported_languages()` to a `OnceLock` registry and compile
  rules once. This also removes the GUI’s per-frame rebuild.
- Delimiter matching should avoid `Vec<char>` allocation per line and per
  `starts_with` call. Prefer `&[u8]` with an ASCII fast path.
- The analyzer remains single-threaded per file; the GUI already runs it on a
  worker thread. File-level parallelism is out of scope until the registry is
  static and allocation-free.

---

## 10. Backward compatibility

- Existing display names and CLI `--lang` values are stable.
- `supported_languages()` keeps its signature; a `&'static` registry is added
  alongside it.
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
