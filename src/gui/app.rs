//! Application state, layout and interaction.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, Layout, RichText, Ui, vec2};

use crate::language;
use crate::output::LanguageSummary;
use crate::report::{Progress, Report, ScanOptions};

use super::charts;
use super::export::{self, ExportKind};
use super::table::{self, FileSort, SortState, SummarySort};
use super::theme;
use super::widgets::{self, format_duration, format_number, format_percent};
use super::worker::{AnalysisWorker, WorkerMsg};

const SIDE_PANEL_WIDTH: f32 = 264.0;
const ERROR_COLOR: Color32 = Color32::from_rgb(0xE5, 0x6B, 0x6F);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Files,
    Diagnostics,
}

/// Side effects requested by the UI and applied after the frame is built.
enum UiAction {
    Browse,
    Analyze,
    Cancel,
    ToggleTheme,
    Export(ExportKind),
    CopyCsv,
    Reveal(PathBuf),
}

struct Toast {
    text: String,
    error: bool,
    created: Instant,
}

impl Toast {
    fn new(text: impl Into<String>, error: bool) -> Self {
        Self {
            text: text.into(),
            error,
            created: Instant::now(),
        }
    }
}

/// Purely presentational state (kept apart from the report data).
struct ViewState {
    tab: Tab,
    summary_sort: SortState<SummarySort>,
    file_sort: SortState<FileSort>,
    file_query: String,
    file_rows: Vec<usize>,
    file_rows_key: Option<(String, FileSort, bool, usize)>,
    selected_file: Option<usize>,
    show_all_languages: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            tab: Tab::Overview,
            summary_sort: SortState::default(),
            file_sort: SortState::default(),
            file_query: String::new(),
            file_rows: Vec::new(),
            file_rows_key: None,
            selected_file: None,
            show_all_languages: false,
        }
    }
}

impl ViewState {
    fn invalidate_file_rows(&mut self) {
        self.file_rows_key = None;
    }

    fn ensure_file_rows(&mut self, files: &[crate::report::FileEntry]) {
        let key = (
            self.file_query.clone(),
            self.file_sort.key,
            self.file_sort.ascending,
            files.len(),
        );
        if self.file_rows_key.as_ref() != Some(&key) {
            self.file_rows = table::filter_and_sort_files(files, &self.file_query, self.file_sort);
            self.file_rows_key = Some(key);
        }
    }
}

/// The CodeLines desktop application.
pub struct CodeLinesApp {
    dark: bool,
    show_filters: bool,
    root: String,
    exclude_dirs: String,
    exclude_files: String,
    selected_languages: BTreeSet<String>,
    language_search: String,
    total_languages: usize,
    view: ViewState,
    worker: Option<AnalysisWorker>,
    report: Option<Report>,
    progress: Option<Progress>,
    error: Option<String>,
    toast: Option<Toast>,
}

