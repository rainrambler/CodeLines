/// Result formatting and output module
use std::collections::HashMap;

use colored::*;

use crate::analyzer::LineStats;
use crate::language;

/// Aggregated statistics summary for one language
#[derive(Debug, Clone, Default)]
pub struct LanguageSummary {
    pub language: String,
    pub file_count: usize,
    pub total_lines: usize,
    pub code_lines: usize,
    pub comment_lines: usize,
    pub mixed_lines: usize,
    pub blank_lines: usize,
}

impl LanguageSummary {
    /// Effective code lines (pure code + mixed)
    pub fn effective_code(&self) -> usize {
        self.code_lines + self.mixed_lines
    }

    /// Total comment-related lines (pure comment + mixed)
    pub fn total_comment(&self) -> usize {
        self.comment_lines + self.mixed_lines
    }

    /// Code ratio
    pub fn code_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.effective_code() as f64 / self.total_lines as f64
        }
    }

    /// Comment ratio
    pub fn comment_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.total_comment() as f64 / self.total_lines as f64
        }
    }
}

/// Build LanguageSummary from per-language line statistics
pub fn build_summaries(
    stats_by_lang: &HashMap<String, LineStats>,
    file_counts: &HashMap<String, usize>,
) -> Vec<LanguageSummary> {
    let mut summaries: Vec<LanguageSummary> = stats_by_lang
        .iter()
        .map(|(lang, stats)| LanguageSummary {
            language: lang.clone(),
            file_count: *file_counts.get(lang).unwrap_or(&0),
            total_lines: stats.total_lines,
            code_lines: stats.code_lines,
            comment_lines: stats.comment_lines,
            mixed_lines: stats.mixed_lines,
            blank_lines: stats.blank_lines,
        })
        .collect();

    // Sort by effective code lines descending
    summaries.sort_by(|a, b| b.effective_code().cmp(&a.effective_code()));
    summaries
}

/// Sum all summaries into a single total row.
pub fn total_summary(summaries: &[LanguageSummary]) -> LanguageSummary {
    summaries
        .iter()
        .fold(LanguageSummary::default(), |acc, s| LanguageSummary {
            language: "Total".to_string(),
            file_count: acc.file_count + s.file_count,
            total_lines: acc.total_lines + s.total_lines,
            code_lines: acc.code_lines + s.code_lines,
            comment_lines: acc.comment_lines + s.comment_lines,
            mixed_lines: acc.mixed_lines + s.mixed_lines,
            blank_lines: acc.blank_lines + s.blank_lines,
        })
}

/// Colored terminal output
pub fn print_colored_table(summaries: &[LanguageSummary], verbose: bool) {
    let total = total_summary(summaries);

    // Header
    let header = if verbose {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
            "Language", "Files", "Total", "Code", "Comment", "Mixed", "Blank", "Code%", "Comment%"
        )
    } else {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
            "Language", "Files", "Total", "Eff.Code", "Comments", "Blank", "Code%", "Comment%"
        )
    };

    println!();
    println!("{}", header.bold().cyan());
    println!("{}", "─".repeat(header.len()).dimmed());

    // One row per language
    for s in summaries {
        let line = if verbose {
            format!(
                "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
                s.language,
                s.file_count,
                s.total_lines,
                s.code_lines,
                s.comment_lines,
                s.mixed_lines,
                s.blank_lines,
                s.code_ratio() * 100.0,
                s.comment_ratio() * 100.0,
            )
        } else {
            format!(
                "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
                s.language,
                s.file_count,
                s.total_lines,
                s.effective_code(),
                s.total_comment(),
                s.blank_lines,
                s.code_ratio() * 100.0,
                s.comment_ratio() * 100.0,
            )
        };
        println!("{}", line);
    }

    // Totals row
    println!("{}", "─".repeat(header.len()).dimmed());
    let total_line = if verbose {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
            total.language.bold(),
            total.file_count.to_string().bold(),
            total.total_lines.to_string().bold(),
            total.code_lines.to_string().bold(),
            total.comment_lines.to_string().bold(),
            total.mixed_lines.to_string().bold(),
            total.blank_lines.to_string().bold(),
            total.code_ratio() * 100.0,
            total.comment_ratio() * 100.0,
        )
    } else {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
            total.language.bold(),
            total.file_count.to_string().bold(),
            total.total_lines.to_string().bold(),
            total.effective_code().to_string().bold(),
            total.total_comment().to_string().bold(),
            total.blank_lines.to_string().bold(),
            total.code_ratio() * 100.0,
            total.comment_ratio() * 100.0,
        )
    };
    println!("{}", total_line);
    println!();
}

