//! Custom painted charts: stacked bars and a language share donut.
//!
//! Everything is drawn with the egui painter so the GUI stays dependency free
//! (no plotting crate) and matches the application theme exactly.

use std::f64::consts::TAU;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Label, Rect, Response, RichText, Sense,
    Shape, Stroke, Ui, pos2, vec2,
};

use crate::output::LanguageSummary;
use crate::report::Report;

use super::theme;
use super::widgets::{format_number, legend_row};

/// One colored segment of a stacked bar.
pub struct Segment {
    pub label: &'static str,
    pub value: usize,
    pub color: Color32,
}

/// Bar rows drawn in the "Lines by language" card.
struct BarRow {
    name: String,
    code: usize,
    comment: usize,
    mixed: usize,
    blank: usize,
    total: usize,
}

impl BarRow {
    fn from_summary(summary: &LanguageSummary) -> Self {
        Self {
            name: summary.language.clone(),
            code: summary.code_lines,
            comment: summary.comment_lines,
            mixed: summary.mixed_lines,
            blank: summary.blank_lines,
            total: summary.total_lines,
        }
    }

    fn empty(name: String) -> Self {
        Self {
            name,
            code: 0,
            comment: 0,
            mixed: 0,
            blank: 0,
            total: 0,
        }
    }

    fn add(&mut self, other: &LanguageSummary) {
        self.code += other.code_lines;
        self.comment += other.comment_lines;
        self.mixed += other.mixed_lines;
        self.blank += other.blank_lines;
        self.total += other.total_lines;
    }
}

/// Draw a horizontal stacked bar inside `width`.
pub fn stacked_bar(
    ui: &mut Ui,
    total: usize,
    segments: &[Segment],
    width: f32,
    height: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let painter = ui.painter();
    let radius = CornerRadius::same((height / 2.0).round() as u8);

    painter.rect_filled(rect, radius, ui.visuals().faint_bg_color);

    if total > 0 {
        let drawn: Vec<usize> = segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| segment.value > 0)
            .map(|(index, _)| index)
            .collect();
        let first = drawn.first().copied();
        let last = drawn.last().copied();

        let mut x = rect.left();
        for (index, segment) in segments.iter().enumerate() {
            if segment.value == 0 {
                continue;
            }
            let width = rect.width() * (segment.value as f32 / total as f32);
            let corners = match (Some(index) == first, Some(index) == last) {
                (true, true) => radius,
                (true, false) => CornerRadius {
                    nw: radius.nw,
                    sw: radius.sw,
                    ne: 0,
                    se: 0,
                },
                (false, true) => CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: radius.ne,
                    se: radius.se,
                },
                (false, false) => CornerRadius::ZERO,
            };
            painter.rect_filled(
                Rect::from_min_size(pos2(x, rect.top()), vec2(width, rect.height())),
                corners,
                segment.color,
            );
            x += width;
        }
    }

    response
}

/// "Lines by language" — one stacked bar per language.
pub fn lines_by_language(ui: &mut Ui, report: &Report, show_all: bool) {
    let limit = if show_all { report.summaries.len() } else { 7 };
    let visible = limit.min(report.summaries.len());

    let mut rows: Vec<BarRow> = report
        .summaries
        .iter()
        .take(visible)
        .map(BarRow::from_summary)
        .collect();

    if visible < report.summaries.len() {
        let mut other = BarRow::empty(format!(
            "Other ({} languages)",
            report.summaries.len() - visible
        ));
        for summary in report.summaries.iter().skip(visible) {
            other.add(summary);
        }
        rows.push(other);
    }

    if rows.is_empty() {
        super::widgets::hint(ui, "No source files analyzed yet.");
        return;
    }

    let max_total = rows.iter().map(|row| row.total).max().unwrap_or(0).max(1);

    for row in &rows {
        ui.horizontal(|ui| {
            let label_width = 138.0;
            ui.allocate_ui_with_layout(
                vec2(label_width, 20.0),
                egui::Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_width(label_width);
                    let text = RichText::new(&row.name).size(11.5).color(theme::muted(ui));
                    ui.add(Label::new(text).truncate()).on_hover_text(&row.name);
                },
            );

            let value_width = 92.0;
            let bar_width = (ui.available_width() - value_width - 6.0).max(50.0);
            let segments = [
                Segment {
                    label: "Code",
                    value: row.code,
                    color: theme::CODE,
                },
                Segment {
                    label: "Comments",
                    value: row.comment,
                    color: theme::COMMENT,
                },
                Segment {
                    label: "Mixed",
                    value: row.mixed,
                    color: theme::MIXED,
                },
                Segment {
                    label: "Blank",
                    value: row.blank,
                    color: theme::BLANK,
                },
            ];

            let response = stacked_bar(ui, max_total, &segments, bar_width, 14.0);
            response.on_hover_ui(|ui| {
                ui.label(RichText::new(&row.name).strong());
                for segment in &segments {
                    let share = if row.total == 0 {
                        0.0
                    } else {
                        segment.value as f64 / row.total as f64
                    };
                    legend_row(
                        ui,
                        segment.color,
                        segment.label,
                        format!("{} ({:.1}%)", format_number(segment.value), share * 100.0),
                    );
                }
                ui.label(format!("Total: {} lines", format_number(row.total)));
            });

            ui.add_sized(
                [value_width, 20.0],
                Label::new(
                    RichText::new(format_number(row.total))
                        .size(11.0)
                        .monospace()
                        .color(theme::muted(ui)),
                ),
            );
        });
    }
}

