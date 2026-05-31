/// CLI 参数定义模块
use clap::Parser;

/// codelines - 代码量统计工具，支持多种语言分别统计，支持注释单独统计
#[derive(Parser, Debug)]
#[command(
    name = "codelines",
    version,
    about = "代码量统计工具 - 支持多语言分别统计，注释单独统计",
    long_about = "codelines 是一个高性能的代码量统计工具，能够递归扫描项目目录，\n\
                  按编程语言分别统计总行数、代码行、注释行、混合行和空白行。\n\
                  支持 30+ 种编程语言，智能识别注释（含块注释和行注释），\n\
                  正确处理字符串中的注释标记避免误判。"
)]
pub struct Args {
    /// 要统计的目录或文件路径（默认为当前目录）
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// 只统计指定语言（可多次使用），如 --language Rust --language Python
    #[arg(short, long = "lang", value_name = "LANGUAGE")]
    pub languages: Vec<String>,

    /// 排除指定目录（可多次使用），如 --exclude vendor --exclude generated
    #[arg(short, long = "exclude", value_name = "DIR")]
    pub exclude_dirs: Vec<String>,

    /// 排除指定文件（可多次使用），如 --exclude-file lock.json
    #[arg(long = "exclude-file", value_name = "FILE")]
    pub exclude_files: Vec<String>,

    /// 显示详细统计（区分纯代码行/纯注释行/混合行）
    #[arg(short, long)]
    pub verbose: bool,

    /// 以纯文本输出（无颜色，适合重定向到文件）
    #[arg(short, long)]
    pub plain: bool,

    /// 以 CSV 格式输出
    #[arg(long)]
    pub csv: bool,

    /// 列出所有支持的语言及文件扩展名
    #[arg(short, long)]
    pub list: bool,

    /// 显示每个文件的详细统计
    #[arg(short = 'f', long)]
    pub files: bool,
}
