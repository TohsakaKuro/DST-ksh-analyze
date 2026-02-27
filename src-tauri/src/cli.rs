mod core;
mod glsl_parser;
mod types;

use clap::{Arg, Command};
use log::info;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("dst-ksh-analyze-cli")
        .version("0.1.0")
        .author("TohsakaKuro<tohsakakuro@outlook.com>")
        .about("饥荒联机版着色器文件分析工具 (CLI)")
        .help_template("用法: {usage}\n\n{all-args}\n\n{about}\n\n{after-help}")
        .arg(
            Arg::new("path1")
                .help(
                    "输入路径，可以是：\n\
                       - .ksh 文件（用于分析）\n\
                       - 包含 vs 和 ps 着色器文件的目录\n\
                       - 两个着色器文件（vs 和 ps，顺序任意）",
                )
                .required(true)
                .index(1)
                .value_name("输入路径")
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            Arg::new("path2")
                .help("第二个着色器文件路径。仅在输入为着色器文件时需要。")
                .required(false)
                .index(2)
                .value_name("第二个文件")
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            Arg::new("path3")
                .help("输出路径。如果未指定，将使用当前目录。")
                .required(false)
                .index(3)
                .value_name("输出目录或文件")
                .value_hint(clap::ValueHint::DirPath),
        )
        .arg(
            Arg::new("debug")
                .help("启用调试日志以获取更详细的输出。")
                .required(false)
                .long("debug")
                .short('d')
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .help("允许覆盖文件")
                .required(false)
                .long("force")
                .short('f')
                .action(clap::ArgAction::SetTrue),
        )
        .after_help(
            "使用示例：\n\
                     \n\
                     分析 .ksh 文件：\n\
                     \tdst-ksh-analyze-cli input.ksh output\n\
                     \n\
                     从包含着色器文件的目录构建：\n\
                     \tdst-ksh-analyze-cli shader_dir output.ksh\n\
                     \n\
                     从两个着色器文件构建（顺序任意）：\n\
                     \tdst-ksh-analyze-cli input.vs input.ps output.ksh\n\
                     \n\
                     启用调试日志：\n\
                     \tdst-ksh-analyze-cli input.ksh --debug\n\
                     \n\
                     强制覆盖已存在的文件：\n\
                     \tdst-ksh-analyze-cli input.ksh --force",
        )
        .get_matches();

    let debug = matches.get_flag("debug");
    let force = matches.get_flag("force");

    if debug {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .format(|buf, record| writeln!(buf, "{}: {}", record.level(), record.args()))
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .format(|buf, record| writeln!(buf, "{}: {}", record.level(), record.args()))
            .init();
    }

    let input = match matches.get_one::<String>("path1") {
        Some(input) => input,
        None => {
            return Err("需要指定输入路径".into());
        }
    };

    let input_path = Path::new(input);
    if input_path.extension().and_then(|s| s.to_str()) == Some("ksh") {
        let output_path = matches
            .get_one::<String>("path3")
            .map(Path::new)
            .unwrap_or_else(|| Path::new(input_path.file_stem().unwrap_or(OsStr::new("output"))));
        if !output_path.exists() {
            fs::create_dir_all(output_path).map_err(|e| format!("创建输出目录失败: {}", e))?;
        } else if !output_path.is_dir() {
            return Err("输出路径不是目录".into());
        }
        core::analyze_ksh_file(input_path, output_path, force)
            .map_err(|e| format!("分析着色器文件失败: {}", e))?;
    } else if input_path.is_dir() {
        let output_path =
            if let Some(output_path) = matches.get_one::<String>("path2").map(Path::new) {
                let mut path = output_path.to_path_buf();
                if path.extension().and_then(|s| s.to_str()) != Some("ksh") {
                    path.set_extension("ksh");
                }
                path
            } else {
                return Err("需要指定输出.ksh文件".into());
            };
        if !force && output_path.exists() {
            return Err(format!("输出文件已存在: {}", output_path.display()).into());
        }
        core::build_ksh_file_from_dir(input_path, &output_path)
            .map_err(|e| format!("构建着色器文件失败: {}", e))?;
    } else if let Some(second_file) = matches.get_one::<String>("path2") {
        if !input_path.exists() {
            return Err(format!("未找到第一个着色器文件: {}", input).into());
        }

        let second_path = Path::new(second_file);
        if !second_path.exists() {
            return Err(format!("未找到第二个着色器文件: {}", second_file).into());
        }

        let mut has_vs = false;
        let mut has_ps = false;
        if input_path.extension().and_then(|s| s.to_str()) == Some("vs")
            || second_path.extension().and_then(|s| s.to_str()) == Some("vs")
        {
            has_vs = true;
        }
        if input_path.extension().and_then(|s| s.to_str()) == Some("ps")
            || second_path.extension().and_then(|s| s.to_str()) == Some("ps")
        {
            has_ps = true;
        }

        if !has_vs || !has_ps {
            return Err("需要指定两个不同类型的着色器文件 (.ps/.vs)".into());
        }

        let output_path =
            if let Some(output_path) = matches.get_one::<String>("path3").map(Path::new) {
                let mut path = output_path.to_path_buf();
                if path.extension().and_then(|s| s.to_str()) != Some("ksh") {
                    path.set_extension("ksh");
                }
                path
            } else {
                return Err("需要指定输出.ksh文件".into());
            };

        if !force && output_path.exists() {
            return Err(format!("输出文件已存在: {}", output_path.display()).into());
        }

        let (vs_file, ps_file) = if input_path.extension().and_then(|s| s.to_str()) == Some("vs") {
            (input_path, second_path)
        } else {
            (second_path, input_path)
        };

        core::build_ksh_file(vs_file, ps_file, &output_path)
            .map_err(|e| format!("构建着色器文件失败: {}", e))?;
    } else {
        return Err("无效的输入. 期望: - .ksh 文件, - 包含 .vs 和 .ps 着色器文件的目录, - 两个着色器文件（.vs 和 .ps，顺序任意）".into());
    }

    info!("所有任务已完成");
    Ok(())
}
