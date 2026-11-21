//! `codelines-gui` — desktop front-end for the CodeLines analyzer.

// Ship as a GUI-subsystem binary so double-clicking it in Explorer does not pop up
// a console window. Debug builds keep the console for logging.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    codelines_gui::run()
}