impl CodeLinesApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx, true);
        let mut app = Self {
            dark: true,
            show_filters: true,
            root: String::new(),
            exclude_dirs: String::new(),
            exclude_files: String::new(),
            selected_languages: BTreeSet::new(),
            language_search: String::new(),
            total_languages: language::supported_languages().len(),
            view: ViewState::default(),
            worker: None,
            report: None,
            progress: None,
            error: None,
            toast: None,
        };

        if let Some(argument) = std::env::args().nth(1) {
            let path = PathBuf::from(argument);
            if path.exists() {
                app.root = path.display().to_string();
                app.start_analysis(&cc.egui_ctx);
            }
        }

        app
    }

    fn start_analysis(&mut self, ctx: &egui::Context) {
        if self.worker.is_some() {
            return;
        }

        let root = self.root.trim().to_string();
        if root.is_empty() {
            self.error = Some("Choose a project folder first.".to_string());
            return;
        }

        let root_path = PathBuf::from(&root);
        if !root_path.exists() {
            self.error = Some(format!("Path does not exist: {root}"));
            return;
        }

        self.error = None;
        self.progress = None;
        self.view.invalidate_file_rows();
        self.view.selected_file = None;

        let options = ScanOptions {
            paths: vec![root_path],
            languages: self
                .selected_languages
                .iter()
                .cloned()
                .collect::<Vec<String>>(),
            exclude_dirs: split_list(&self.exclude_dirs),
            exclude_files: split_list(&self.exclude_files),
        };

        self.worker = Some(AnalysisWorker::spawn(ctx.clone(), options));
    }

    fn poll_worker(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };

        let mut finished = None;
        for message in worker.poll() {
            match message {
                WorkerMsg::Progress(progress) => self.progress = Some(progress),
                WorkerMsg::Finished(report) => finished = Some(report),
            }
        }

        let Some(report) = finished else {
            return;
        };

        self.worker = None;
        self.progress = None;
        self.view.invalidate_file_rows();
        self.view.selected_file = None;

        if report.cancelled {
            self.notify("Analysis cancelled — showing partial results", false);
        } else if report.is_empty() {
            self.notify(
                "No recognizable source files were found in this folder",
                true,
            );
        }

        self.report = Some(*report);
    }

    fn collect_shortcuts(&self, ctx: &egui::Context, actions: &mut Vec<UiAction>) {
        let (open, analyze, export, cancel) = ctx.input(|input| {
            (
                input.modifiers.command && input.key_pressed(egui::Key::O),
                (input.modifiers.command && input.key_pressed(egui::Key::R))
                    || input.key_pressed(egui::Key::F5),
                input.modifiers.command && input.key_pressed(egui::Key::E),
                input.key_pressed(egui::Key::Escape),
            )
        });

        if cancel && self.worker.is_some() {
            actions.push(UiAction::Cancel);
        }
        if ctx.memory(|memory| memory.focused().is_some()) {
            return;
        }
        if open {
            actions.push(UiAction::Browse);
        }
        if analyze {
            actions.push(UiAction::Analyze);
        }
        if export {
            actions.push(UiAction::Export(ExportKind::SummaryCsv));
        }
    }

    fn collect_dropped_paths(&mut self, ctx: &egui::Context) {
        if self.worker.is_some() {
            return;
        }
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        let Some(path) = dropped.iter().map(|file| file.path().to_path_buf()).next() else {
            return;
        };
        let folder = if path.is_file() {
            path.parent().map(Path::to_path_buf).unwrap_or(path)
        } else {
            path
        };
        self.root = folder.display().to_string();
        self.start_analysis(ctx);
    }

    fn apply_actions(&mut self, ctx: &egui::Context, actions: Vec<UiAction>) {
        for action in actions {
            match action {
                UiAction::Browse => self.browse(ctx),
                UiAction::Analyze => self.start_analysis(ctx),
                UiAction::Cancel => {
                    if let Some(worker) = &self.worker {
                        worker.cancel();
                    }
                }
                UiAction::ToggleTheme => {
                    self.dark = !self.dark;
                    theme::apply(ctx, self.dark);
                }
                UiAction::Export(kind) => self.export(kind),
                UiAction::CopyCsv => {
                    if let Some(report) = &self.report {
                        ctx.copy_text(export::summary_csv(report));
                        self.notify("CSV copied to clipboard", false);
                    }
                }
                UiAction::Reveal(path) => export::reveal(&path),
            }
        }
    }

    fn browse(&mut self, ctx: &egui::Context) {
        if let Some(folder) = rfd::FileDialog::new()
            .set_title("Select a project folder")
            .pick_folder()
        {
            self.root = folder.display().to_string();
            self.start_analysis(ctx);
        }
    }

    fn export(&mut self, kind: ExportKind) {
        let Some(report) = &self.report else {
            self.notify("Analyze a project before exporting", true);
            return;
        };
        let Some(path) = export::save_dialog(kind) else {
            return;
        };
        match export::write(kind, report, &path) {
            Ok(()) => self.notify(format!("Exported to {}", path.display()), false),
            Err(error) => self.notify(format!("Export failed: {error}"), true),
        }
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some(Toast::new(text, error));
    }

    fn language_checked(&self, name: &str) -> bool {
        self.selected_languages.is_empty() || self.selected_languages.contains(name)
    }

    fn set_language(&mut self, name: &str, checked: bool) {
        let all: Vec<&'static str> = language::supported_languages()
            .iter()
            .map(|lang| lang.name)
            .collect();

        if checked {
            if self.selected_languages.is_empty() {
                return;
            }
            self.selected_languages.insert(name.to_string());
            if self.selected_languages.len() >= all.len() {
                self.selected_languages.clear();
            }
        } else if self.selected_languages.is_empty() {
            self.selected_languages = all
                .iter()
                .filter(|language| **language != name)
                .map(|language| language.to_string())
                .collect();
        } else {
            self.selected_languages.remove(name);
            if self.selected_languages.is_empty() {
                // An empty set means "all languages"; keep the behaviour obvious.
                self.notify("At least one language stays selected — filter reset", false);
            }
        }
    }

    fn status(&self) -> (Color32, String) {
        if let Some(error) = &self.error {
            return (ERROR_COLOR, error.clone());
        }
        if self.worker.is_some() {
            let text = match self.progress {
                Some(Progress::Analyzing { .. }) => "Analyzing source files…",
                Some(Progress::Scanning { .. }) => "Walking the file tree…",
                None => "Starting analysis…",
            };
            return (theme::ACCENT, text.to_string());
        }
        if let Some(report) = &self.report {
            if report.cancelled {
                return (
                    theme::MIXED,
                    "Analysis cancelled — partial results shown".to_string(),
                );
            }
            let total = report.total();
            return (
                theme::COMMENT,
                format!(
                    "{} languages · {} files · {} lines · analyzed in {}",
                    report.summaries.len(),
                    format_number(total.file_count),
                    format_number(total.total_lines),
                    format_duration(report.elapsed)
                ),
            );
        }
        (
            theme::OTHER,
            "Ready — choose a project folder to begin".to_string(),
        )
    }
}

