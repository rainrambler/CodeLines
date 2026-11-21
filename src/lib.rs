//! codelines - Core library and CLI for code line statistics.
//!
//! The workspace contains two crates:
//!
//! * `codelines` — this crate: the reusable analysis library and the `codelines`
//!   command line interface (`cli` + `main.rs`).
//! * `codelines-gui` — the separate desktop front-end (`gui/`).
//!
//! Both front-ends share the same analysis pipeline exposed by [`report`].

pub mod analyzer;
pub mod cli;
pub mod language;
pub mod output;
pub mod report;
pub mod scanner;
