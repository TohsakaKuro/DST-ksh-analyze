// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dst_ksh_analyze_lib::{core, types::VariableScope};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
struct ShaderInfo {
    name: String,
    content: String,
    uniform_indices: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct UniformInfo {
    index: usize,
    name: String,
    scope_code: u32,
    scope: &'static str,
    type_code: u32,
    #[serde(rename = "type")]
    type_name: String,
    array_count: u32,
    default_data: Vec<u32>,
    stages: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct AnalyzeKshResult {
    effect: String,
    uniforms: Vec<UniformInfo>,
    vs: ShaderInfo,
    ps: ShaderInfo,
}

#[derive(Debug, Deserialize)]
struct BuildKshParams {
    output_path: String,
    base_ksh_path: Option<String>,
    vs_name: String,
    vs_content: String,
    ps_name: String,
    ps_content: String,
}

fn scope_name(scope: VariableScope) -> &'static str {
    match scope {
        VariableScope::Uniform => "uniform",
        VariableScope::Unknown(_) => "unknown",
    }
}

fn extension_is(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

#[tauri::command]
async fn analyze_ksh(file_path: String) -> Result<AnalyzeKshResult, String> {
    let input_path = Path::new(&file_path);
    let ksh_content = fs::read(input_path)
        .map_err(|error| format!("读取 KSH 文件 {} 失败: {error}", input_path.display()))?;
    let ksh = core::parse_ksh(&ksh_content).map_err(|error| error.to_string())?;

    let mut uniforms = Vec::with_capacity(ksh.uniforms.len());
    for (index, uniform) in ksh.uniforms.iter().enumerate() {
        let index_u32 = u32::try_from(index)
            .map_err(|_| format!("uniform 索引 {index} 超过 u32 可表示范围"))?;
        let mut stages = Vec::with_capacity(2);
        if ksh.vertex.uniform_indices.contains(&index_u32) {
            stages.push("vs");
        }
        if ksh.pixel.uniform_indices.contains(&index_u32) {
            stages.push("ps");
        }
        uniforms.push(UniformInfo {
            index,
            name: uniform.name.clone(),
            scope_code: uniform.scope.code(),
            scope: scope_name(uniform.scope),
            type_code: uniform.uniform_type.id(),
            type_name: String::from(&uniform.uniform_type),
            array_count: uniform.array_count,
            default_data: uniform.default_data.clone(),
            stages,
        });
    }

    Ok(AnalyzeKshResult {
        effect: ksh.effect_name,
        uniforms,
        vs: ShaderInfo {
            name: ksh.vertex.source_name,
            content: ksh.vertex.source,
            uniform_indices: ksh.vertex.uniform_indices,
        },
        ps: ShaderInfo {
            name: ksh.pixel.source_name,
            content: ksh.pixel.source,
            uniform_indices: ksh.pixel.uniform_indices,
        },
    })
}

#[tauri::command]
async fn build_ksh(params: BuildKshParams) -> Result<(), String> {
    let output_path = Path::new(&params.output_path);
    require_extension(output_path, "ksh", "KSH 输出文件")?;
    let file_name = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "无法从输出路径解析文件名".to_string())?;

    let ksh_content = if let Some(base_path) = params.base_ksh_path.as_deref() {
        let base_path = Path::new(base_path);
        require_extension(base_path, "ksh", "KSH 元数据来源文件")?;
        let base_bytes = fs::read(base_path).map_err(|error| {
            format!(
                "读取 KSH 元数据来源文件 {} 失败: {error}",
                base_path.display()
            )
        })?;
        let base = core::parse_ksh(&base_bytes).map_err(|error| {
            format!(
                "解析 KSH 元数据来源文件 {} 失败: {error}",
                base_path.display()
            )
        })?;
        core::build_ksh_preserving_metadata(
            &base,
            &params.vs_name,
            &params.vs_content,
            &params.ps_name,
            &params.ps_content,
        )
        .map_err(|error| error.to_string())?
    } else {
        core::build_ksh(
            file_name,
            &params.vs_name,
            &params.vs_content,
            &params.ps_name,
            &params.ps_content,
        )
        .map_err(|error| error.to_string())?
    };

    core::write_file_atomic(output_path, &ksh_content)
        .map_err(|error| format!("写入 KSH 文件 {} 失败: {error}", output_path.display()))?;

    Ok(())
}

#[tauri::command]
async fn write_shader_source(
    file_path: String,
    content: String,
    stage: String,
) -> Result<(), String> {
    let output_path = Path::new(&file_path);
    let expected_stage = parse_shader_stage(&stage)?;
    require_extension(
        output_path,
        expected_stage.extension(),
        expected_stage.label(),
    )?;
    if content.as_bytes().contains(&0) {
        return Err("着色器源码不能包含 NUL 字节".to_string());
    }

    core::write_file_atomic(output_path, content.as_bytes())
        .map_err(|error| format!("写入着色器源码 {} 失败: {error}", output_path.display()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShaderStage {
    Vertex,
    Pixel,
}

impl ShaderStage {
    const fn extension(self) -> &'static str {
        match self {
            Self::Vertex => "vs",
            Self::Pixel => "ps",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Vertex => "顶点着色器源码",
            Self::Pixel => "像素着色器源码",
        }
    }
}

fn parse_shader_stage(value: &str) -> Result<ShaderStage, String> {
    if value.eq_ignore_ascii_case("vs") {
        Ok(ShaderStage::Vertex)
    } else if value.eq_ignore_ascii_case("ps") {
        Ok(ShaderStage::Pixel)
    } else {
        Err(format!("未知的着色器阶段: {value:?}；只支持 vs 或 ps"))
    }
}

fn require_extension(path: &Path, expected: &str, label: &str) -> Result<(), String> {
    if extension_is(path, expected) {
        Ok(())
    } else {
        Err(format!(
            "{label}必须使用 .{expected} 扩展名: {}",
            path.display()
        ))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            analyze_ksh,
            build_ksh,
            write_shader_source
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_output_extensions_are_stage_specific() {
        assert!(require_extension(Path::new("effect.ksh"), "ksh", "KSH").is_ok());
        assert!(require_extension(Path::new("effect.ps"), "ksh", "KSH").is_err());

        let pixel = parse_shader_stage("ps").unwrap();
        assert_eq!(pixel.extension(), "ps");
        assert!(
            require_extension(Path::new("effect.ps"), pixel.extension(), pixel.label()).is_ok()
        );
        assert!(
            require_extension(Path::new("effect.vs"), pixel.extension(), pixel.label()).is_err()
        );
        assert!(parse_shader_stage("geometry").is_err());
    }
}