impl eframe::App for CodeLinesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut actions = Vec::new();

        self.poll_worker();
        self.collect_shortcuts(&ctx, &mut actions);
        self.collect_dropped_paths(&ctx);

        let window_fill = ui.visuals().window_fill;
        let stroke = theme::card_stroke(ui);

        egui::Panel::top("codelines-toolbar")
            .exact_size(54.0)
            .frame(
                egui::Frame::NONE
                    .fill(window_fill)
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ui, |ui| self.toolbar(ui, &mut actions));

        egui::Panel::bottom("codelines-status")
            .exact_size(30.0)
            .frame(
                egui::Frame::NONE
                    .fill(window_fill)
                    .inner_margin(egui::Margin::symmetric(14, 4))
                    .stroke(stroke),
            )
            .show(ui, |ui| self.status_bar(ui));

        if self.show_filters {
            egui::Panel::left("codelines-filters")
                .exact_size(SIDE_PANEL_WIDTH)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(window_fill)
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .stroke(stroke),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.filters_panel(ui));
                });
        }

        egui::CentralPanel::default().show(ui, |ui| self.central(ui, &mut actions));

        self.show_toast(&ctx);
        self.apply_actions(&ctx, actions);

        if self.worker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| toast.created.elapsed() < Duration::from_secs(4))
        {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }
}

