/// codelines - Code line statistics tool
///
/// Supports multi-language per-language line counting, with comment analysis.
/// Intelligently detects line comments and block comments, correctly handling
/// comment-like markers inside string literals.
use std::path::PathBuf;

use clap::Parser;
use colored::Colorize;

use codelines::cli::Args;
use codelines::output::*;
use codelines::report::{self, ScanOptions};

fn main() {
    let args = Args::parse();

    // List supported languages
    if args.list {
        print_supported_languages();
        return;
    }

    let options = ScanOptions {
        paths: args.paths.iter().map(PathBuf::from).collect(),
        languages: args.languages.clone(),
        exclude_dirs: args.exclude_dirs.clone(),
        exclude_files: args.exclude_files.clone(),
    };

    let report = report::analyze(&options, None, |_| {});

    for warning in &report.warnings {
        eprintln!("{}", warning);
    }

    // If no code files were found
    if report.is_empty() {
        if report.total_files_scanned == 0 {
            eprintln!("No files found. Please check the path(s).");
        } else {
            eprintln!("No recognizable code files found. Use --list to see supported languages.");
        }
        std::process::exit(1);
    }

    // Output results
    if args.csv {
        print_csv_table(&report.summaries);
    } else if args.plain {
        print_plain_table(&report.summaries, args.verbose);
    } else {
        print_colored_table(&report.summaries, args.verbose);
    }

    // Output per-file detailed statistics
    if args.files {
        println!("{}", "Per-file detailed statistics:".bold());
        println!();

        for summary in &report.summaries {
            let mut details: Vec<_> = report
                .files
                .iter()
                .filter(|file| file.language == summary.language)
                .collect();

            println!("  {} ({} files)", summary.language.bold(), details.len());

            // Sort by effective code lines descending
            details.sort_by(|a, b| {
                b.stats
                    .effective_code_lines()
                    .cmp(&a.stats.effective_code_lines())
            });

            for file in details {
                println!(
                    "    {:<50} {:>6} lines  code:{:>5}  comment:{:>5}  mixed:{:>4}  blank:{:>4}",
                    file.path.to_string_lossy(),
                    file.stats.total_lines,
                    file.stats.code_lines,
                    file.stats.comment_lines,
                    file.stats.mixed_lines,
                    file.stats.blank_lines,
                );
            }
            println!();
        }
    }

    // Print unknown file types
    if !report.unknown_extensions.is_empty() {
        print_unknown_extensions(&report.unknown_extensions);
    }

    // Print scan summary
    let total_files: usize = report.summaries.iter().map(|s| s.file_count).sum();
    let total_langs = report.summaries.len();
    println!(
        "{} languages, {} source files, {} files scanned total",
        total_langs.to_string().green().bold(),
        total_files.to_string().green().bold(),
        report.total_files_scanned.to_string().dimmed(),
    );
}
