# CodeLines Desktop GUI — Design

Status: implemented (feature `gui`).

The desktop front-end shares the exact analysis engine with the CLI and adds a
visual, exploratory layer on top of it: pick a folder, watch the scan progress,
compare languages, drill into individual files and export the result.

All user-facing text in the GUI is **English**.

---

## 1. Goals and non-goals

### Goals

| # | Goal | How it is met |
|---|------|---------------|
| 1 | Zero duplication of analysis logic | One `codelines` library crate; CLI and GUI both call `report::analyze` |
| 2 | Never block the window | Analysis runs on a worker thread; UI only renders snapshots |
| 3 | Instant feedback on big trees | Streamed progress, throttled messages, virtualized file table |
| 4 | Understand the result at a glance | KPI cards, stacked bars, language-share donut, sortable tables |
| 5 | Keep the CLI untouched | `gui` is an optional Cargo feature; the `codelines` binary does not link egui |
| 6 | Ship a real artifact | CSV / JSON / plain-text export plus clipboard copy |

### Non-goals

* Editing source files.
* A code browser or syntax highlighting.
* Remote or multi-project (multi-root) analysis — the GUI analyzes one root at a time.
* Persisting window state or preferences across launches.

---

## 2. Architecture

### 2.1 Crate layout

Before this change the whole tool was a binary crate (`main.rs` + `mod` files).
The GUI needs the same engine, so the code was split into a library plus two
binaries:

```
src/
├── lib.rs            # public API: analyzer, cli, language, output, report, scanner, gui
├── main.rs           # `codelines` CLI binary (thin argument parsing + rendering)
├── bin/
│   └── codelines-gui.rs   # `codelines-gui` binary, compiled only with --features gui
├── analyzer.rs       # line classification (code / comment / mixed / blank)
├── language.rs       # 33 language definitions and comment rules
├── scanner.rs        # recursive walk, ignore lists, extension → language mapping
├── output.rs         # summary aggregation and CLI formatters (colored / plain / CSV)
├── report.rs         # shared pipeline: scan → analyze → Report (+ progress, cancel)
├── cli.rs            # clap argument definition
└── gui/              # (feature `gui`)
    ├── mod.rs        # run(): window setup
    ├── app.rs        # application state, layout, toolbar, side panel, tabs
    ├── worker.rs     # background analysis thread + mpsc channel
    ├── charts.rs     # painted stacked bars and donut chart
    ├── table.rs      # sortable virtualized tables
    ├── widgets.rs    # KPI cards, legends, number/duration formatting
    ├── theme.rs      # palette, spacing, dark/light visuals
    └── export.rs     # CSV / JSON / text export, save dialog, reveal in file manager
```

### 2.2 Dependency direction

```
             ┌──────────────┐        ┌──────────────────┐
             │  codelines   │        │  codelines-gui   │
             │  (CLI bin)   │        │     (bin)        │
             └──────┬───────┘        └────────┬─────────┘
                    │                         │
                    └──────────┬──────────────┘
                               ▼
                     ┌───────────────────┐
                     │  codelines (lib)  │
                     │  report / scanner │
                     │  analyzer / …     │
                     └───────────────────┘
                               ▲
                     ┌───────────────────┐
                     │   gui modules     │  (feature = "gui")
                     └───────────────────┘
```

`report::analyze` is the single entry point for both front-ends:

```rust
pub fn analyze(
    options: &ScanOptions,
    cancel: Option<&AtomicBool>,
    progress: impl FnMut(Progress),
) -> Report;
```

### 2.3 Cargo features

```toml
[features]
default = []
gui = ["dep:eframe", "dep:egui_extras", "dep:rfd"]
```

`cargo build` / `cargo install` keeps producing the lean CLI. Only
`cargo build --features gui` pulls in `eframe` (egui + glow renderer),
`egui_extras` (tables) and `rfd` (native open/save dialogs).

---

## 3. Concurrency model

```
UI thread (egui frame loop)                  Worker thread
──────────────────────────                   ─────────────
start_analysis()
   └─ AnalysisWorker::spawn ────────────────► report::analyze(options, cancel, |p| …)
                                                │  Progress::Scanning   ─┐
poll_worker()  ◄────── mpsc channel ◄──────────┤  Progress::Analyzing  ─┤ throttled
   updates status/progress bar                  │                       │ to ~50 ms
                                               └─ WorkerMsg::Finished ──┘
                                                   → ctx.request_repaint()
```

* **One worker per run.** Starting a new analysis while one is running is
  disabled; `Cancel` (or `Esc`) sets an `AtomicBool` that the pipeline checks
  between files, returning a partial `Report` with `cancelled = true`.
* **No shared mutable state.** The worker owns `ScanOptions`, sends owned
  `Progress` / `Report` values, and never touches app state.
* **Repaint-driven.** The worker calls `Context::request_repaint()` after every
  message, so the UI wakes only when there is news. While a run is active the
  UI also requests a repaint every 100 ms so the spinner keeps animating.
* **Throttling.** `report::analyze` emits progress at most every 50 ms; the
  channel therefore carries a few hundred messages even for 50 000 files.