impl CodeLinesApp {
    fn toolbar(&mut self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.horizontal_centered(|ui| {
            ui.label(
                RichText::new("CodeLines")
                    .size(18.0)
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.add_space(2.0);
            ui.separator();

            // Keep the path field flexible but leave room for the actions on the right.
            let width = (ui.available_width() - 430.0).clamp(170.0, 560.0);
            let response = ui.add_sized(
                [width, 26.0],
                egui::TextEdit::singleline(&mut self.root)
                    .hint_text("Project folder — or drag & drop one here"),
            );
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                actions.push(UiAction::Analyze);
            }

            if ui.button("Browse…").clicked() {
                actions.push(UiAction::Browse);
            }

            if self.worker.is_some() {
                if ui
                    .add(egui::Button::new(
                        RichText::new("Cancel").color(theme::MIXED),
                    ))
                    .clicked()
                {
                    actions.push(UiAction::Cancel);
                }
            } else if ui
                .add(
                    egui::Button::new(
                        RichText::new("Analyze")
                            .strong()
                            .color(Color32::from_rgb(0x0B, 0x1B, 0x33)),
                    )
                    .fill(theme::ACCENT),
                )
                .clicked()
            {
                actions.push(UiAction::Analyze);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let theme_label = if self.dark { "Light" } else { "Dark" };
                if ui
                    .button(theme_label)
                    .on_hover_text("Toggle light / dark theme")
                    .clicked()
                {
                    actions.push(UiAction::ToggleTheme);
                }

                ui.menu_button("Export", |ui| {
                    let enabled = self.report.is_some();
                    if ui
                        .add_enabled(enabled, egui::Button::new("Language summary (CSV)"))
                        .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::SummaryCsv));
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Per-file statistics (CSV)"))
                        .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::FilesCsv));
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Report (JSON)"))
                        .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::Json));
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Report (plain text)"))
                        .clicked()
                    {
                        actions.push(UiAction::Export(ExportKind::PlainText));
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(enabled, egui::Button::new("Copy summary CSV"))
                        .clicked()
                    {
                        actions.push(UiAction::CopyCsv);
                        ui.close();
                    }
                });

                let filters_label = if self.show_filters {
                    "Hide filters"
                } else {
                    "Show filters"
                };
                if ui.button(filters_label).clicked() {
                    self.show_filters = !self.show_filters;
                }
            });
        });
    }

    fn filters_panel(&mut self, ui: &mut Ui) {
        widgets::section_label(ui, "Scan scope");
        ui.add_space(2.0);

        ui.label(RichText::new("Exclude directories").size(11.5));
        ui.add(
            egui::TextEdit::multiline(&mut self.exclude_dirs)
                .desired_rows(2)
                .desired_width(ui.available_width())
                .hint_text("vendor, generated"),
        );
        ui.label(
            RichText::new("Comma or newline separated folder names")
                .size(9.5)
                .color(theme::muted(ui)),
        );

        ui.add_space(8.0);
        ui.label(RichText::new("Exclude files").size(11.5));
        ui.add(
            egui::TextEdit::multiline(&mut self.exclude_files)
                .desired_rows(2)
                .desired_width(ui.available_width())
                .hint_text("Cargo.lock, bundle.min.js"),
        );
        ui.label(
            RichText::new("Exact file names, comma or newline separated")
                .size(9.5)
                .color(theme::muted(ui)),
        );

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        widgets::section_label(ui, "Languages");
        ui.add(
            egui::TextEdit::singleline(&mut self.language_search)
                .desired_width(ui.available_width())
                .hint_text("Search languages…"),
        );

        let filtered = !self.selected_languages.is_empty();
        ui.horizontal(|ui| {
            let summary = if filtered {
                format!(
                    "{} of {} selected",
                    self.selected_languages.len(),
                    self.total_languages
                )
            } else {
                format!("All {} languages", self.total_languages)
            };
            ui.label(RichText::new(summary).size(9.5).color(theme::muted(ui)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if filtered && ui.small_button("Reset").clicked() {
                    self.selected_languages.clear();
                }
            });
        });
        ui.add_space(2.0);

        let list_height = (ui.available_height() - 40.0).max(120.0);
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let needle = self.language_search.trim().to_lowercase();
                let mut shown = 0;
                for lang in language::supported_languages() {
                    if !needle.is_empty() && !lang.name.to_lowercase().contains(&needle) {
                        continue;
                    }
                    shown += 1;
                    let mut checked = self.language_checked(lang.name);
                    let response = ui.checkbox(&mut checked, lang.name);
                    if response.changed() {
                        self.set_language(lang.name, checked);
                    }
                    response.on_hover_text(lang.extensions.join(", "));
                }
                if shown == 0 {
                    widgets::hint(ui, "No languages match this search.");
                }
            });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let (color, text) = self.status();
            let (rect, _) = ui.allocate_exact_size(vec2(7.0, 7.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 3.5, color);
            ui.add(
                egui::Label::new(RichText::new(text).size(11.0).color(theme::muted(ui))).truncate(),
            );

            match &self.progress {
                Some(Progress::Analyzing {
                    done,
                    total,
                    current,
                }) => {
                    ui.add_space(6.0);
                    let fraction = if *total == 0 {
                        0.0
                    } else {
                        *done as f32 / *total as f32
                    };
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .desired_width(180.0)
                            .text(format!(
                                "{} / {}",
                                format_number(*done),
                                format_number(*total)
                            )),
                    );
                    if let Some(path) = current {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ui.add(
                            egui::Label::new(
                                RichText::new(name).size(10.5).color(theme::muted(ui)),
                            )
                            .truncate(),
                        );
                    }
                }
                Some(Progress::Scanning { files_seen, .. }) => {
                    ui.add_space(6.0);
                    ui.spinner();
                    ui.label(
                        RichText::new(format!("{} files visited", format_number(*files_seen)))
                            .size(10.5)
                            .color(theme::muted(ui)),
                    );
                }
                None => {}
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .size(10.0)
                        .color(theme::muted(ui)),
                );
            });
        });
    }

    fn central(&mut self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        if self.report.is_none() {
            self.empty_state(ui, actions);
            return;
        }

        let file_count = self
            .report
            .as_ref()
            .map(|report| report.files.len())
            .unwrap_or(0);
        let diagnostics = self
            .report
            .as_ref()
            .map(|report| report.warnings.len() + report.unknown_extensions.len())
            .unwrap_or(0);

        ui.horizontal(|ui| {
            let tabs = [
                (Tab::Overview, "Overview".to_string()),
                (Tab::Files, format!("Files ({})", format_number(file_count))),
                (
                    Tab::Diagnostics,
                    format!("Diagnostics ({})", format_number(diagnostics)),
                ),
            ];
            for (tab, label) in tabs {
                let selected = self.view.tab == tab;
                if ui
                    .selectable_label(selected, RichText::new(label).size(12.0))
                    .clicked()
                {
                    self.view.tab = tab;
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(report) = &self.report {
                    let total = report.total();
                    ui.label(
                        RichText::new(format!(
                            "{} files · {} lines",
                            format_number(total.file_count),
                            format_number(total.total_lines)
                        ))
                        .size(10.5)
                        .color(theme::muted(ui)),
                    );
                }
            });
        });
        ui.add_space(4.0);

        let report = self.report.as_ref().expect("report checked above");
        let view = &mut self.view;

        match view.tab {
            Tab::Overview => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| overview_tab(ui, report, view));
            }
            Tab::Files => files_tab(ui, report, view, actions),
            Tab::Diagnostics => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| diagnostics_tab(ui, report));
            }
        }
    }

    fn empty_state(&mut self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.vertical_centered(|ui| {
            ui.add_space(56.0);
            ui.label(
                RichText::new("CodeLines")
                    .size(34.0)
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.label(
                RichText::new("Line statistics for 35+ languages")
                    .size(14.0)
                    .color(theme::muted(ui)),
            );
            ui.add_space(18.0);

            if ui
                .add(
                    egui::Button::new(RichText::new("Open Project Folder…").size(14.0))
                        .min_size(vec2(220.0, 38.0)),
                )
                .clicked()
            {
                actions.push(UiAction::Browse);
            }

            ui.add_space(8.0);
            ui.label(
                RichText::new("or drag and drop a folder onto this window")
                    .size(11.0)
                    .color(theme::muted(ui)),
            );

            if let Some(error) = &self.error {
                ui.add_space(10.0);
                ui.colored_label(ERROR_COLOR, error);
            }

            ui.add_space(30.0);
            ui.allocate_ui_with_layout(vec2(340.0, 130.0), Layout::top_down(Align::Min), |ui| {
                self.shortcut_help(ui)
            });
        });
    }

    fn shortcut_help(&self, ui: &mut Ui) {
        let rows = [
            ("Cmd/Ctrl + O", "Open a project folder"),
            ("Cmd/Ctrl + R", "Analyze again (F5 also works)"),
            ("Cmd/Ctrl + E", "Export the language summary (CSV)"),
            ("Esc", "Cancel a running analysis"),
        ];

        let frame = theme::card(ui);
        frame.show(ui, |ui| {
            ui.set_width(320.0);
            widgets::section_label(ui, "Keyboard shortcuts");
            ui.add_space(4.0);
            egui::Grid::new("shortcut-grid")
                .num_columns(2)
                .spacing(vec2(18.0, 6.0))
                .show(ui, |ui| {
                    for (keys, description) in rows {
                        ui.label(
                            RichText::new(keys)
                                .size(10.5)
                                .monospace()
                                .color(theme::ACCENT),
                        );
                        ui.label(
                            RichText::new(description)
                                .size(10.5)
                                .color(theme::muted(ui)),
                        );
                        ui.end_row();
                    }
                });
        });
    }

    fn show_toast(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else {
            return;
        };
        if toast.created.elapsed() > Duration::from_secs(4) {
            self.toast = None;
            return;
        }

        let (background, text_color) = if toast.error {
            (Color32::from_rgb(0x3A, 0x1F, 0x24), ERROR_COLOR)
        } else {
            (Color32::from_rgb(0x1B, 0x2B, 0x24), theme::COMMENT)
        };

        egui::Area::new(egui::Id::unique("codelines-toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, vec2(-18.0, -42.0))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(background)
                    .stroke(egui::Stroke::new(1.0, text_color.gamma_multiply(0.6)))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(RichText::new(&toast.text).size(11.5).color(text_color));
                    });
            });
    }
}

