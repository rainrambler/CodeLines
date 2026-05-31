/// 结果格式化输出模块
use std::collections::HashMap;

use colored::*;

use crate::analyzer::LineStats;
use crate::language;

/// 按语言汇总的统计结果
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
    /// 有效代码行（纯代码 + 混合行）
    pub fn effective_code(&self) -> usize {
        self.code_lines + self.mixed_lines
    }

    /// 总注释相关行（纯注释 + 混合行）
    pub fn total_comment(&self) -> usize {
        self.comment_lines + self.mixed_lines
    }

    /// 代码占比
    pub fn code_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.effective_code() as f64 / self.total_lines as f64
        }
    }

    /// 注释占比
    pub fn comment_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.total_comment() as f64 / self.total_lines as f64
        }
    }
}

/// 从语言级别的行统计汇总计算 LanguageSummary
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

    // 按有效代码行数降序排列
    summaries.sort_by(|a, b| b.effective_code().cmp(&a.effective_code()));
    summaries
}

/// 彩色终端输出
pub fn print_colored_table(summaries: &[LanguageSummary], verbose: bool) {
    // 计算全局汇总
    let total = summaries.iter().fold(LanguageSummary::default(), |acc, s| {
        LanguageSummary {
            language: "总计".to_string(),
            file_count: acc.file_count + s.file_count,
            total_lines: acc.total_lines + s.total_lines,
            code_lines: acc.code_lines + s.code_lines,
            comment_lines: acc.comment_lines + s.comment_lines,
            mixed_lines: acc.mixed_lines + s.mixed_lines,
            blank_lines: acc.blank_lines + s.blank_lines,
        }
    });

    // 表头
    let header = if verbose {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
            "语言", "文件数", "总行数", "代码行", "注释行", "混合行", "空白行", "代码%", "注释%"
        )
    } else {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>8} {:>8}",
            "语言", "文件数", "总行数", "有效代码", "总注释", "空白行", "代码%", "注释%"
        )
    };

    println!();
    println!("{}", header.bold().cyan());
    println!("{}", "─".repeat(header.len()).dimmed());

    // 每种语言一行
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

    // 汇总行
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

/// 纯文本输出（无颜色控制字符，适合重定向到文件）
pub fn print_plain_table(summaries: &[LanguageSummary], verbose: bool) {
    let total = summaries.iter().fold(LanguageSummary::default(), |acc, s| {
        LanguageSummary {
            language: "Total".to_string(),
            file_count: acc.file_count + s.file_count,
            total_lines: acc.total_lines + s.total_lines,
            code_lines: acc.code_lines + s.code_lines,
            comment_lines: acc.comment_lines + s.comment_lines,
            mixed_lines: acc.mixed_lines + s.mixed_lines,
            blank_lines: acc.blank_lines + s.blank_lines,
        }
    });

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

    println!("{}", header);
    println!("{}", "-".repeat(header.len()));

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

    println!("{}", "-".repeat(header.len()));
    let total_line = if verbose {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
            total.language,
            total.file_count,
            total.total_lines,
            total.code_lines,
            total.comment_lines,
            total.mixed_lines,
            total.blank_lines,
            total.code_ratio() * 100.0,
            total.comment_ratio() * 100.0,
        )
    } else {
        format!(
            "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}%",
            total.language,
            total.file_count,
            total.total_lines,
            total.effective_code(),
            total.total_comment(),
            total.blank_lines,
            total.code_ratio() * 100.0,
            total.comment_ratio() * 100.0,
        )
    };
    println!("{}", total_line);
}

/// CSV 输出
pub fn print_csv_table(summaries: &[LanguageSummary]) {
    println!("Language,Files,TotalLines,CodeLines,CommentLines,MixedLines,BlankLines,EffectiveCode,TotalComment,CodeRatio,CommentRatio");

    for s in summaries {
        println!(
            "{},{},{},{},{},{},{},{},{},{:.4},{:.4}",
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
        );
    }
}

/// 打印支持的语言列表
pub fn print_supported_languages() {
    println!("\n支持的语言及文件扩展名：\n");
    for lang in language::supported_languages() {
        let exts = lang.extensions.join(", ");
        println!("  {:<24} {}", format!("[{}]", lang.name), exts);
    }
    println!();
}

/// 打印未知扩展名统计
pub fn print_unknown_extensions(unknown: &HashMap<String, usize>) {
    if unknown.is_empty() {
        return;
    }
    println!("{}", "未识别的文件类型（已跳过）：".dimmed());
    let mut entries: Vec<_> = unknown.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    for (ext, count) in entries {
        println!("  {} {} 个文件", ext, count);
    }
    println!();
}
