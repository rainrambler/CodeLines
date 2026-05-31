/// codelines - 代码量统计工具
///
/// 支持多种编程语言分别统计，注释单独统计。
/// 智能识别行注释、块注释，正确处理字符串中的注释标记。

mod analyzer;
mod cli;
mod language;
mod output;
mod scanner;

use std::collections::HashMap;
use std::path::PathBuf;

use clap::Parser;
use colored::Colorize;

use analyzer::{analyze_content, LineStats};
use cli::Args;
use language::supported_languages;
use output::*;
use scanner::{read_file_content, scan_directory};

fn main() {
    let args = Args::parse();

    // 列出支持的语言
    if args.list {
        print_supported_languages();
        return;
    }

    // 解析路径
    let paths: Vec<PathBuf> = args
        .paths
        .iter()
        .map(|p| PathBuf::from(p))
        .collect();

    // 扫描文件
    let scan_result = scan_directory(
        &paths,
        &args.exclude_dirs,
        &args.exclude_files,
        &args.languages,
    );

    // 如果没有找到任何代码文件
    if scan_result.files_by_language.is_empty() {
        if scan_result.total_files_scanned == 0 {
            eprintln!("未找到任何文件。请检查路径是否正确。");
        } else {
            eprintln!("未找到可识别的代码文件。使用 --list 查看支持的语言。");
        }
        std::process::exit(1);
    }

    // 构建语言名称 → 语言定义的映射
    let lang_map: HashMap<String, language::Language> = supported_languages()
        .into_iter()
        .map(|l| (l.name.to_string(), l))
        .collect();

    // 按语言统计
    let mut stats_by_lang: HashMap<String, LineStats> = HashMap::new();
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut file_details: HashMap<String, Vec<(PathBuf, LineStats)>> = HashMap::new();

    for (lang_name, files) in &scan_result.files_by_language {
        let lang_def = match lang_map.get(lang_name) {
            Some(l) => l,
            None => continue,
        };

        file_counts.insert(lang_name.clone(), files.len());

        let mut lang_stats = LineStats::default();
        let mut details = Vec::new();

        for file_path in files {
            match read_file_content(file_path) {
                Some(content) => {
                    let stats = analyze_content(&content, &lang_def.comment_rule);
                    if args.files {
                        details.push((file_path.clone(), stats.clone()));
                    }
                    lang_stats += stats;
                }
                None => continue,
            }
        }

        stats_by_lang.insert(lang_name.clone(), lang_stats);
        if args.files {
            file_details.insert(lang_name.clone(), details);
        }
    }

    // 构建汇总
    let summaries = build_summaries(&stats_by_lang, &file_counts);

    // 输出结果
    if args.csv {
        print_csv_table(&summaries);
    } else if args.plain {
        print_plain_table(&summaries, args.verbose);
    } else {
        print_colored_table(&summaries, args.verbose);
    }

    // 输出每个文件的详细统计
    if args.files {
        println!("{}", "按文件详细统计：".bold());
        println!();

        // 获取语言名排序（与主表一致）
        let lang_order: Vec<String> = summaries.iter().map(|s| s.language.clone()).collect();

        for lang_name in &lang_order {
            if let Some(details) = file_details.get(lang_name) {
                println!("  {} ({} 个文件)", lang_name.bold(), details.len());

                // 按有效代码行数降序
                let mut sorted_details = details.clone();
                sorted_details.sort_by(|a, b| b.1.effective_code_lines().cmp(&a.1.effective_code_lines()));

                for (path, stats) in &sorted_details {
                    let rel_path = path.to_string_lossy();
                    println!(
                        "    {:<50} {:>6} 行  代码:{:>5}  注释:{:>5}  混合:{:>4}  空白:{:>4}",
                        rel_path,
                        stats.total_lines,
                        stats.code_lines,
                        stats.comment_lines,
                        stats.mixed_lines,
                        stats.blank_lines,
                    );
                }
                println!();
            }
        }
    }

    // 打印未知文件类型
    if !scan_result.unknown_extensions.is_empty() {
        print_unknown_extensions(&scan_result.unknown_extensions);
    }

    // 打印扫描概要
    let total_files: usize = file_counts.values().sum();
    let total_langs = stats_by_lang.len();
    println!(
        "{} 个语言，{} 个文件，共扫描 {} 个文件",
        total_langs.to_string().green().bold(),
        total_files.to_string().green().bold(),
        scan_result.total_files_scanned.to_string().dimmed(),
    );
}
