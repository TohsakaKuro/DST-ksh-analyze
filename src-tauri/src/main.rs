// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dst_ksh_analyze_lib::{
    core,
    types::{KshFile, VariableScope},
};
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
    metadata: KshFile,
    effect: String,
    uniforms: Vec<UniformInfo>,
    vs: ShaderInfo,
    ps: ShaderInfo,
}

#[derive(Debug, Deserialize)]
struct BuildKshParams {
    output_path: String,
    #[serde(default)]
    name_from_output: bool,
    force: Option<bool>,
    base_ksh: Option<KshFile>,
    base_ksh_path: Option<String>,
    vs_name: String,
    vs_content: String,
    ps_name: String,
    ps_content: String,
}

#[derive(Debug, Deserialize)]
struct CheckKshParams {
    base_ksh: Option<KshFile>,
    vs_content: String,
    ps_content: String,
}

#[derive(Debug, Deserialize)]
struct ShaderSourceParams {
    stage: String,
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct SaveShaderSourcesParams {
    sources: Vec<ShaderSourceParams>,
    force: bool,
}

#[derive(Debug, Deserialize)]
struct SaveEditorSourceParams {
    path: String,
    content: String,
    force: bool,
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
    analyze_ksh_metadata(ksh)
}

fn analyze_ksh_metadata(ksh: KshFile) -> Result<AnalyzeKshResult, String> {
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
        metadata: ksh.clone(),
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
async fn check_ksh(params: CheckKshParams) -> Result<(), String> {
    check_ksh_impl(params)
}

fn check_ksh_impl(params: CheckKshParams) -> Result<(), String> {
    // Run the real builder in memory; the output name is not chosen yet.
    build_ksh_bytes(&BuildKshParams {
        output_path: "preflight.ksh".to_owned(),
        name_from_output: true,
        force: None,
        base_ksh: params.base_ksh,
        base_ksh_path: None,
        vs_name: "preflight.vs".to_owned(),
        vs_content: params.vs_content,
        ps_name: "preflight.ps".to_owned(),
        ps_content: params.ps_content,
    })
    .map(|_| ())
}

#[tauri::command]
async fn build_ksh(params: BuildKshParams) -> Result<(), String> {
    build_ksh_impl(&params)
}

fn build_ksh_impl(params: &BuildKshParams) -> Result<(), String> {
    let ksh_content = build_ksh_bytes(params)?;
    let output_path = Path::new(&params.output_path);
    core::write_source_files_atomic(&[(output_path, &ksh_content)], params.force.unwrap_or(true))
        .map_err(|error| format!("写入 KSH 文件 {} 失败: {error}", output_path.display()))
}

fn build_ksh_bytes(params: &BuildKshParams) -> Result<Vec<u8>, String> {
    let output_path = Path::new(&params.output_path);
    require_extension(output_path, "ksh", "KSH 输出文件")?;
    let file_name = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "无法从输出路径解析文件名".to_string())?;
    let vs_name = if params.name_from_output {
        format!("{file_name}.vs")
    } else {
        params.vs_name.clone()
    };
    let ps_name = if params.name_from_output {
        format!("{file_name}.ps")
    } else {
        params.ps_name.clone()
    };

    let path_metadata;
    let base = if let Some(base) = params.base_ksh.as_ref() {
        Some(base)
    } else if let Some(base_path) = params.base_ksh_path.as_deref() {
        let base_path = Path::new(base_path);
        require_extension(base_path, "ksh", "KSH 元数据来源文件")?;
        let base_bytes = fs::read(base_path).map_err(|error| {
            format!(
                "读取 KSH 元数据来源文件 {} 失败: {error}",
                base_path.display()
            )
        })?;
        path_metadata = core::parse_ksh(&base_bytes).map_err(|error| {
            format!(
                "解析 KSH 元数据来源文件 {} 失败: {error}",
                base_path.display()
            )
        })?;
        Some(&path_metadata)
    } else {
        None
    };

    if let Some(base) = base {
        let mut renamed_base;
        let base = if params.name_from_output {
            renamed_base = base.clone();
            renamed_base.effect_name = file_name.to_owned();
            &renamed_base
        } else {
            base
        };
        core::build_ksh_preserving_metadata(
            base,
            &vs_name,
            &params.vs_content,
            &ps_name,
            &params.ps_content,
        )
        .map_err(|error| error.to_string())
    } else {
        core::build_ksh(
            file_name,
            &vs_name,
            &params.vs_content,
            &ps_name,
            &params.ps_content,
        )
        .map_err(|error| error.to_string())
    }
}

#[tauri::command]
async fn save_shader_sources(params: SaveShaderSourcesParams) -> Result<(), String> {
    save_shader_sources_impl(&params)
}

#[tauri::command]
async fn save_editor_source(params: SaveEditorSourceParams) -> Result<(), String> {
    save_editor_source_impl(&params)
}

fn save_editor_source_impl(params: &SaveEditorSourceParams) -> Result<(), String> {
    let path = Path::new(&params.path);
    if !["vs", "ps", "glsl", "txt"]
        .iter()
        .any(|extension| extension_is(path, extension))
    {
        return Err("源码文件应使用 .vs、.ps、.glsl 或 .txt 扩展名".to_string());
    }
    if params.path.as_bytes().contains(&0) || params.content.as_bytes().contains(&0) {
        return Err("源码路径和内容不能包含 NUL 字节".to_string());
    }
    core::write_source_files_atomic(&[(path, params.content.as_bytes())], params.force)
        .map_err(|error| format!("保存源码失败: {error}"))
}

fn save_shader_sources_impl(params: &SaveShaderSourcesParams) -> Result<(), String> {
    if !(1..=2).contains(&params.sources.len()) {
        return Err("一次只能保存 1 或 2 个着色器源码文件".to_string());
    }

    let mut stages = Vec::with_capacity(2);
    let mut sources = Vec::with_capacity(2);
    for source in &params.sources {
        let stage = parse_shader_stage(&source.stage)?;
        if stages.contains(&stage) {
            return Err(format!("不能重复保存同一个着色器阶段: {}", source.stage));
        }
        if source.path.as_bytes().contains(&0) || source.content.as_bytes().contains(&0) {
            return Err("着色器路径和源码不能包含 NUL 字节".to_string());
        }
        let path = Path::new(&source.path);
        require_extension(path, stage.extension(), stage.label())?;
        stages.push(stage);
        sources.push((path, source.content.as_bytes()));
    }

    core::write_source_files_atomic(&sources, params.force)
        .map_err(|error| format!("保存着色器源码失败: {error}"))
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
            check_ksh,
            write_shader_source,
            save_shader_sources,
            save_editor_source
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dst_ksh_analyze_lib::types::{KshStage, KshUniform, KshUniformType};

    fn sample_metadata() -> KshFile {
        KshFile {
            effect_name: "original_effect".to_string(),
            uniforms: vec![KshUniform {
                name: "OPAQUE".to_string(),
                scope: VariableScope::Unknown(7),
                uniform_type: KshUniformType::Unknown(99),
                array_count: 1,
                default_data: vec![0x8000_0000, 0x7fc0_1234, u32::MAX],
            }],
            vertex: KshStage {
                source_name: "original.vs".to_string(),
                source: "void main() {}".to_string(),
                uniform_indices: vec![0],
            },
            pixel: KshStage {
                source_name: "original.ps".to_string(),
                source: "void main() {}".to_string(),
                uniform_indices: vec![],
            },
        }
    }

    fn unique_test_dir(name: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dst-ksh-command-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn metadata_snapshot_survives_json_and_takes_priority_over_disk_path() {
        let original = sample_metadata();
        let analyzed = analyze_ksh_metadata(original.clone()).unwrap();
        let result = serde_json::to_value(analyzed).unwrap();
        let params: BuildKshParams = serde_json::from_value(serde_json::json!({
            "output_path": "renamed.ksh",
            "base_ksh": result["metadata"],
            "base_ksh_path": "missing-or-replaced-file.txt",
            "vs_name": "original.vs",
            "vs_content": original.vertex.source,
            "ps_name": "original.ps",
            "ps_content": original.pixel.source
        }))
        .unwrap();
        assert_eq!(params.base_ksh.as_ref(), Some(&original));
        let bytes = build_ksh_bytes(&params).unwrap();
        assert_eq!(bytes, core::encode_ksh(&original).unwrap());
    }

    #[test]
    fn legacy_base_path_still_supplies_metadata_when_snapshot_is_missing() {
        let directory = unique_test_dir("legacy-metadata");
        let base_path = directory.join("original.ksh");
        let original = sample_metadata();
        fs::write(&base_path, core::encode_ksh(&original).unwrap()).unwrap();
        let params: BuildKshParams = serde_json::from_value(serde_json::json!({
            "output_path": "renamed.ksh",
            "base_ksh_path": base_path,
            "vs_name": "original.vs",
            "vs_content": original.vertex.source,
            "ps_name": "original.ps",
            "ps_content": original.pixel.source
        }))
        .unwrap();
        assert_eq!(
            build_ksh_bytes(&params).unwrap(),
            core::encode_ksh(&original).unwrap()
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn preflight_uses_the_builder_for_fresh_and_imported_sources() {
        for base in [None, Some(sample_metadata())] {
            let vertex = "uniform float SHARED; void main() { gl_Position = vec4(SHARED); }";
            let pixel =
                "uniform vec2 SHARED; void main() { gl_FragColor = vec4(SHARED, 0.0, 1.0); }";
            let error = check_ksh_impl(CheckKshParams {
                base_ksh: base.clone(),
                vs_content: vertex.to_owned(),
                ps_content: pixel.to_owned(),
            })
            .unwrap_err();
            let build_error = build_ksh_bytes(&BuildKshParams {
                output_path: "chosen.ksh".to_owned(),
                name_from_output: true,
                force: None,
                base_ksh: base,
                base_ksh_path: None,
                vs_name: "chosen.vs".to_owned(),
                vs_content: vertex.to_owned(),
                ps_name: "chosen.ps".to_owned(),
                ps_content: pixel.to_owned(),
            })
            .unwrap_err();
            assert_eq!(error, build_error);
        }
        let original = sample_metadata();
        check_ksh_impl(CheckKshParams {
            base_ksh: Some(original.clone()),
            vs_content: original.vertex.source,
            ps_content: original.pixel.source,
        })
        .unwrap();
        check_ksh_impl(CheckKshParams {
            base_ksh: None,
            vs_content: "void main() {}".to_owned(),
            ps_content: "void main() {}".to_owned(),
        })
        .unwrap();
    }

    #[test]
    fn output_name_updates_all_names_without_losing_imported_metadata() {
        let original = sample_metadata();
        let params: BuildKshParams = serde_json::from_value(serde_json::json!({
            "output_path": "glow.ksh", "name_from_output": true,
            "base_ksh": original, "vs_name": "ignored.vs", "ps_name": "ignored.ps",
            "vs_content": original.vertex.source, "ps_content": original.pixel.source
        }))
        .unwrap();
        let actual = core::parse_ksh(&build_ksh_bytes(&params).unwrap()).unwrap();
        let mut expected = original.clone();
        expected.effect_name = "glow".to_owned();
        expected.vertex.source_name = "glow.vs".to_owned();
        expected.pixel.source_name = "glow.ps".to_owned();
        assert_eq!(actual, expected);
        assert_eq!(params.base_ksh.as_ref(), Some(&original));
    }

    #[test]
    fn free_editor_saves_unfinished_sources_without_a_stage() {
        let directory = unique_test_dir("free-editor");
        for extension in ["vs", "ps", "glsl", "txt"] {
            let path = directory.join(format!("draft.{extension}"));
            let mut params = SaveEditorSourceParams {
                path: path.to_string_lossy().into_owned(),
                content: "unfinished (".to_owned(),
                force: false,
            };
            save_editor_source_impl(&params).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), "unfinished (");
            params.content = "newer (".to_owned();
            assert!(save_editor_source_impl(&params).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), "unfinished (");
            params.force = true;
            save_editor_source_impl(&params).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), "newer (");
        }
        for name in ["blocked.ksh", "blocked.exe", "blocked"] {
            let path = directory.join(name);
            let params = SaveEditorSourceParams {
                path: path.to_string_lossy().into_owned(),
                content: "text".to_owned(),
                force: true,
            };
            assert!(save_editor_source_impl(&params).is_err());
            assert!(!path.exists());
        }
        let path = directory.join("nul.glsl");
        let params = SaveEditorSourceParams {
            path: path.to_string_lossy().into_owned(),
            content: "bad\0text".to_owned(),
            force: true,
        };
        assert!(save_editor_source_impl(&params).is_err());
        assert!(!path.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn ksh_export_respects_no_overwrite_and_preserves_legacy_force_default() {
        let directory = unique_test_dir("export-no-clobber");
        let output_path = directory.join("existing.ksh");
        fs::write(&output_path, b"keep the existing output").unwrap();
        let original = sample_metadata();
        let mut params: BuildKshParams = serde_json::from_value(serde_json::json!({
            "output_path": output_path,
            "force": false,
            "base_ksh": original,
            "vs_name": "original.vs",
            "vs_content": original.vertex.source,
            "ps_name": "original.ps",
            "ps_content": original.pixel.source
        }))
        .unwrap();
        assert!(build_ksh_impl(&params).is_err());
        assert_eq!(fs::read(&output_path).unwrap(), b"keep the existing output");

        params.force = None;
        build_ksh_impl(&params).unwrap();
        assert_eq!(
            fs::read(&output_path).unwrap(),
            core::encode_ksh(&original).unwrap()
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_save_preserves_unfinished_syntax_and_validates_the_whole_batch() {
        let directory = unique_test_dir("source-save");
        let vertex_path = directory.join("draft.vs");
        let pixel_path = directory.join("draft.ps");
        let mut params: SaveShaderSourcesParams = serde_json::from_value(serde_json::json!({
            "sources": [
                { "stage": "vs", "path": vertex_path, "content": "void main( unfinished" },
                { "stage": "ps", "path": pixel_path, "content": "bad\u{0}source" }
            ],
            "force": false
        }))
        .unwrap();
        assert!(save_shader_sources_impl(&params).is_err());
        assert!(!vertex_path.exists());
        assert!(!pixel_path.exists());

        params.sources[1].content = "also unfinished (".to_string();
        save_shader_sources_impl(&params).unwrap();
        assert_eq!(
            fs::read_to_string(&vertex_path).unwrap(),
            params.sources[0].content
        );
        assert_eq!(
            fs::read_to_string(&pixel_path).unwrap(),
            params.sources[1].content
        );
        assert!(save_shader_sources_impl(&params).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_save_rejects_invalid_stage_paths_and_cardinality_without_writes() {
        let directory = unique_test_dir("invalid-sources");
        let vertex_path = directory.join("draft.vs");
        for sources in [
            serde_json::json!([]),
            serde_json::json!([
                { "stage": "vs", "path": vertex_path, "content": "draft" },
                { "stage": "vs", "path": directory.join("other.vs"), "content": "draft" }
            ]),
            serde_json::json!([
                { "stage": "vs", "path": vertex_path, "content": "draft" },
                { "stage": "ps", "path": directory.join("wrong.vs"), "content": "draft" }
            ]),
            serde_json::json!([
                { "stage": "vs", "path": vertex_path, "content": "draft" },
                { "stage": "ps", "path": "bad\u{0}.ps", "content": "draft" }
            ]),
            serde_json::json!([
                { "stage": "vs", "path": vertex_path, "content": "draft" },
                { "stage": "ps", "path": directory.join("draft.ps"), "content": "draft" },
                { "stage": "vs", "path": directory.join("extra.vs"), "content": "draft" }
            ]),
        ] {
            let params: SaveShaderSourcesParams = serde_json::from_value(serde_json::json!({
                "sources": sources,
                "force": true
            }))
            .unwrap();
            assert!(save_shader_sources_impl(&params).is_err());
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        }
        fs::remove_dir_all(directory).unwrap();
    }

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
