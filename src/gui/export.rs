//! Export helpers: CSV, JSON and plain text, plus platform specific "reveal".

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::output::{format_csv_table, format_plain_table};
use crate::report::Report;

use super::widgets::{format_duration, format_number};

/// Available export formats.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    SummaryCsv,
    FilesCsv,
    PlainText,
    Json,
}

impl ExportKind {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::SummaryCsv => "codelines-summary.csv",
            Self::FilesCsv => "codelines-files.csv",
            Self::PlainText => "codelines-report.txt",
            Self::Json => "codelines-report.json",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::SummaryCsv | Self::FilesCsv => "csv",
            Self::PlainText => "txt",
            Self::Json => "json",
        }
    }

    fn filter_name(self) -> &'static str {
        match self {
            Self::SummaryCsv | Self::FilesCsv => "CSV",
            Self::PlainText => "Text",
            Self::Json => "JSON",
        }
    }

    pub fn content(self, report: &Report) -> String {
        match self {
            Self::SummaryCsv => summary_csv(report),
            Self::FilesCsv => files_csv(report),
            Self::PlainText => plain_text(report),
            Self::Json => json(report),
        }
    }
}

/// Language summary in CSV form (same layout as `codelines --csv`).
pub fn summary_csv(report: &Report) -> String {
    format_csv_table(&report.summaries)
}

/// Per-file statistics in CSV form.
pub fn files_csv(report: &Report) -> String {
    let mut out = String::from(
        "Path,Language,TotalLines,CodeLines,CommentLines,MixedLines,BlankLines,EffectiveCode,TotalComment,CodeRatio,CommentRatio\n",
    );
    for file in &report.files {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.4},{:.4}\n",
            csv_field(&file.path.display().to_string()),
            csv_field(&file.language),
            file.stats.total_lines,
            file.stats.code_lines,
            file.stats.comment_lines,
            file.stats.mixed_lines,
            file.stats.blank_lines,
            file.stats.effective_code_lines(),
            file.stats.total_comment_lines(),
            file.stats.code_ratio(),
            file.stats.comment_ratio(),
        ));
    }
    out
}

/// Plain text report: summary table followed by per-file sections.
pub fn plain_text(report: &Report) -> String {
    let mut out = format_plain_table(&report.summaries, true);
    out.push_str(&format!(
        "\nRoots: {}\nFiles scanned: {}\nAnalyzed files: {}\nElapsed: {}\n",
        report
            .roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        format_number(report.total_files_scanned),
        format_number(report.files.len()),
        format_duration(report.elapsed),
    ));

    if !report.unknown_extensions.is_empty() {
        out.push_str("\nUnrecognized file types:\n");
        for (extension, count) in &report.unknown_extensions {
            out.push_str(&format!("  {extension}  {count} file(s)\n"));
        }
    }

    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("  {warning}\n"));
        }
    }

    out.push_str("\nPer-file statistics:\n");
    for summary in &report.summaries {
        out.push_str(&format!(
            "\n  {} ({} files)\n",
            summary.language, summary.file_count
        ));
        let mut files: Vec<_> = report
            .files
            .iter()
            .filter(|file| file.language == summary.language)
            .collect();
        files.sort_by(|a, b| {
            b.stats
                .effective_code_lines()
                .cmp(&a.stats.effective_code_lines())
        });
        for file in files {
            out.push_str(&format!(
                "    {:<64} {:>7} lines  code:{:>6}  comment:{:>6}  mixed:{:>5}  blank:{:>5}\n",
                file.path.display().to_string(),
                file.stats.total_lines,
                file.stats.code_lines,
                file.stats.comment_lines,
                file.stats.mixed_lines,
                file.stats.blank_lines,
            ));
        }
    }

    out
}

