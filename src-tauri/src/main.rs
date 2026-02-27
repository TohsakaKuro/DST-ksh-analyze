// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod core;
mod glsl_parser;
mod types;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![analyze_ksh, build_ksh])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
