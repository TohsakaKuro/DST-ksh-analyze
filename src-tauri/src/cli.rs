use clap::{Arg, Command};
use dst_ksh_analyze_lib::core;
use log::info;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn extension_is(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn ensure_output_available(path: &Path, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !force && path.exists() {
        return Err(format!("输出文件已存在: {}", path.display()).into());
    }
    Ok(())
}

fn with_ksh_extension(path: &Path) -> PathBuf {
    let mut output = path.to_path_buf();
    if !extension_is(&output, "ksh") {
        output.set_extension("ksh");
    }
    output
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShaderStage {
    Vertex,
    Pixel,
}

fn shader_stage(path: &Path) -> Option<ShaderStage> {
    if extension_is(path, "vs") {
        Some(ShaderStage::Vertex)
    } else if extension_is(path, "ps") {
        Some(ShaderStage::Pixel)
    } else {
        None
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("dst-ksh-analyze-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TohsakaKuro<tohsakakuro@outlook.com>")
        .about(env!("CARGO_PKG_DESCRIPTION"))
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
                .help("KSH 解包输出目录、KSH 构建输出文件，或第二个着色器文件。")
                .required(false)
                .index(2)
                .value_name("输出路径或第二个文件")
                .value_hint(clap::ValueHint::AnyPath),
        )
        .arg(
            Arg::new("path3")
                .help("输入两个着色器文件时必填的 .ksh 输出文件路径。")
                .required(false)
                .index(3)
                .value_name("输出文件")
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            Arg::new("debug")
                .help("启用调试日志以获取更详细的输出。")
                .required(false)
                .long("debug")
                .short('d')
                .action(clap::ArgAction::SetTrue),
        )
        // 允许覆盖文件
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
                     \tdst-ksh-analyze-cli input.ksh output_dir\n\
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

    // 设置日志级别
    let debug = matches.get_flag("debug");
    let force = matches.get_flag("force");

    env_logger::Builder::from_default_env()
        .filter_level(if debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .format(|buf, record| writeln!(buf, "{}: {}", record.level(), record.args()))
        .try_init()?;

    let input = matches
        .get_one::<String>("path1")
        .ok_or("需要指定输入路径")?;
    let input_path = Path::new(input);
    if !input_path.exists() {
        return Err(format!("未找到输入路径: {}", input_path.display()).into());
    }

    if extension_is(input_path, "ksh") {
        if matches.get_one::<String>("path3").is_some() {
            return Err("分析 KSH 时输出目录应作为第二个位置参数".into());
        }
        let output_path = match matches.get_one::<String>("path2") {
            Some(path) => PathBuf::from(path),
            None => match input_path.file_stem() {
                Some(stem) if !stem.is_empty() => PathBuf::from(stem),
                _ => PathBuf::from("output"),
            },
        };
        if !output_path.exists() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("创建输出目录 {} 失败: {error}", output_path.display()))?;
        } else if !output_path.is_dir() {
            return Err(format!("输出路径不是目录: {}", output_path.display()).into());
        }
        core::analyze_ksh_file(input_path, &output_path, force)
            .map_err(|error| format!("分析着色器文件失败: {error}"))?;
    } else if input_path.is_dir() {
        if matches.get_one::<String>("path3").is_some() {
            return Err("从目录构建时只需要输入目录和输出 KSH 两个位置参数".into());
        }
        let output_argument = matches
            .get_one::<String>("path2")
            .ok_or("需要指定输出 .ksh 文件")?;
        let output_path = with_ksh_extension(Path::new(output_argument));
        ensure_output_available(&output_path, force)?;
        core::build_ksh_file_from_dir(input_path, &output_path)
            .map_err(|error| format!("构建着色器文件失败: {error}"))?;
    } else {
        let second_file = matches
            .get_one::<String>("path2")
            .ok_or("无效的输入；需要 .ksh 文件、包含 .vs/.ps 的目录，或两个着色器文件")?;
        let second_path = Path::new(second_file);
        if !second_path.is_file() {
            return Err(format!("未找到第二个着色器文件: {}", second_path.display()).into());
        }

        let (vs_path, ps_path) = match (shader_stage(input_path), shader_stage(second_path)) {
            (Some(ShaderStage::Vertex), Some(ShaderStage::Pixel)) => (input_path, second_path),
            (Some(ShaderStage::Pixel), Some(ShaderStage::Vertex)) => (second_path, input_path),
            _ => return Err("需要分别指定一个 .vs 和一个 .ps 着色器文件".into()),
        };
        let output_argument = matches
            .get_one::<String>("path3")
            .ok_or("需要指定输出 .ksh 文件")?;
        let output_path = with_ksh_extension(Path::new(output_argument));
        ensure_output_available(&output_path, force)?;
        core::build_ksh_file(vs_path, ps_path, &output_path)
            .map_err(|error| format!("构建着色器文件失败: {error}"))?;
    }

    info!("所有任务已完成");
    Ok(())
}
