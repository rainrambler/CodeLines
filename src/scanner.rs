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
    /// Ambiguous extension resolutions that used the default language
    pub ambiguity_counts: HashMap<String, usize>,
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
            ambiguity_counts: HashMap::new(),
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

    let allowed: Option<HashSet<String>> = if languages_filter.is_empty() {
        None
    } else {
        Some(
            languages_filter
                .iter()
                .map(|name| name.to_ascii_lowercase())
                .collect(),
        )
    };

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
            process_file(root_path, allowed.as_ref(), &mut result);
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
                        process_file(e.path(), allowed.as_ref(), &mut result);
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

    let mut notes: Vec<_> = result.ambiguity_counts.drain().collect();
    notes.sort();
    for (resolution, count) in notes {
        result.warnings.push(format!(
            "Detection: {count} file(s) used the default language for ambiguous extension \
             {resolution} (no content hint)"
        ));
    }

    result
}

/// Result of resolving a single path.
#[derive(Debug)]
enum Detection {
    Language(&'static Language),
    /// A shared extension was resolved; `hinted` is false when the preference
    /// table had to be used.
    Ambiguous {
        language: &'static Language,
        extension: String,
        hinted: bool,
    },
    /// The file was recognized but excluded by the language filter.
    Filtered,
    Unknown(String),
}

/// Process a single file: identify language and classify
fn process_file(path: &Path, allowed: Option<&HashSet<String>>, result: &mut ScanResult) {
    match detect_language(path, allowed) {
        Detection::Language(language) => push_file(result, path, language),
        Detection::Ambiguous {
            language,
            extension,
            hinted,
        } => {
            if !hinted {
                let key = format!("{extension} -> {}", language.name);
                *result.ambiguity_counts.entry(key).or_insert(0) += 1;
            }
            push_file(result, path, language);
        }
        Detection::Filtered => {}
        Detection::Unknown(extension) => {
            *result.unknown_extensions.entry(extension).or_insert(0) += 1;
        }
    }
}

fn push_file(result: &mut ScanResult, path: &Path, language: &Language) {
    result
        .files_by_language
        .entry(language.name.to_string())
        .or_default()
        .push(path.to_path_buf());
}

fn detect_language(path: &Path, allowed: Option<&HashSet<String>>) -> Detection {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    if let Some(language) = language::language_for_filename(&filename) {
        return if is_allowed(language, allowed) {
            Detection::Language(language)
        } else {
            Detection::Filtered
        };
    }

    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()));
    let mut sample: Option<String> = None;

    if let Some(extension) = &extension {
        let registry = language::registry();
        let indices = registry.extension_indices(extension);
        if indices.is_empty() {
            return Detection::Unknown(extension.clone());
        }

        let mut allowed_count = 0;
        let mut only_allowed = None;
        for &index in indices {
            let language = registry.language(index);
            if is_allowed(language, allowed) {
                allowed_count += 1;
                only_allowed = Some(language);
            }
        }

        match allowed_count {
            0 => return Detection::Filtered,
            1 => return Detection::Language(only_allowed.unwrap()),
            _ => {
                let candidates: Vec<&'static Language> = indices
                    .iter()
                    .map(|&index| registry.language(index))
                    .filter(|language| is_allowed(language, allowed))
                    .collect();
                let content = sample.get_or_insert_with(|| read_sample(path));
                if let Some((language, hinted)) =
                    language::resolve_ambiguous_extension(extension, &candidates, content)
                {
                    return Detection::Ambiguous {
                        language,
                        extension: extension.clone(),
                        hinted,
                    };
                }
            }
        }
    } else {
        // Only extensionless files are checked for a shebang; this keeps image
        // and binary files from being read unnecessarily.
        let content = sample.get_or_insert_with(|| read_sample(path));
        if let Some(language) = content
            .lines()
            .next()
            .and_then(language::language_for_shebang)
        {
            return if is_allowed(language, allowed) {
                Detection::Language(language)
            } else {
                Detection::Filtered
            };
        }
    }

    Detection::Unknown(extension.unwrap_or_else(|| "(no extension)".to_string()))
}

fn is_allowed(language: &Language, allowed: Option<&HashSet<String>>) -> bool {
    allowed.is_none_or(|allowed| allowed.contains(&language.name.to_ascii_lowercase()))
}

fn read_sample(path: &Path) -> String {
    use std::io::Read;

    let mut content = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        let _ = file.take(8192).read_to_end(&mut content);
    }
    String::from_utf8_lossy(&content).into_owned()
}

/// Read file content from disk.
///
/// Returns a human readable message on failure so callers can decide whether to
/// print it (CLI) or collect it as a diagnostic (GUI).
pub fn read_file_content(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|e| format!("Warning: unable to read file {}: {}", path.display(), e))
}
