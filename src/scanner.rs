/// File traversal and identification module: recursively scans directories,
/// grouping files by language.
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::language::{self, Language};
use ignore::WalkBuilder;

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
///
/// When `respect_gitignore` is true, paths matched by `.gitignore` files are
/// skipped (including `.gitignore` files in parent directories of the roots).
pub fn scan_directory(
    paths: &[PathBuf],
    ignore_dirs: &[String],
    ignore_files: &[String],
    languages_filter: &[String],
    respect_gitignore: bool,
) -> ScanResult {
    scan_directory_with_progress(
        paths,
        ignore_dirs,
        ignore_files,
        languages_filter,
        respect_gitignore,
        |_| {},
    )
}

/// Same as [`scan_directory`], but calls `on_file` for every visited file,
/// which allows front-ends to display live progress.
pub fn scan_directory_with_progress(
    paths: &[PathBuf],
    ignore_dirs: &[String],
    ignore_files: &[String],
    languages_filter: &[String],
    respect_gitignore: bool,
    mut on_file: impl FnMut(&Path),
) -> ScanResult {
    let mut result = ScanResult::new();
    let all_langs = language::supported_languages();

    // Build filter sets
    let ignore_dir_set: HashSet<String> = DEFAULT_IGNORE_DIRS
        .iter()
        .map(|s| s.to_string())
        .chain(ignore_dirs.iter().cloned())
        .collect();

    let ignore_file_set: HashSet<String> = DEFAULT_IGNORE_FILES
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

    // Shared between the per-root walkers; `ignore`'s filter callback must be
    // `'static`, so the sets are reference-counted rather than borrowed.
    let ignore_dir_set = Arc::new(ignore_dir_set);
    let ignore_file_set = Arc::new(ignore_file_set);

    for root_path in paths {
        if !root_path.exists() {
            result.warnings.push(format!(
                "Warning: path does not exist, skipping: {}",
                root_path.display()
            ));
            continue;
        }

        // If it's a file, process directly (an explicitly named file is always
        // analyzed, regardless of ignore rules)
        if root_path.is_file() {
            result.total_files_scanned += 1;
            on_file(root_path);
            process_file(root_path, &ext_to_lang, &mut result);
            continue;
        }

        // If it's a directory, walk recursively
        let dirs = Arc::clone(&ignore_dir_set);
        let files = Arc::clone(&ignore_file_set);
        let walker = WalkBuilder::new(root_path)
            // Hidden files are counted; only the named ignore lists below apply.
            .hidden(false)
            // `.ignore` files are a ripgrep convention, not git's.
            .ignore(false)
            .git_global(false)
            .git_exclude(false)
            .git_ignore(respect_gitignore)
            // Honour `.gitignore` even when the tree is not a git repository.
            .require_git(false)
            // Read `.gitignore` files from parent directories of the roots too.
            .parents(respect_gitignore)
            .filter_entry(move |entry| {
                let name = entry.file_name().to_string_lossy();
                if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                    !dirs.contains(name.as_ref())
                } else {
                    !files.contains(name.as_ref())
                }
            })
            .build();

        for entry in walker {
            match entry {
                Ok(e) => {
                    if e.file_type().is_some_and(|ft| ft.is_file()) {
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