fn overview_tab(ui: &mut Ui, report: &Report, view: &mut ViewState) {
    let total = report.total();
    kpi_cards(ui, report, &total);
    ui.add_space(12.0);

    ui.columns(2, |columns| {
        card_ui(&mut columns[0], |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Lines by language").size(13.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if view.show_all_languages {
                        "Collapse"
                    } else {
                        "Show all"
                    };
                    if ui.small_button(label).clicked() {
                        view.show_all_languages = !view.show_all_languages;
                    }
                });
            });
            ui.add_space(4.0);
            charts::lines_by_language(ui, report, view.show_all_languages);
            ui.add_space(6.0);
            charts::category_legend(ui, &total);
        });

        card_ui(&mut columns[1], |ui| {
            ui.label(RichText::new("Language share").size(13.0).strong());
            ui.label(
                RichText::new("by effective code lines")
                    .size(10.0)
                    .color(theme::muted(ui)),
            );
            ui.add_space(4.0);
            charts::language_share(ui, report, 152.0);
        });
    });

    ui.add_space(12.0);
    card_ui(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Languages").size(13.0).strong());
            ui.label(
                RichText::new(format!("{} detected", report.summaries.len()))
                    .size(10.0)
                    .color(theme::muted(ui)),
            );
        });
        ui.add_space(4.0);
        let rows = table::sort_summaries(&report.summaries, view.summary_sort);
        table::language_table(ui, &rows, &mut view.summary_sort);
    });
}

