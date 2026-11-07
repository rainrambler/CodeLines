//! codelines - Core library for code line statistics.
//!
//! The crate is split into a reusable library and two front-ends:
//!
//! * `codelines` — the command line interface ([`cli`] + `main.rs`).
//! * `codelines-gui` — the desktop GUI (see [`gui`], enabled by the `gui` feature).
//!
//! Both front-ends share the same analysis pipeline exposed by [`report`].

pub mod analyzer;
pub mod cli;
pub mod language;
pub mod output;
pub mod report;
pub mod scanner;

#[cfg(feature = "gui")]
pub mod gui;