---

## 4. Information architecture

Three views, one toolbar, one filter rail, one status bar.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ CodeLines │ [ project folder ................................ ] Browse Analyze│
│                                              Hide filters  Export  Light     │
├───────────────┬─────────────────────────────────────────────────────────────┤
│ SCAN SCOPE    │ Overview   Files (19)   Diagnostics (1)      19 files · 4,515│
│ Exclude dirs  │ ┌──────────┐┌──────────┐┌──────────┐┌──────────┐┌──────────┐ │
│ [ vendor   ]  │ │ Source   ││ Total    ││ Effective││ Comments ││ Mixed    │ │
│               │ │ files 19 ││ lines    ││ code     ││ 257      ││ 20       │ │
│ Exclude files │ └──────────┘└──────────┘└──────────┘└──────────┘└──────────┘ │
│ [ Cargo.lock] │ ┌─────────────────────────────┐ ┌──────────────────────────┐│
│               │ │ Lines by language           │ │ Language share           ││
│ LANGUAGES     │ │ Rust  ████████████░░  4,392 │ │      ╭──────╮            ││
│ [ search…   ] │ │ Py    ▎                89   │ │     │ 3,848  │  ● Rust    ││
│ ☑ Rust        │ │ TOML  ▎                34   │ │      ╰──────╯  ● Markdown ││
│ ☑ C/C++       │ │ ● Code ● Comments ● Mixed … │ │                          ││
│ ☑ Java        │ └─────────────────────────────┘ └──────────────────────────┘│
│ …             │ ┌──────────────────────────────────────────────────────────┐│
│               │ │ Languages  3 detected                                     ││
│               │ │ Language   Files   Total   Code ▼  Comment  Mixed  …      ││
├───────────────┴─────────────────────────────────────────────────────────────┤
│ ● 3 languages · 19 files · 4,515 lines · analyzed in 33 ms          v1.0.1  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Empty state

Shown when no report exists and no run is active: product name, one-line pitch,
primary `Open Project Folder…` button, drag-and-drop hint, and a keyboard
shortcut card. Errors (missing path, unreadable folder) are shown in red in the
same place, so the user never has to look for a dialog.

### 4.2 Overview tab

| Region | Content |
|--------|---------|
| KPI cards | Source files, Total lines, Effective code, Comments, Mixed lines, Blank lines — each with a percentage or secondary value |
| Lines by language | Stacked horizontal bars per language (code / comment / mixed / blank), top 7 by default with an `Other (n languages)` roll-up and a `Show all` toggle |
| Language share | Donut of effective code per language, top 6 + `Other`, with a percentage legend |
| Languages table | Every language with files, total, code, comment, mixed, blank, code %, comment % |

Choosing *effective code* (`code + mixed`) as the donut metric keeps the chart
consistent with the CLI, where mixed lines count as code.

### 4.3 Files tab

* Filter box matching path or language (case-insensitive substring).
* `Export CSV` button for this view.
* Table columns: Path (relative to root), Language, Total, Code, Comment, Mixed,
  Blank, Code %. Every column is sortable.
* Single click selects a row; double click reveals the file in Finder / Explorer
  / the desktop file manager.
* Rows are virtualized (`TableBody::rows`), so 50 000 files scroll at the same
  cost as 50.

### 4.4 Diagnostics tab

* Scan summary: roots, files visited, analyzed files, elapsed time, cancelled
  flag, warning count.
* Skipped file types with occurrence counts.
* Warning list (missing paths, unreadable files).

### 4.5 Status bar

Always visible: a colored state dot plus either
`3 languages · 19 files · 4,515 lines · analyzed in 33 ms`,
the live progress (`Analyzing source files… [15 / 47,533] bindings.cpp`), or the
cancellation/error message. The right side shows the app version.

---

## 5. Interaction model

| Action | Trigger |
|--------|---------|
| Open folder | `Browse…`, `Cmd/Ctrl + O`, `Open Project Folder…`, or drag & drop onto the window |
| Analyze | `Analyze`, `Enter` in the path field, `Cmd/Ctrl + R`, `F5` |
| Cancel | `Cancel`, `Esc` |
| Export | `Export` menu, `Cmd/Ctrl + E` (summary CSV) |
| Switch theme | `Light` / `Dark` button |
| Show/hide filters | `Hide filters` / `Show filters` |
| Sort a table | Click a column header (click again to reverse) |
| Reveal a file | Double click a row in the Files tab |

The filter rail edits are *inputs* to the next run: changing exclusions,
language selection or the root and pressing `Analyze` re-runs the pipeline. The
current run is never mutated underneath the user.

Language filtering uses an empty set as "all languages". Unchecking a language
from the "all" state materializes the full set minus that language; re-checking
everything collapses back to "all", and a `Reset` button clears the filter.

---

## 6. Visual design

### 6.1 Palette

