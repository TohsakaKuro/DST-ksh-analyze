// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(all(not(debug_assertions), not(feature = "cli")), windows_subsystem = "windows")]
mod core;
mod glsl_parser;
mod types;

use clap::{Arg, Command};
use log::info;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
struct ShaderInfo {
    name: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BuildKshParams {
    output_path: String,
    vs_name: String,
    vs_content: String,
    ps_name: String,
    ps_content: String,
}

#[tauri::command]
async fn analyze_ksh(file_path: String) -> Result<serde_json::Value, String> {
    let input_path = Path::new(&file_path);
    let ksh_content = fs::read(input_path).map_err(|e| e.to_string())?;

    let (vs_name, vs_content, ps_name, ps_content) =
        core::analyze_ksh(&ksh_content).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "vs": {
            "name": vs_name,
            "content": vs_content
        },
        "ps": {
            "name": ps_name,
            "content": ps_content
        }
    }))
}

#[tauri::command]
async fn build_ksh(params: BuildKshParams) -> Result<(), String> {
    let output_path = Path::new(&params.output_path);
    let file_name = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "无法从输出路径解析文件名".to_string())?;

    let ksh_content = core::build_ksh(
        file_name,
        &params.vs_name,
        &params.vs_content,
        &params.ps_name,
        &params.ps_content,
    )
    .map_err(|e| e.to_string())?;

    fs::write(&params.output_path, ksh_content).map_err(|e| e.to_string())?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("dst-ksh-analyze")
        .version("0.1.0")
        .author("TohsakaKuro<tohsakakuro@outlook.com>")
        .about("饥荒联机版着色器文件分析工具")
        .help_template("用法: {usage}\n\n{all-args}\n\n{about}\n\n{after-help}")
        .arg(
            Arg::new("path1")
                .help(
                    "输入路径，可以是：\n\
                       - .ksh 文件（用于分析）\n\
                       - 包含 vs 和 ps 着色器文件的目录\n\
                       - 两个着色器文件（vs 和 ps，顺序任意）",
                )
                .required(false)
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
                     \tksh-analyzer input.ksh output\n\
                     \n\
                     从包含着色器文件的目录构建：\n\
                     \tksh-analyzer shader_dir output.ksh\n\
                     \n\
                     从两个着色器文件构建（顺序任意）：\n\
                     \tksh-analyzer input.vs input.ps output.ksh\n\
                     \n\
                     启用调试日志：\n\
                     \tksh-analyzer input.ksh --debug\n\
                     \n\
                     强制覆盖已存在的文件：\n\
                     \tksh-analyzer input.ksh --force",
        )
        .get_matches();

    // 如果没有任何参数，启动 Tauri 应用
    if std::env::args().len() <= 1 {
        tauri::Builder::default()
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_fs::init())
            .plugin(tauri_plugin_dialog::init())
            .invoke_handler(tauri::generate_handler![analyze_ksh, build_ksh])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
        return Ok(());
    }

    // 设置日志级别
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
            dst_ksh_analyze_lib::run();
            return Ok(());
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
        if !force {
            if output_path.exists() {
                return Err(format!("输出文件已存在: {}", output_path.display()).into());
            }
        }
        core::build_ksh_file_from_dir(input_path, &output_path)
            .map_err(|e| format!("构建着色器文件失败: {}", e))?;
    } else if let Some(second_file) = matches.get_one::<String>("path2") {
        if !input_path.exists() {
            return Err(format!("未找到第一个着色器文件: {}", input).into());
        }
        // 处理两个文件输入
        let second_path = Path::new(second_file);
        if !second_path.exists() {
            return Err(format!("未找到第二个着色器文件: {}", second_file).into());
        }
        // 从第一个 第二个输出里,自动识别出 vs ps文件
        let mut vs_file_path = None;
        let mut ps_file_path = None;
        if input_path.extension().and_then(|s| s.to_str()) == Some("vs") {
            vs_file_path = Some(input_path);
        } else if input_path.extension().and_then(|s| s.to_str()) == Some("ps") {
            ps_file_path = Some(input_path);
        }
        if second_path.extension().and_then(|s| s.to_str()) == Some("vs") {
            vs_file_path = Some(second_path);
        } else if second_path.extension().and_then(|s| s.to_str()) == Some("ps") {
            ps_file_path = Some(second_path);
        }
        if vs_file_path.is_none() || ps_file_path.is_none() {
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
        if !force {
            if output_path.exists() {
                return Err(format!("输出文件已存在: {}", output_path.display()).into());
            }
        }
        core::build_ksh_file(input_path, second_path, &output_path)
            .map_err(|e| format!("构建着色器文件失败: {}", e))?;
    } else {
        return Err("无效的输入. 期望: - .ksh 文件, - 包含 .vs 和 .ps 着色器文件的目录, - 两个着色器文件（.vs 和 .ps，顺序任意）".into());
    }
    info!("所有任务已完成");
    Ok(())
}
