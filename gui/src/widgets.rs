//! Small reusable UI building blocks (formatting helpers, cards, legends).

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, Align, Color32, RichText, Sense, Ui, vec2};

use super::theme;

/// `1234567` → `1,234,567`.
pub fn format_number(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Compact form for narrow table cells: `9,999`, `39.8k`, `18.60M`.
pub fn format_compact(value: usize) -> String {
    const THOUSAND: f64 = 1_000.0;
    const MILLION: f64 = 1_000_000.0;
    const BILLION: f64 = 1_000_000_000.0;
    let value_f64 = value as f64;

    if value < 10_000 {
        format_number(value)
    } else if value_f64 < MILLION {
        format!("{:.1}k", value_f64 / THOUSAND)
    } else if value_f64 < 10.0 * MILLION {
        format!("{:.2}M", value_f64 / MILLION)
    } else if value_f64 < BILLION {
        format!("{:.1}M", value_f64 / MILLION)
    } else {
        format!("{:.2}G", value_f64 / BILLION)
    }
}

/// `0.8133` → `81.3%`.
pub fn format_percent(ratio: f64) -> String {
    format!("{:.1}%", ratio * 100.0)
}

/// Human readable duration.
pub fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds < 1.0 {
        format!("{} ms", (seconds * 1000.0).round() as u64)
    } else if seconds < 60.0 {
        format!("{seconds:.2} s")
    } else {
        format!("{}m {:02}s", (seconds / 60.0) as u64, seconds as u64 % 60)
    }
}

/// Path relative to the first matching scan root, when possible.
pub fn relative_path(path: &Path, roots: &[PathBuf]) -> String {
    for root in roots {
        if let Ok(stripped) = path.strip_prefix(root) {
            if stripped.as_os_str().is_empty() {
                return path.display().to_string();
            }
            return stripped.display().to_string();
        }
    }
    path.display().to_string()
}

/// Uppercase section caption, e.g. `FILTERS`.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(10.0)
            .strong()
            .color(theme::muted(ui)),
    );
}

/// A small colored square used in legends.
pub fn swatch(ui: &mut Ui, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(3), color);
    response
}

/// One legend entry: swatch, label and right-aligned value.
pub fn legend_row(ui: &mut Ui, color: Color32, label: &str, value: String) {
    ui.horizontal(|ui| {
        swatch(ui, color);
        ui.add(egui::Label::new(RichText::new(label).size(11.5)).truncate());
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .size(11.0)
                    .monospace()
                    .color(theme::muted(ui)),
            );
        });
    });
}

/// KPI card: label, big value and a secondary hint.
pub fn stat_card(ui: &mut Ui, label: &str, value: &str, hint: &str, color: Color32) {
    let width = ui.available_width();
    theme::card(ui).show(ui, |ui| {
        ui.set_min_size(vec2(width - 28.0, 62.0));
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                swatch(ui, color);
                ui.label(RichText::new(label).size(11.0).color(theme::muted(ui)));
            });
            ui.label(RichText::new(value).size(22.0).strong().color(color));
            ui.add(egui::Label::new(
                RichText::new(hint).size(10.0).color(theme::muted(ui)),
            ));
        });
    });
}

/// Centered placeholder text.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(12.0)
            .italics()
            .color(theme::muted(ui)),
    );
}
