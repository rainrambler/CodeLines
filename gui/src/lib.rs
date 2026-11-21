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

/// Launch the native CodeLines window.
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 620.0])
            .with_title("CodeLines — Code Statistics"),
        ..Default::default()
    };

    eframe::run_native(
        "CodeLines",
        options,
        Box::new(|cc| Ok(Box::new(CodeLinesApp::new(cc)))),
    )
}