/// Plain text table (no ANSI control characters).
pub fn format_plain_table(summaries: &[LanguageSummary], verbose: bool) -> String {
    let total = total_summary(summaries);
    let header = plain_header(verbose);

    let mut out = String::new();
    out.push_str(&header);
    out.push('\n');
    out.push_str(&"-".repeat(header.len()));
    out.push('\n');

    for s in summaries {
        push_plain_row(&mut out, s, verbose);
    }

    out.push_str(&"-".repeat(header.len()));
    out.push('\n');
    push_plain_row(&mut out, &total, verbose);
    out
}

fn plain_header(verbose: bool) -> String {
    if verbose {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
            "Language", "Files", "Total", "Code", "Comment", "Mixed", "Blank", "Code%", "Comment%"
        )
    } else {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
            "Language", "Files", "Total", "Eff.Code", "Comments", "Blank", "Code%", "Comment%"
        )
    }
}

fn push_plain_row(out: &mut String, s: &LanguageSummary, verbose: bool) {
    let line = if verbose {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
            s.language,
            s.file_count,
            s.total_lines,
            s.code_lines,
            s.comment_lines,
            s.mixed_lines,
            s.blank_lines,
            s.code_ratio() * 100.0,
            s.comment_ratio() * 100.0,
        )
    } else {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
            s.language,
            s.file_count,
            s.total_lines,
            s.effective_code(),
            s.total_comment(),
            s.blank_lines,
            s.code_ratio() * 100.0,
            s.comment_ratio() * 100.0,
        )
    };
    out.push_str(&line);
    out.push('\n');
}

/// Print the plain text table (no colors, suitable for piping to a file)
pub fn print_plain_table(summaries: &[LanguageSummary], verbose: bool) {
    print!("{}", format_plain_table(summaries, verbose));
}

/// CSV table.
pub fn format_csv_table(summaries: &[LanguageSummary]) -> String {
    let mut out = String::from(
        "Language,Files,TotalLines,CodeLines,CommentLines,MixedLines,BlankLines,EffectiveCode,TotalComment,CodeRatio,CommentRatio\n",
    );

    for s in summaries {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.4},{:.4}\n",
            s.language,
            s.file_count,
            s.total_lines,
            s.code_lines,
            s.comment_lines,
            s.mixed_lines,
            s.blank_lines,
            s.effective_code(),
            s.total_comment(),
            s.code_ratio(),
            s.comment_ratio(),
        ));
    }

    out
}

/// Print the CSV table.
pub fn print_csv_table(summaries: &[LanguageSummary]) {
    print!("{}", format_csv_table(summaries));
}

/// Print the list of all supported languages
pub fn print_supported_languages() {
    println!("\nSupported languages and file extensions:\n");
    for lang in language::supported_languages() {
        let exts = lang.extensions.join(", ");
        println!("  {:<24} {}", format!("[{}]", lang.name), exts);
    }
    println!();
}

/// Print unknown extension statistics (expects entries sorted by count, descending)
pub fn print_unknown_extensions(unknown: &[(String, usize)]) {
    if unknown.is_empty() {
        return;
    }
    println!("{}", "Unrecognized file types (skipped):".dimmed());
    for (ext, count) in unknown {
        println!("  {} {} file(s)", ext, count);
    }
    println!();
}