fn kpi_cards(ui: &mut Ui, report: &Report, total: &LanguageSummary) {
    let spacing = ui.spacing().item_spacing.x;
    let available = ui.available_width();
    let columns = (((available + spacing) / (150.0 + spacing)).floor() as usize).clamp(2, 6);
    let card_width = (available - spacing * (columns as f32 - 1.0)) / columns as f32;

    let cards = [
        (
            "Source files",
            format_number(total.file_count),
            format!("{} languages", report.summaries.len()),
            theme::ACCENT,
        ),
        (
            "Total lines",
            format_number(total.total_lines),
            format!(
                "{} files visited",
                format_number(report.total_files_scanned)
            ),
            theme::OTHER,
        ),
        (
            "Effective code",
            format_number(total.effective_code()),
            "code + mixed lines".to_string(),
            theme::CODE,
        ),
        (
            "Comments",
            format_number(total.total_comment()),
            format_percent(total.comment_ratio()),
            theme::COMMENT,
        ),
        (
            "Mixed lines",
            format_number(total.mixed_lines),
            format_percent(if total.total_lines == 0 {
                0.0
            } else {
                total.mixed_lines as f64 / total.total_lines as f64
            }),
            theme::MIXED,
        ),
        (
            "Blank lines",
            format_number(total.blank_lines),
            format_percent(if total.total_lines == 0 {
                0.0
            } else {
                total.blank_lines as f64 / total.total_lines as f64
            }),
            theme::BLANK,
        ),
    ];

    ui.horizontal(|ui| {
        for (label, value, hint, color) in cards {
            ui.allocate_ui_with_layout(
                vec2(card_width, 94.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(card_width);
                    widgets::stat_card(ui, label, &value, &hint, color);
                },
            );
        }
    });
}

