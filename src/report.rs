//! Shared analysis pipeline used by both the CLI and the GUI.
//!
//! [`analyze`] scans the requested paths, analyzes every recognized source file
//! and returns a [`Report`] containing per-language summaries, per-file details
//! and diagnostics. Progress can be observed through the [`Progress`] callback,
//! and long running scans can be aborted with a cancellation flag.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::analyzer::{LineStats, analyze_content};
use crate::language;
use crate::output::{LanguageSummary, build_summaries, total_summary};
use crate::scanner::{read_file_content, scan_directory_with_progress};

/// Time between two progress notifications.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// Options controlling a scan.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Directories (or single files) to analyze.
    pub paths: Vec<PathBuf>,
    /// Restrict the scan to these language names (empty = all languages).
    pub languages: Vec<String>,
    /// Extra directory names to skip.
    pub exclude_dirs: Vec<String>,
    /// Extra file names to skip.
    pub exclude_files: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            paths: vec![PathBuf::from(".")],
            languages: Vec::new(),
            exclude_dirs: Vec::new(),
            exclude_files: Vec::new(),
        }
    }
}

/// A single analyzed file.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub language: String,
    pub stats: LineStats,
}

/// Progress notification emitted while [`analyze`] runs.
#[derive(Debug, Clone)]
pub enum Progress {
    /// Walking the file system; `files_seen` files have been visited so far.
    Scanning {
        files_seen: usize,
        current: Option<PathBuf>,
    },
    /// Reading and analyzing recognized source files.
    Analyzing {
        done: usize,
        total: usize,
        current: Option<PathBuf>,
    },
}

/// Result of a full analysis run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// One summary per language, sorted by effective code lines (descending).
    pub summaries: Vec<LanguageSummary>,
    /// Per-file details for every analyzed file.
    pub files: Vec<FileEntry>,
    /// Number of files visited during the walk (recognized or not).
    pub total_files_scanned: usize,
    /// Unrecognized file extensions and how often they occurred (sorted desc).
    pub unknown_extensions: Vec<(String, usize)>,
    /// Non-fatal problems encountered while walking or reading files.
    pub warnings: Vec<String>,
    /// Wall-clock time spent analyzing.
    pub elapsed: Duration,
    /// The analyzed roots.
    pub roots: Vec<PathBuf>,
    /// True when the run was aborted through the cancellation flag.
    pub cancelled: bool,
}

impl Report {
    /// Aggregate of all language summaries.
    pub fn total(&self) -> LanguageSummary {
        let mut total = total_summary(&self.summaries);
        total.language = "Total".to_string();
        total
    }

    /// True when no recognized source file was found.
    pub fn is_empty(&self) -> bool {
        self.summaries.is_empty()
    }
}

