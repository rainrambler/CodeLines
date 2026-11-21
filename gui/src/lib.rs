//! Desktop GUI front-end for CodeLines.
//!
//! This is a separate crate from the `codelines` library/CLI: it depends on the
//! public analysis pipeline ([`codelines::report`]) and runs it on a background
//! thread, so scanning a large project never blocks the window.

mod app;
mod charts;
mod export;
mod table;
mod theme;
mod widgets;
mod worker;

pub use app::CodeLinesApp;

/// Window and taskbar icon, embedded from the generated asset set.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");

/// Launch the native CodeLines window.
pub fn run() -> eframe::Result<()> {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([980.0, 620.0])
        .with_title("CodeLines — Code Statistics");

    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "CodeLines",
        options,
        Box::new(|cc| Ok(Box::new(CodeLinesApp::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_icon_decodes() {
        let icon = eframe::icon_data::from_png_bytes(ICON_PNG).expect("icon-256.png is valid PNG");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}