fn files_tab(ui: &mut Ui, report: &Report, view: &mut ViewState, actions: &mut Vec<UiAction>) {
    let frame = theme::card(ui);
    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.set_min_height(ui.available_height());

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut view.file_query)
                    .desired_width(240.0)
                    .hint_text("Filter by path or language…"),
            );
            view.ensure_file_rows(&report.files);
            ui.label(
                RichText::new(format!(
                    "{} of {} files",
                    format_number(view.file_rows.len()),
                    format_number(report.files.len())
                ))
                .size(10.5)
                .color(theme::muted(ui)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Export CSV").clicked() {
                    actions.push(UiAction::Export(ExportKind::FilesCsv));
                }
            });
        });

        ui.label(
            RichText::new("Double-click a row to reveal the file in the system file manager.")
                .size(9.5)
                .color(theme::muted(ui)),
        );
        ui.add_space(4.0);

        let revealed = table::file_table(
            ui,
            &report.files,
            &report.roots,
            &view.file_rows,
            &mut view.file_sort,
            &mut view.selected_file,
        );
        if let Some(path) = revealed {
            actions.push(UiAction::Reveal(path));
        }

        if report.files.is_empty() {
            ui.add_space(8.0);
            widgets::hint(ui, "No files to show.");
        }
    });
}

fn diagnostics_tab(ui: &mut Ui, report: &Report) {
    card_ui(ui, |ui| {
        ui.label(RichText::new("Scan summary").size(13.0).strong());
        ui.add_space(4.0);
        egui::Grid::new("scan-summary")
            .num_columns(2)
            .spacing(vec2(20.0, 6.0))
            .show(ui, |ui| {
                let roots = report
                    .roots
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let rows = [
                    ("Roots", roots),
                    ("Files visited", format_number(report.total_files_scanned)),
                    ("Analyzed files", format_number(report.files.len())),
                    ("Elapsed", format_duration(report.elapsed)),
                    (
                        "Cancelled",
                        if report.cancelled { "yes" } else { "no" }.to_string(),
                    ),
                    ("Warnings", format_number(report.warnings.len())),
                ];
                for (label, value) in rows {
                    ui.label(RichText::new(label).size(11.0).color(theme::muted(ui)));
                    ui.label(RichText::new(value).size(11.0));
                    ui.end_row();
                }
            });
    });

    ui.add_space(12.0);
    card_ui(ui, |ui| {
        ui.label(
            RichText::new(format!(
                "Skipped file types ({})",
                report.unknown_extensions.len()
            ))
            .size(13.0)
            .strong(),
        );
        ui.label(
            RichText::new("Files whose extension is not associated with a supported language.")
                .size(10.0)
                .color(theme::muted(ui)),
        );
        ui.add_space(4.0);

        if report.unknown_extensions.is_empty() {
            widgets::hint(ui, "Every file in this project was recognized.");
        } else {
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("unknown-extensions")
                        .num_columns(2)
                        .striped(true)
                        .spacing(vec2(20.0, 4.0))
                        .show(ui, |ui| {
                            for (extension, count) in &report.unknown_extensions {
                                ui.label(RichText::new(extension).size(11.0).monospace());
                                ui.label(
                                    RichText::new(format!("{} file(s)", format_number(*count)))
                                        .size(11.0)
                                        .color(theme::muted(ui)),
                                );
                                ui.end_row();
                            }
                        });
                });
        }
    });

    ui.add_space(12.0);
    card_ui(ui, |ui| {
        ui.label(
            RichText::new(format!("Warnings ({})", report.warnings.len()))
                .size(13.0)
                .strong(),
        );
        ui.add_space(4.0);
        if report.warnings.is_empty() {
            widgets::hint(ui, "No warnings — every file was read successfully.");
        } else {
            for warning in &report.warnings {
                ui.label(RichText::new(warning).size(10.5).color(theme::MIXED));
            }
        }
    });
}

/// Wrap content in a themed card that stretches to the available width.
fn card_ui(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let frame = theme::card(ui);
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        add_contents(ui);
    });
}

fn split_list(input: &str) -> Vec<String> {
    let mut values: Vec<String> = input
        .split(['\n', ','])
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    values.dedup();
    values
}