/// Legend for the line categories shared by every bar.
pub fn category_legend(ui: &mut Ui, total: &LanguageSummary) {
    ui.horizontal_wrapped(|ui| {
        let entries = [
            ("Code", total.code_lines, theme::CODE),
            ("Comments", total.comment_lines, theme::COMMENT),
            ("Mixed", total.mixed_lines, theme::MIXED),
            ("Blank", total.blank_lines, theme::BLANK),
        ];
        for (label, value, color) in entries {
            super::widgets::swatch(ui, color);
            ui.label(
                RichText::new(format!("{label} {}", format_number(value)))
                    .size(10.5)
                    .color(theme::muted(ui)),
            );
            ui.add_space(8.0);
        }
    });
}

struct Slice {
    label: String,
    fraction: f64,
    value: usize,
    color: Color32,
}

/// Donut chart plus legend showing each language's share of effective code.
pub fn language_share(ui: &mut Ui, report: &Report, diameter: f32) {
    let total: usize = report
        .summaries
        .iter()
        .map(LanguageSummary::effective_code)
        .sum();

    if total == 0 {
        super::widgets::hint(ui, "Nothing to visualize yet.");
        return;
    }

    const MAX_SLICES: usize = 6;
    let visible = MAX_SLICES.min(report.summaries.len());
    let mut slices: Vec<Slice> = report
        .summaries
        .iter()
        .take(visible)
        .enumerate()
        .map(|(index, summary)| Slice {
            label: summary.language.clone(),
            fraction: summary.effective_code() as f64 / total as f64,
            value: summary.effective_code(),
            color: theme::language_color(index),
        })
        .collect();

    if visible < report.summaries.len() {
        let rest: usize = report
            .summaries
            .iter()
            .skip(visible)
            .map(LanguageSummary::effective_code)
            .sum();
        slices.push(Slice {
            label: format!("Other ({} languages)", report.summaries.len() - visible),
            fraction: rest as f64 / total as f64,
            value: rest,
            color: theme::OTHER,
        });
    }

    let hole = theme::card_fill(ui);
    ui.vertical_centered(|ui| {
        donut(
            ui,
            diameter,
            &slices,
            "effective code",
            &format_number(total),
            hole,
        );
    });
    ui.add_space(6.0);

    for slice in &slices {
        legend_row(
            ui,
            slice.color,
            &slice.label,
            format!("{:.1}%", slice.fraction * 100.0),
        );
    }
}

fn donut(
    ui: &mut Ui,
    diameter: f32,
    slices: &[Slice],
    caption: &str,
    center: &str,
    hole: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(diameter, diameter), Sense::hover());
    let painter = ui.painter();
    let center_pos = rect.center();
    let radius = diameter * 0.5;
    let inner = radius * 0.62;
    let pointer = response.hover_pos();

    let text_color = ui.visuals().text_color();
    let muted = theme::muted(ui);

    let mut hovered = None;
    let mut start = -TAU / 4.0;

    for (index, slice) in slices.iter().enumerate() {
        let sweep = slice.fraction * TAU;
        if sweep <= 0.0 {
            continue;
        }
        let steps = ((sweep / 0.05).ceil() as usize).max(2);
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center_pos);
        for step in 0..=steps {
            let angle = start + sweep * step as f64 / steps as f64;
            points
                .push(center_pos + vec2(radius * angle.cos() as f32, radius * angle.sin() as f32));
        }
        let stroke: Stroke = Stroke::NONE;
        painter.add(Shape::convex_polygon(points, slice.color, stroke));

        if let Some(pointer) = pointer {
            let delta = pointer - center_pos;
            let distance = delta.length();
            if distance <= radius
                && distance >= inner
                && angle_within(delta.angle() as f64, start, sweep)
            {
                hovered = Some(index);
            }
        }

        start += sweep;
    }

    painter.circle_filled(center_pos, inner, hole);
    painter.text(
        center_pos + vec2(0.0, -9.0),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(10.0),
        muted,
    );
    painter.text(
        center_pos + vec2(0.0, 9.0),
        Align2::CENTER_CENTER,
        center,
        FontId::proportional(16.0),
        text_color,
    );

    match hovered {
        Some(index) => {
            let slice = &slices[index];
            response.on_hover_text_at_pointer(format!(
                "{} — {} lines ({:.1}%)",
                slice.label,
                format_number(slice.value),
                slice.fraction * 100.0
            ))
        }
        None => response,
    }
}

fn angle_within(angle: f64, start: f64, sweep: f64) -> bool {
    let mut delta = (angle - start) % TAU;
    if delta < 0.0 {
        delta += TAU;
    }
    delta <= sweep
}

