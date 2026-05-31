/// 文件遍历与识别模块：递归扫描目录，按语言分类收集文件
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::language::{self, Language};
use walkdir::WalkDir;

/// 默认忽略的目录名
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

/// 默认忽略的文件名
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

/// 扫描结果：按语言分组收集到的文件路径列表
pub struct ScanResult {
    /// 语言名称 → 文件路径列表
    pub files_by_language: HashMap<String, Vec<PathBuf>>,
    /// 未识别的文件扩展名集合
    pub unknown_extensions: HashMap<String, usize>,
    /// 忽略的目录数
    pub skipped_dirs: usize,
    /// 总扫描文件数
    pub total_files_scanned: usize,
}

impl ScanResult {
    pub fn new() -> Self {
        ScanResult {
            files_by_language: HashMap::new(),
            unknown_extensions: HashMap::new(),
            skipped_dirs: 0,
            total_files_scanned: 0,
        }
    }
}

/// 递归扫描指定路径，按语言分类收集源代码文件
pub fn scan_directory(
    paths: &[PathBuf],
    ignore_dirs: &[String],
    ignore_files: &[String],
    languages_filter: &[String],
) -> ScanResult {
    let mut result = ScanResult::new();
    let all_langs = language::supported_languages();

    // 构建过滤集合
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

    // 构建语言过滤：如果用户指定了语言，则只包含那些语言
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

    // 构建扩展名 → 语言名称的快速查找表
    let mut ext_to_lang: HashMap<String, String> = HashMap::new();
    for lang in &filtered_langs {
        for ext in &lang.extensions {
            ext_to_lang.insert(ext.to_lowercase(), lang.name.to_string());
        }
    }

    for root_path in paths {
        if !root_path.exists() {
            eprintln!("警告：路径不存在，已跳过: {}", root_path.display());
            continue;
        }

        // 如果是文件，直接处理
        if root_path.is_file() {
            result.total_files_scanned += 1;
            process_file(root_path, &ext_to_lang, &mut result);
            continue;
        }

        // 如果是目录，递归遍历
        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // 过滤要忽略的目录
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
                        // 过滤要忽略的文件
                        let file_name = e.file_name().to_string_lossy();
                        if ignore_file_set.iter().any(|f| file_name == f.as_str()) {
                            continue;
                        }

                        result.total_files_scanned += 1;
                        process_file(e.path(), &ext_to_lang, &mut result);
                    }
                }
                Err(err) => {
                    eprintln!("警告：无法访问文件: {}", err);
                }
            }
        }
    }

    result
}

/// 处理单个文件：识别语言并归类
fn process_file(
    path: &Path,
    ext_to_lang: &HashMap<String, String>,
    result: &mut ScanResult,
) {
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
                    *result
                        .unknown_extensions
                        .entry(ext_str)
                        .or_insert(0) += 1;
                }
            }
        }
        None => {
            // 没有扩展名的文件，跳过
            *result
                .unknown_extensions
                .entry("(无扩展名)".to_string())
                .or_insert(0) += 1;
        }
    }
}

/// 读取文件内容
pub fn read_file_content(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(e) => {
            eprintln!("警告：无法读取文件 {}: {}", path.display(), e);
            None
        }
    }
}
