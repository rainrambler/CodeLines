/// codelines - Code line statistics tool
///
/// Supports multi-language per-language line counting, with comment analysis.
/// Intelligently detects line comments and block comments, correctly handling
/// comment-like markers inside string literals.

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

    // List supported languages
    if args.list {
        print_supported_languages();
        return;
    }

    // Parse paths
    let paths: Vec<PathBuf> = args
        .paths
        .iter()
        .map(|p| PathBuf::from(p))
        .collect();

    // Scan files
    let scan_result = scan_directory(
        &paths,
        &args.exclude_dirs,
        &args.exclude_files,
        &args.languages,
    );

    // If no code files were found
    if scan_result.files_by_language.is_empty() {
        if scan_result.total_files_scanned == 0 {
            eprintln!("No files found. Please check the path(s).");
        } else {
            eprintln!("No recognizable code files found. Use --list to see supported languages.");
        }
        std::process::exit(1);
    }

    // Build language name → language definition mapping
    let lang_map: HashMap<String, language::Language> = supported_languages()
        .into_iter()
        .map(|l| (l.name.to_string(), l))
        .collect();

    // Per-language statistics
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

    // Build summaries
    let summaries = build_summaries(&stats_by_lang, &file_counts);

    // Output results
    if args.csv {
        print_csv_table(&summaries);
    } else if args.plain {
        print_plain_table(&summaries, args.verbose);
    } else {
        print_colored_table(&summaries, args.verbose);
    }

    // Output per-file detailed statistics
    if args.files {
        println!("{}", "Per-file detailed statistics:".bold());
        println!();

        // Get language name ordering (consistent with main table)
        let lang_order: Vec<String> = summaries.iter().map(|s| s.language.clone()).collect();

        for lang_name in &lang_order {
            if let Some(details) = file_details.get(lang_name) {
                println!("  {} ({} files)", lang_name.bold(), details.len());

                // Sort by effective code lines descending
                let mut sorted_details = details.clone();
                sorted_details.sort_by(|a, b| b.1.effective_code_lines().cmp(&a.1.effective_code_lines()));

                for (path, stats) in &sorted_details {
                    let rel_path = path.to_string_lossy();
                    println!(
                        "    {:<50} {:>6} lines  code:{:>5}  comment:{:>5}  mixed:{:>4}  blank:{:>4}",
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

    // Print unknown file types
    if !scan_result.unknown_extensions.is_empty() {
        print_unknown_extensions(&scan_result.unknown_extensions);
    }

    // Print scan summary
    let total_files: usize = file_counts.values().sum();
    let total_langs = stats_by_lang.len();
    println!(
        "{} languages, {} source files, {} files scanned total",
        total_langs.to_string().green().bold(),
        total_files.to_string().green().bold(),
        scan_result.total_files_scanned.to_string().dimmed(),
    );
}