/// Analyze the given paths and build a [`Report`].
///
/// `progress` is called at most every [`PROGRESS_INTERVAL`] while scanning and
/// analyzing. When `cancel` is set to `true` the analysis stops as soon as
/// possible and the partial report is returned with `cancelled = true`.
pub fn analyze(
    options: &ScanOptions,
    cancel: Option<&AtomicBool>,
    mut progress: impl FnMut(Progress),
) -> Report {
    let started = Instant::now();
    let is_cancelled = || cancel.is_some_and(|flag| flag.load(Ordering::Relaxed));

    let mut files_seen = 0usize;
    let mut last_emit = started;
    let scan = scan_directory_with_progress(
        &options.paths,
        &options.exclude_dirs,
        &options.exclude_files,
        &options.languages,
        |path| {
            files_seen += 1;
            if files_seen == 1 || last_emit.elapsed() >= PROGRESS_INTERVAL {
                last_emit = Instant::now();
                progress(Progress::Scanning {
                    files_seen,
                    current: Some(path.to_path_buf()),
                });
            }
        },
    );

    let lang_map: HashMap<&'static str, language::Language> = language::supported_languages()
        .into_iter()
        .map(|lang| (lang.name, lang))
        .collect();

    // Deterministic processing order so progress reporting is stable.
    let mut languages: Vec<&String> = scan.files_by_language.keys().collect();
    languages.sort();

    let total: usize = scan.files_by_language.values().map(Vec::len).sum();
    let mut done = 0usize;

    let mut stats_by_lang: HashMap<String, LineStats> = HashMap::new();
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut files: Vec<FileEntry> = Vec::with_capacity(total);
    let mut warnings = scan.warnings.clone();
    let mut cancelled = false;

    progress(Progress::Analyzing {
        done: 0,
        total,
        current: None,
    });

    'outer: for lang_name in languages {
        let lang_def = match lang_map.get(lang_name.as_str()) {
            Some(lang) => lang,
            None => continue,
        };

        let paths = &scan.files_by_language[lang_name];
        file_counts.insert(lang_name.clone(), paths.len());

        let mut lang_stats = LineStats::default();
        for path in paths {
            if is_cancelled() {
                cancelled = true;
                break 'outer;
            }

            match read_file_content(path) {
                Ok(content) => {
                    let stats = analyze_content(&content, &lang_def.comment_rule);
                    lang_stats += stats.clone();
                    files.push(FileEntry {
                        path: path.clone(),
                        language: lang_name.clone(),
                        stats,
                    });
                }
                Err(message) => warnings.push(message),
            }

            done += 1;
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                last_emit = Instant::now();
                progress(Progress::Analyzing {
                    done,
                    total,
                    current: Some(path.clone()),
                });
            }
        }

        stats_by_lang.insert(lang_name.clone(), lang_stats);
    }

    let mut unknown_extensions: Vec<(String, usize)> =
        scan.unknown_extensions.into_iter().collect();
    unknown_extensions.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    Report {
        summaries: build_summaries(&stats_by_lang, &file_counts),
        files,
        total_files_scanned: scan.total_files_scanned,
        unknown_extensions,
        warnings,
        elapsed: started.elapsed(),
        roots: options.paths.clone(),
        cancelled,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicBool;

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("codelines-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("src/main.rs"),
            "// comment\nfn main() {\n    let value = 1; // inline\n\n}\n",
        )
        .unwrap();
        fs::write(dir.join("src/util.py"), "# comment\nprint('hi')\n").unwrap();
        fs::write(dir.join("notes.txt"), "hello\n").unwrap();
        dir
    }

    #[test]
    fn groups_files_by_language_and_classifies_lines() {
        let dir = fixture("report");
        let options = ScanOptions {
            paths: vec![dir.clone()],
            ..Default::default()
        };

        let mut progress_events = 0;
        let report = analyze(&options, None, |_| progress_events += 1);

        assert!(progress_events > 0, "progress callback should fire");
        assert_eq!(report.summaries.len(), 2);

        let rust = report
            .summaries
            .iter()
            .find(|summary| summary.language == "Rust")
            .expect("Rust summary");
        assert_eq!(rust.file_count, 1);
        assert_eq!(rust.total_lines, 5);
        assert_eq!(rust.code_lines, 2);
        assert_eq!(rust.comment_lines, 1);
        assert_eq!(rust.mixed_lines, 1);
        assert_eq!(rust.blank_lines, 1);

        let python = report
            .summaries
            .iter()
            .find(|summary| summary.language == "Python")
            .expect("Python summary");
        assert_eq!(python.comment_lines, 1);
        assert_eq!(python.code_lines, 1);

        assert_eq!(report.files.len(), 2);
        assert!(
            report
                .unknown_extensions
                .iter()
                .any(|(extension, count)| extension == ".txt" && *count == 1)
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn honours_language_filter() {
        let dir = fixture("filter");
        let options = ScanOptions {
            paths: vec![dir.clone()],
            languages: vec!["Rust".to_string()],
            ..Default::default()
        };

        let report = analyze(&options, None, |_| {});
        assert_eq!(report.summaries.len(), 1);
        assert_eq!(report.summaries[0].language, "Rust");
        assert_eq!(report.files.len(), 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn report_can_be_cancelled() {
        let dir = fixture("cancel");
        let options = ScanOptions {
            paths: vec![dir.clone()],
            ..Default::default()
        };
        let cancel = AtomicBool::new(true);

        let report = analyze(&options, Some(&cancel), |_| {});
        assert!(report.cancelled);
        assert!(report.files.is_empty());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_paths_are_reported_as_warnings() {
        let options = ScanOptions {
            paths: vec![PathBuf::from("/definitely/not/a/real/path")],
            ..Default::default()
        };
        let report = analyze(&options, None, |_| {});
        assert!(report.is_empty());
        assert_eq!(report.total_files_scanned, 0);
        assert_eq!(report.warnings.len(), 1);
    }
}
