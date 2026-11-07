//! Sortable tables for language summaries and per-file statistics.

use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, RichText, Sense, Shape, Stroke, Ui, pos2, vec2};
use egui_extras::{Column, TableBuilder};

use crate::output::LanguageSummary;
use crate::report::FileEntry;

use super::theme;
use super::widgets::{format_compact, format_number, format_percent, relative_path};

/// Sortable columns of the language table.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SummarySort {
    Language,
    Files,
    Total,
    #[default]
    Code,
    Comment,
    Mixed,
    Blank,
    CodePercent,
    CommentPercent,
}

/// Sortable columns of the per-file table.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FileSort {
    Path,
    Language,
    Total,
    #[default]
    Code,
    Comment,
    Mixed,
    Blank,
    CodePercent,
}

/// Current sort key and direction.
#[derive(Clone, Copy, Debug)]
pub struct SortState<K> {
    pub key: K,
    pub ascending: bool,
}

impl<K: Copy + PartialEq> SortState<K> {
    pub fn click(&mut self, key: K) {
        if self.key == key {
            self.ascending = !self.ascending;
        } else {
            self.key = key;
            self.ascending = false;
        }
    }
}

impl<K: Copy + PartialEq + Default> Default for SortState<K> {
    fn default() -> Self {
        Self {
            key: K::default(),
            ascending: false,
        }
    }
}

fn ordered<K: Copy + PartialEq>(
    state: SortState<K>,
    value: std::cmp::Ordering,
) -> std::cmp::Ordering {
    if state.ascending {
        value
    } else {
        value.reverse()
    }
}

/// Return the summaries sorted according to `state`.
pub fn sort_summaries(
    summaries: &[LanguageSummary],
    state: SortState<SummarySort>,
) -> Vec<LanguageSummary> {
    let mut rows = summaries.to_vec();
    rows.sort_by(|a, b| {
        let ordering = match state.key {
            SummarySort::Language => a.language.cmp(&b.language),
            SummarySort::Files => a.file_count.cmp(&b.file_count),
            SummarySort::Total => a.total_lines.cmp(&b.total_lines),
            SummarySort::Code => a.effective_code().cmp(&b.effective_code()),
            SummarySort::Comment => a.total_comment().cmp(&b.total_comment()),
            SummarySort::Mixed => a.mixed_lines.cmp(&b.mixed_lines),
            SummarySort::Blank => a.blank_lines.cmp(&b.blank_lines),
            SummarySort::CodePercent => a
                .code_ratio()
                .partial_cmp(&b.code_ratio())
                .unwrap_or(std::cmp::Ordering::Equal),
            SummarySort::CommentPercent => a
                .comment_ratio()
                .partial_cmp(&b.comment_ratio())
                .unwrap_or(std::cmp::Ordering::Equal),
        };
        ordered(state, ordering).then_with(|| a.language.cmp(&b.language))
    });
    rows
}

/// Filter files by `query` and sort them according to `state`.
pub fn filter_and_sort_files(
    files: &[FileEntry],
    query: &str,
    state: SortState<FileSort>,
) -> Vec<usize> {
    let needle = query.trim().to_lowercase();
    let matches = |file: &FileEntry| {
        if needle.is_empty() {
            return true;
        }
        file.path.to_string_lossy().to_lowercase().contains(&needle)
            || file.language.to_lowercase().contains(&needle)
    };

    let mut indices: Vec<usize> = files
        .iter()
        .enumerate()
        .filter(|(_, file)| matches(file))
        .map(|(index, _)| index)
        .collect();

    indices.sort_by(|a, b| {
        let left = &files[*a];
        let right = &files[*b];
        let ordering = match state.key {
            FileSort::Path => left.path.cmp(&right.path),
            FileSort::Language => left
                .language
                .cmp(&right.language)
                .then_with(|| left.path.cmp(&right.path)),
            FileSort::Total => left.stats.total_lines.cmp(&right.stats.total_lines),
            FileSort::Code => left
                .stats
                .effective_code_lines()
                .cmp(&right.stats.effective_code_lines()),
            FileSort::Comment => left
                .stats
                .total_comment_lines()
                .cmp(&right.stats.total_comment_lines()),
            FileSort::Mixed => left.stats.mixed_lines.cmp(&right.stats.mixed_lines),
            FileSort::Blank => left.stats.blank_lines.cmp(&right.stats.blank_lines),
            FileSort::CodePercent => left
                .stats
                .code_ratio()
                .partial_cmp(&right.stats.code_ratio())
                .unwrap_or(std::cmp::Ordering::Equal),
        };
        ordered(state, ordering).then_with(|| left.path.cmp(&right.path))
    });

    indices
}

fn header_button<K: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    key: K,
    state: &mut SortState<K>,
    align_right: bool,
) {
    let active = state.key == key;
    let color = if active {
        theme::ACCENT
    } else {
        theme::muted(ui)
    };
    let button =
        egui::Button::new(RichText::new(label).size(10.5).strong().color(color)).frame(false);

    let clicked = if align_right {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let clicked = ui.add(button).clicked();
            if active {
                paint_sort_arrow(ui, state.ascending);
            }
            clicked
        })
        .inner
    } else {
        let clicked = ui.add(button).clicked();
        if active {
            paint_sort_arrow(ui, state.ascending);
        }
        clicked
    };

    if clicked {
        state.click(key);
    }
}

