/// File traversal and identification module: recursively scans directories,
/// grouping files by language.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::language::{self, Language};
use walkdir::WalkDir;

/// Default directory names to ignore
const DEFAULT_IGNORE_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
    ".idea",
    ".vscode",
    ".vs",
    "__pycache__",
    ".next",
    ".nuxt",
    "vendor",
    "Pods",
    ".gradle",
    ".cache",
    "bazel-bin",
    "bazel-out",
    "bazel-testlogs",
    ".dart_tool",
    ".fvm",
    "cmake-build-debug",
    "cmake-build-release",
];

/// Default file names to ignore
const DEFAULT_IGNORE_FILES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "Gemfile.lock",
    "poetry.lock",
    "pdm.lock",
    ".DS_Store",
    "Thumbs.db",
];

/// Scan result: file paths grouped by language
pub struct ScanResult {
    /// Language name → list of file paths
    pub files_by_language: HashMap<String, Vec<PathBuf>>,
    /// Unrecognized file extensions and their counts
    pub unknown_extensions: HashMap<String, usize>,
    /// Number of skipped directories
    pub skipped_dirs: usize,
    /// Total files scanned
    pub total_files_scanned: usize,
    /// Non-fatal problems (missing paths, unreadable entries)
    pub warnings: Vec<String>,
}

impl ScanResult {
    pub fn new() -> Self {
        ScanResult {
            files_by_language: HashMap::new(),
            unknown_extensions: HashMap::new(),
            skipped_dirs: 0,
            total_files_scanned: 0,
            warnings: Vec::new(),
        }
    }
}

impl Default for ScanResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursively scan the given paths, collecting source files grouped by language.
pub fn scan_directory(
    paths: &[PathBuf],
    ignore_dirs: &[String],
    ignore_files: &[String],
    languages_filter: &[String],
) -> ScanResult {
    scan_directory_with_progress(paths, ignore_dirs, ignore_files, languages_filter, |_| {})
}

/// Same as [`scan_directory`], but calls `on_file` for every visited file,
/// which allows front-ends to display live progress.
pub fn scan_directory_with_progress(
    paths: &[PathBuf],
    ignore_dirs: &[String],
    ignore_files: &[String],
    languages_filter: &[String],
    mut on_file: impl FnMut(&Path),
) -> ScanResult {
    let mut result = ScanResult::new();
    let all_langs = language::supported_languages();

    // Build filter sets
    let ignore_dir_set: Vec<String> = DEFAULT_IGNORE_DIRS
        .iter()
        .map(|s| s.to_string())
        .chain(ignore_dirs.iter().cloned())
        .collect();

    let ignore_file_set: Vec<String> = DEFAULT_IGNORE_FILES
        .iter()
        .map(|s| s.to_string())
        .chain(ignore_files.iter().cloned())
        .collect();

    // Build language filter: if user specified languages, only include those
    let filtered_langs: Vec<Language> = if languages_filter.is_empty() {
        all_langs
    } else {
        all_langs
            .into_iter()
            .filter(|l| {
                languages_filter
                    .iter()
                    .any(|f| l.name.eq_ignore_ascii_case(f))
            })
            .collect()
    };

    // Build extension → language name lookup table
    let mut ext_to_lang: HashMap<String, String> = HashMap::new();
    for lang in &filtered_langs {
        for ext in &lang.extensions {
            ext_to_lang.insert(ext.to_lowercase(), lang.name.to_string());
        }
    }

    for root_path in paths {
        if !root_path.exists() {
            result.warnings.push(format!(
                "Warning: path does not exist, skipping: {}",
                root_path.display()
            ));
            continue;
        }

        // If it's a file, process directly
        if root_path.is_file() {
            result.total_files_scanned += 1;
            on_file(root_path);
            process_file(root_path, &ext_to_lang, &mut result);
            continue;
        }

        // If it's a directory, walk recursively
        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Filter out directories to ignore
                if e.file_type().is_dir() {
                    let name = e.file_name().to_string_lossy();
                    if ignore_dir_set.iter().any(|d| name == d.as_str()) {
                        return false;
                    }
                }
                true
            })
        {
            match entry {
                Ok(e) => {
                    if e.file_type().is_file() {
                        // Filter out files to ignore
                        let file_name = e.file_name().to_string_lossy();
                        if ignore_file_set.iter().any(|f| file_name == f.as_str()) {
                            continue;
                        }

                        result.total_files_scanned += 1;
                        on_file(e.path());
                        process_file(e.path(), &ext_to_lang, &mut result);
                    }
                }
                Err(err) => {
                    result
                        .warnings
                        .push(format!("Warning: unable to access: {}", err));
                }
            }
        }
    }

    result
}

/// Process a single file: identify language and classify
fn process_file(path: &Path, ext_to_lang: &HashMap<String, String>, result: &mut ScanResult) {
    match path.extension() {
        Some(ext_os) => {
            let ext_str = format!(".{}", ext_os.to_string_lossy());
            let ext_lower = ext_str.to_lowercase();

            match ext_to_lang.get(&ext_lower) {
                Some(lang_name) => {
                    result
                        .files_by_language
                        .entry(lang_name.clone())
                        .or_default()
                        .push(path.to_path_buf());
                }
                None => {
                    *result.unknown_extensions.entry(ext_str).or_insert(0) += 1;
                }
            }
        }
        None => {
            // Files without an extension, skip
            *result
                .unknown_extensions
                .entry("(no extension)".to_string())
                .or_insert(0) += 1;
        }
    }
}

/// Read file content from disk.
///
/// Returns a human readable message on failure so callers can decide whether to
/// print it (CLI) or collect it as a diagnostic (GUI).
pub fn read_file_content(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|e| format!("Warning: unable to read file {}: {}", path.display(), e))
}