| Role | Dark | Light |
|------|------|-------|
| Window / panel | `#14161B` | `#F4F6FA` |
| Card | `#1E222B` | `#FFFFFF` |
| Card border | `#2A303B` | `#E2E6EE` |
| Accent (interactive, focus, primary button) | `#4F9CF7` | `#4F9CF7` |
| Code | `#4F9CF7` | `#4F9CF7` |
| Comments | `#3FC18F` | `#3FC18F` |
| Mixed | `#F2B541` | `#F2B541` |
| Blank | `#5D687A` | `#5D687A` |
| Error | `#E56B6F` | `#E56B6F` |

Language series use a fixed 12-color palette; `Other` is always neutral gray so
aggregation stays visually distinct from a real language.

Category colors are reused everywhere (KPI cards, bars, legends, tables), so a
color always means the same thing.

### 6.2 Typography and spacing

* Proportional font for UI text (13 px body), monospace for numbers and paths so
  columns align.
* KPI value 22 px semi-bold, KPI label 11 px, hints 10 px.
* 8 px vertical rhythm, 12 px card padding, 12 px corner radius on cards,
  7 px on controls, 10 px gap between cards.
* Tables use 24 px rows with alternating stripes; headers are 26 px,
  10.5 px semi-bold.

### 6.3 Charts

* **Stacked bar**: pill-shaped track, proportional segments, hover tooltip with
  per-category counts and shares. Bars are scaled against the largest language so
  small languages stay visible but comparable.
* **Donut**: painted wedges (center + arc polygon) with a hole in the card color,
  center label showing "effective code" and the total; hovering a wedge opens a
  tooltip with name, lines and share; legend rows show percentages.

Both are drawn with the egui painter — no extra plotting dependency.

---

## 7. Data model

```rust
pub struct ScanOptions {            // input
    pub paths: Vec<PathBuf>,
    pub languages: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub exclude_files: Vec<String>,
}

pub enum Progress {                 // streaming
    Scanning  { files_seen: usize, current: Option<PathBuf> },
    Analyzing { done: usize, total: usize, current: Option<PathBuf> },
}

pub struct Report {                 // result
    pub summaries: Vec<LanguageSummary>,   // sorted by effective code
    pub files: Vec<FileEntry>,             // path, language, LineStats
    pub total_files_scanned: usize,
    pub unknown_extensions: Vec<(String, usize)>,
    pub warnings: Vec<String>,
    pub elapsed: Duration,
    pub roots: Vec<PathBuf>,
    pub cancelled: bool,
}
```

`Report` is owned by the UI thread; the Files table caches a filtered/sorted index
list keyed by `(query, sort key, direction, file count)` so it is rebuilt only
when one of those changes.

---

## 8. Export formats

| Format | Content | Notes |
|--------|---------|-------|
| `codelines-summary.csv` | Language summaries | Byte-identical to `codelines --csv` (shared formatter) |
| `codelines-files.csv` | Per-file rows | Path and language are CSV-quoted when needed |
| `codelines-report.txt` | Plain table + metadata + per-file sections | Same layout as `codelines --plain --files` |
| `codelines-report.json` | totals, languages, files, unknown extensions, warnings | Hand-written JSON with full string escaping, stable key order |

A save dialog is provided by `rfd`; failures surface as a toast.

---

## 9. Performance

| Concern | Strategy |
|---------|----------|
| Huge trees (50 000+ files) | Analysis on a worker thread, progress throttled to 50 ms |
| File table rendering | Virtualized rows; only visible rows build strings |
| Filtering | Cached index vector, rebuilt on query/sort change |
| Chart rendering | Aggregated to top 7/6 rows + `Other` |
| Re-render cost | No per-frame allocation of the report; charts paint primitives only |
| Memory | One `FileEntry` per file (path + 5 counters) |

---

## 10. Accessibility, i18n, platform

* All strings are English and centralized in the layout code; no hardcoded
  locale-specific formatting. Numbers use thousands separators, percentages one
  decimal.
* egui provides keyboard focus traversal for the toolbar and filter rail; every
  interactive element has a tooltip or a visible label.
* Theme can be switched at runtime; the window follows the explicit choice rather
  than the OS theme, so screenshots and demos are reproducible.
* Native dialogs come from `rfd`; "reveal file" uses `open -R` (macOS),
  `explorer /select,` (Windows) and `xdg-open` (Linux).

---

## 11. Build, run, test

```bash
# CLI (unchanged)
cargo build --release
./target/release/codelines src

# GUI
cargo run --release --features gui --bin codelines-gui
cargo run --release --features gui --bin codelines-gui -- /path/to/project

# Tests
cargo test                    # core pipeline
cargo test --features gui     # pipeline + export formats
```

Optional arguments: a path passed to `codelines-gui` pre-fills the root and
starts the analysis immediately.

---

## 12. Future work

1. **Watch mode** — re-analyze on file-system events, showing deltas since the
   last run.
2. **History and trends** — persist reports and plot lines over time.
3. **Diff view** — compare two reports (branches, commits, before/after).
4. **Multi-root workspaces** — analyze several folders into one report.
5. **Configurable rules** — user-defined languages and comment rules, editable in
   the GUI.
6. **Saved preferences** — remember window size, theme, exclusions and last root.