fn paint_sort_arrow(ui: &mut Ui, ascending: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 10.0), Sense::hover());
    let center = rect.center();
    let points = if ascending {
        vec![
            pos2(center.x - 3.5, center.y + 2.0),
            pos2(center.x + 3.5, center.y + 2.0),
            pos2(center.x, center.y - 2.5),
        ]
    } else {
        vec![
            pos2(center.x - 3.5, center.y - 2.0),
            pos2(center.x + 3.5, center.y - 2.0),
            pos2(center.x, center.y + 2.5),
        ]
    };
    ui.painter()
        .add(Shape::convex_polygon(points, theme::ACCENT, Stroke::NONE));
}

fn number_cell(ui: &mut Ui, value: usize) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let response = ui.label(RichText::new(format_compact(value)).size(11.0).monospace());
        if value >= 10_000 {
            response.on_hover_text(format_number(value));
        }
    });
}

fn percent_cell(ui: &mut Ui, ratio: f64) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(
            RichText::new(format_percent(ratio))
                .size(11.0)
                .monospace()
                .color(theme::muted(ui)),
        );
    });
}

/// Language summary table with clickable, sortable headers.
pub fn language_table(ui: &mut Ui, rows: &[LanguageSummary], state: &mut SortState<SummarySort>) {
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::remainder().at_least(120.0))
        .column(Column::exact(52.0))
        .column(Column::exact(68.0))
        .column(Column::exact(66.0))
        .column(Column::exact(70.0))
        .column(Column::exact(58.0))
        .column(Column::exact(58.0))
        .column(Column::exact(58.0))
        .column(Column::exact(68.0))
        .vscroll(false)
        .auto_shrink([false, true])
        .header(26.0, |mut header| {
            header.col(|ui| header_button(ui, "Language", SummarySort::Language, state, false));
            header.col(|ui| header_button(ui, "Files", SummarySort::Files, state, true));
            header.col(|ui| header_button(ui, "Total", SummarySort::Total, state, true));
            header.col(|ui| header_button(ui, "Code", SummarySort::Code, state, true));
            header.col(|ui| header_button(ui, "Comment", SummarySort::Comment, state, true));
            header.col(|ui| header_button(ui, "Mixed", SummarySort::Mixed, state, true));
            header.col(|ui| header_button(ui, "Blank", SummarySort::Blank, state, true));
            header.col(|ui| header_button(ui, "Code%", SummarySort::CodePercent, state, true));
            header
                .col(|ui| header_button(ui, "Comment%", SummarySort::CommentPercent, state, true));
        })
        .body(|body| {
            body.rows(24.0, rows.len(), |mut row| {
                let summary = &rows[row.index()];
                row.col(|ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&summary.language).size(11.5).strong())
                            .truncate(),
                    );
                });
                row.col(|ui| number_cell(ui, summary.file_count));
                row.col(|ui| number_cell(ui, summary.total_lines));
                row.col(|ui| number_cell(ui, summary.effective_code()));
                row.col(|ui| number_cell(ui, summary.total_comment()));
                row.col(|ui| number_cell(ui, summary.mixed_lines));
                row.col(|ui| number_cell(ui, summary.blank_lines));
                row.col(|ui| percent_cell(ui, summary.code_ratio()));
                row.col(|ui| percent_cell(ui, summary.comment_ratio()));
            });
        });
}

/// Per-file table. Returns the path of a row that was double-clicked.
pub fn file_table(
    ui: &mut Ui,
    files: &[FileEntry],
    roots: &[PathBuf],
    rows: &[usize],
    state: &mut SortState<FileSort>,
    selected: &mut Option<usize>,
) -> Option<PathBuf> {
    let mut revealed = None;

    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::remainder().at_least(160.0))
        .column(Column::exact(92.0))
        .column(Column::exact(58.0))
        .column(Column::exact(58.0))
        .column(Column::exact(64.0))
        .column(Column::exact(52.0))
        .column(Column::exact(52.0))
        .column(Column::exact(56.0))
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .header(26.0, |mut header| {
            header.col(|ui| header_button(ui, "Path", FileSort::Path, state, false));
            header.col(|ui| header_button(ui, "Language", FileSort::Language, state, false));
            header.col(|ui| header_button(ui, "Total", FileSort::Total, state, true));
            header.col(|ui| header_button(ui, "Code", FileSort::Code, state, true));
            header.col(|ui| header_button(ui, "Comment", FileSort::Comment, state, true));
            header.col(|ui| header_button(ui, "Mixed", FileSort::Mixed, state, true));
            header.col(|ui| header_button(ui, "Blank", FileSort::Blank, state, true));
            header.col(|ui| header_button(ui, "Code%", FileSort::CodePercent, state, true));
        })
        .body(|body| {
            body.rows(24.0, rows.len(), |mut row| {
                let index = rows[row.index()];
                let file = &files[index];
                row.set_selected(*selected == Some(index));

                row.col(|ui| {
                    let path = relative_path(&file.path, roots);
                    ui.add(
                        egui::Label::new(RichText::new(&path).size(11.0).monospace()).truncate(),
                    )
                    .on_hover_text(file.path.display().to_string());
                });
                row.col(|ui| {
                    ui.label(
                        RichText::new(&file.language)
                            .size(11.0)
                            .color(theme::muted(ui)),
                    );
                });
                row.col(|ui| number_cell(ui, file.stats.total_lines));
                row.col(|ui| number_cell(ui, file.stats.effective_code_lines()));
                row.col(|ui| number_cell(ui, file.stats.total_comment_lines()));
                row.col(|ui| number_cell(ui, file.stats.mixed_lines));
                row.col(|ui| number_cell(ui, file.stats.blank_lines));
                row.col(|ui| percent_cell(ui, file.stats.code_ratio()));

                let response = row.response();
                if response.clicked() {
                    *selected = Some(index);
                }
                if response.double_clicked() {
                    revealed = Some(file.path.clone());
                }
            });
        });

    revealed
}