/// Machine readable report.
pub fn json(report: &Report) -> String {
    let total = report.total();
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"tool\": \"codelines\",\n");
    out.push_str(&format!(
        "  \"version\": \"{}\",\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str("  \"roots\": [");
    for (index, root) in report.roots.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&json_string(&root.display().to_string()));
    }
    out.push_str("],\n");
    out.push_str(&format!(
        "  \"total_files_scanned\": {},\n",
        report.total_files_scanned
    ));
    out.push_str(&format!(
        "  \"elapsed_seconds\": {:.3},\n",
        report.elapsed.as_secs_f64()
    ));
    out.push_str(&format!("  \"cancelled\": {},\n", report.cancelled));
    out.push_str(&format!(
        "  \"totals\": {{\"files\": {}, \"total_lines\": {}, \"code_lines\": {}, \"comment_lines\": {}, \"mixed_lines\": {}, \"blank_lines\": {}, \"effective_code\": {}, \"total_comment\": {}, \"code_ratio\": {:.4}, \"comment_ratio\": {:.4}}},\n",
        total.file_count,
        total.total_lines,
        total.code_lines,
        total.comment_lines,
        total.mixed_lines,
        total.blank_lines,
        total.effective_code(),
        total.total_comment(),
        total.code_ratio(),
        total.comment_ratio(),
    ));

    out.push_str("  \"languages\": [\n");
    for (index, summary) in report.summaries.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"language\": {}, \"files\": {}, \"total_lines\": {}, \"code_lines\": {}, \"comment_lines\": {}, \"mixed_lines\": {}, \"blank_lines\": {}, \"effective_code\": {}, \"total_comment\": {}, \"code_ratio\": {:.4}, \"comment_ratio\": {:.4}}}",
            json_string(&summary.language),
            summary.file_count,
            summary.total_lines,
            summary.code_lines,
            summary.comment_lines,
            summary.mixed_lines,
            summary.blank_lines,
            summary.effective_code(),
            summary.total_comment(),
            summary.code_ratio(),
            summary.comment_ratio(),
        ));
        out.push_str(if index + 1 == report.summaries.len() {
            "\n"
        } else {
            ",\n"
        });
    }
    out.push_str("  ],\n");

    out.push_str("  \"files\": [\n");
    for (index, file) in report.files.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"path\": {}, \"language\": {}, \"total_lines\": {}, \"code_lines\": {}, \"comment_lines\": {}, \"mixed_lines\": {}, \"blank_lines\": {}, \"effective_code\": {}, \"total_comment\": {}, \"code_ratio\": {:.4}, \"comment_ratio\": {:.4}}}",
            json_string(&file.path.display().to_string()),
            json_string(&file.language),
            file.stats.total_lines,
            file.stats.code_lines,
            file.stats.comment_lines,
            file.stats.mixed_lines,
            file.stats.blank_lines,
            file.stats.effective_code_lines(),
            file.stats.total_comment_lines(),
            file.stats.code_ratio(),
            file.stats.comment_ratio(),
        ));
        out.push_str(if index + 1 == report.files.len() {
            "\n"
        } else {
            ",\n"
        });
    }
    out.push_str("  ],\n");

    out.push_str("  \"unknown_extensions\": [");
    for (index, (extension, count)) in report.unknown_extensions.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!(
            "{{\"extension\": {}, \"count\": {}}}",
            json_string(extension),
            count
        ));
    }
    out.push_str("],\n");

    out.push_str("  \"warnings\": [");
    for (index, warning) in report.warnings.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&json_string(warning));
    }
    out.push_str("]\n");
    out.push_str("}\n");
    out
}

/// Ask the user where to save the export.
pub fn save_dialog(kind: ExportKind) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export statistics")
        .set_file_name(kind.file_name())
        .add_filter(kind.filter_name(), &[kind.extension()])
        .save_file()
}

/// Write an export to disk.
pub fn write(kind: ExportKind, report: &Report, path: &Path) -> Result<(), String> {
    std::fs::write(path, kind.content(report)).map_err(|error| error.to_string())
}

/// Reveal a file in the platform file manager.
pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();

    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();

    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    let _ = path;
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::report::{ScanOptions, analyze};

    use super::*;

    fn sample_report(name: &str) -> (Report, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("codelines-export-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), "// note\nfn main() {}\n").unwrap();
        fs::write(dir.join("lib.rs"), "/* block */\nfn helper() {}\n").unwrap();

        let options = ScanOptions {
            paths: vec![dir.clone()],
            ..Default::default()
        };
        (analyze(&options, None, |_| {}), dir)
    }

    #[test]
    fn csv_exports_have_headers_and_rows() {
        let (report, dir) = sample_report("csv");

        let summary = summary_csv(&report);
        assert!(summary.starts_with("Language,Files,TotalLines"));
        assert!(summary.contains("Rust,2,"));

        let files = files_csv(&report);
        assert!(files.starts_with("Path,Language,TotalLines"));
        assert_eq!(files.lines().count(), report.files.len() + 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn json_export_is_balanced_and_escaped() {
        let (report, dir) = sample_report("json");
        let json = json(&report);

        assert!(json.starts_with('{'));
        assert!(json.trim_end().ends_with('}'));
        assert!(json.contains("\"languages\": ["));
        assert!(json.contains("\"files\": ["));

        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        for ch in json.chars() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '{' | '[' => depth += 1,
                '}' | ']' => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "unbalanced json: {json}");
        }
        assert_eq!(depth, 0, "unbalanced json: {json}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plain_text_export_contains_table_and_files() {
        let (report, dir) = sample_report("text");
        let text = plain_text(&report);

        assert!(text.contains("Language"));
        assert!(text.contains("Per-file statistics:"));
        assert!(text.contains("main.rs"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn quirks_are_escaped() {
        assert_eq!(json_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("plain"), "plain");
    }
}
