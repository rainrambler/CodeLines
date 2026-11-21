//! Colors, palette and global styling for the GUI.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, ThemePreference,
    Visuals,
};

/// Primary accent color used for interactive elements and totals.
pub const ACCENT: Color32 = Color32::from_rgb(0x4F, 0x9C, 0xF7);
/// Pure code lines.
pub const CODE: Color32 = Color32::from_rgb(0x4F, 0x9C, 0xF7);
/// Pure comment lines.
pub const COMMENT: Color32 = Color32::from_rgb(0x3F, 0xC1, 0x8F);
/// Lines containing both code and a comment.
pub const MIXED: Color32 = Color32::from_rgb(0xF2, 0xB5, 0x41);
/// Blank lines.
pub const BLANK: Color32 = Color32::from_rgb(0x5D, 0x68, 0x7A);
/// Neutral color for aggregated "Other" entries and skipped files.
pub const OTHER: Color32 = Color32::from_rgb(0x7A, 0x83, 0x95);

const PALETTE: [Color32; 12] = [
    Color32::from_rgb(0x4F, 0x9C, 0xF7),
    Color32::from_rgb(0x3F, 0xC1, 0x8F),
    Color32::from_rgb(0xF2, 0xB5, 0x41),
    Color32::from_rgb(0xE5, 0x6B, 0x6F),
    Color32::from_rgb(0xA9, 0x8B, 0xFA),
    Color32::from_rgb(0x38, 0xBD, 0xD9),
    Color32::from_rgb(0xF2, 0x8B, 0x5C),
    Color32::from_rgb(0x84, 0xCC, 0x16),
    Color32::from_rgb(0xEC, 0x6C, 0xB4),
    Color32::from_rgb(0x66, 0x9A, 0xD8),
    Color32::from_rgb(0xD4, 0xA0, 0x17),
    Color32::from_rgb(0x5A, 0xB0, 0xA0),
];

/// Color for the language at `index` in a chart legend.
pub fn language_color(index: usize) -> Color32 {
    PALETTE[index % PALETTE.len()]
}

/// Card / panel background that adapts to the active theme.
pub fn card_fill(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0x1E, 0x22, 0x2B)
    } else {
        Color32::WHITE
    }
}

/// Hairline stroke used for card borders.
pub fn card_stroke(ui: &egui::Ui) -> Stroke {
    if ui.visuals().dark_mode {
        Stroke::new(1.0, Color32::from_rgb(0x2A, 0x30, 0x3B))
    } else {
        Stroke::new(1.0, Color32::from_rgb(0xE2, 0xE6, 0xEE))
    }
}

/// Muted text color for secondary information.
pub fn muted(ui: &egui::Ui) -> Color32 {
    ui.visuals().text_color().gamma_multiply(0.62)
}

/// A card container with a title.
pub fn card(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::group(ui.style())
        .fill(card_fill(ui))
        .stroke(card_stroke(ui))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(14, 12))
}

/// Apply the global visual style.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    if dark {
        visuals.panel_fill = Color32::from_rgb(0x14, 0x16, 0x1B);
        visuals.window_fill = Color32::from_rgb(0x1A, 0x1D, 0x24);
        visuals.faint_bg_color = Color32::from_rgb(0x1C, 0x20, 0x28);
        visuals.extreme_bg_color = Color32::from_rgb(0x10, 0x12, 0x17);
    } else {
        visuals.panel_fill = Color32::from_rgb(0xF4, 0xF6, 0xFA);
        visuals.window_fill = Color32::WHITE;
        visuals.faint_bg_color = Color32::from_rgb(0xEF, 0xF2, 0xF7);
        visuals.extreme_bg_color = Color32::WHITE;
    }

    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);

    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(7);
    }
    visuals.widgets.inactive.weak_bg_fill = if dark {
        Color32::from_rgb(0x25, 0x2A, 0x34)
    } else {
        Color32::from_rgb(0xEC, 0xEF, 0xF5)
    };
    visuals.widgets.hovered.weak_bg_fill = ACCENT.gamma_multiply(0.28);
    visuals.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.45);

    let border = if dark {
        Color32::from_rgb(0x39, 0x40, 0x4D)
    } else {
        Color32::from_rgb(0xD5, 0xDA, 0xE3)
    };
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.75));
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, border);
    visuals.striped = true;

    let theme = if dark { Theme::Dark } else { Theme::Light };
    ctx.set_theme(if dark {
        ThemePreference::Dark
    } else {
        ThemePreference::Light
    });
    ctx.set_visuals_of(theme, visuals);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style_of(theme, style);
}
