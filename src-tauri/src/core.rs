use crate::glsl_parser::parse_glsl_uniforms;
use crate::types::{Variable, VariableScope};
use clap::Error;
use glsl_lang::ast::TypeSpecifierNonArrayData;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

fn read_u32(cursor: &mut std::io::Cursor<&[u8]>) -> io::Result<u32> {
    let mut buffer = [0; 4];
    cursor.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

fn read_string(cursor: &mut std::io::Cursor<&[u8]>) -> io::Result<String> {
    let length = read_u32(cursor)? as usize;
    let mut buffer = vec![0; length];
    cursor.read_exact(&mut buffer)?;
    Ok(String::from_utf8(buffer).expect("Invalid UTF-8 sequence"))
}

fn read_variable(cursor: &mut std::io::Cursor<&[u8]>) -> Result<Variable, Error> {
    let mut var = Variable::new();
    var.name = read_string(cursor)?;
    var.set_scope(read_u32(cursor)?)?;
    var.set_type(read_u32(cursor)?)?;
    let length = read_u32(cursor)?;
    var.array_length = if length > 1 { Some(length) } else { None };
    if var.r#type != TypeSpecifierNonArrayData::Sampler2D {
        let data_length = read_u32(cursor)?;
        var.default_data = (0..data_length)
            .map(|_| read_u32(cursor).unwrap())
            .collect();
    }
    Ok(var)
}

/// 解析 KSH 文件内容，返回着色器内容
pub fn analyze_ksh(
    content: &[u8],
) -> Result<(String, String, String, String), Box<dyn std::error::Error>> {
    let mut cursor = std::io::Cursor::new(content);
    let file_name = read_string(&mut cursor)?;
    let uniforms_count = read_u32(&mut cursor)?;
    log::debug!("Uniforms数量: {}", uniforms_count);

    let uniforms: Vec<Variable> = (0..uniforms_count)
        .map(|_| read_variable(&mut cursor).expect("Failed to read variable"))
        .collect();

    let vs_name = read_string(&mut cursor)?;
    let mut vs_content = read_string(&mut cursor)?;
    vs_content.pop(); // 移除file_content最后的u8 0

    let ps_name = read_string(&mut cursor)?;
    let mut ps_content = read_string(&mut cursor)?;
    ps_content.pop();

    // 读取并忽略 uniforms 引用
    let _vs_uniforms: Result<Vec<String>, io::Error> = (0..read_u32(&mut cursor)?)
        .map(|_| {
            let index = read_u32(&mut cursor)?;
            Ok(uniforms[index as usize].name.clone())
        })
        .collect();

    let _ps_uniforms: Result<Vec<String>, io::Error> = (0..read_u32(&mut cursor)?)
        .map(|_| {
            let index = read_u32(&mut cursor)?;
            Ok(uniforms[index as usize].name.clone())
        })
        .collect();

    // 忽略剩余的 uniform pointers
    let _ = (0..)
        .map(|_| read_u32(&mut cursor))
        .collect::<Result<Vec<_>, _>>();

    Ok((vs_name, vs_content, ps_name, ps_content))
}

/// 分析 KSH 文件并输出着色器文件
pub fn analyze_ksh_file(
    file_path: &Path,
    out_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("分析文件: {:?}", file_path);
    let content = fs::read(file_path)?;
    let (vs_name, vs_content, ps_name, ps_content) = analyze_ksh(&content)?;

    let vs_file_path = out_path.join(vs_name);
    if !force && vs_file_path.exists() {
        return Err(format!("输出文件已存在: {}", vs_file_path.display()).into());
    }
    let ps_file_path = out_path.join(ps_name);
    if !force && ps_file_path.exists() {
        return Err(format!("输出文件已存在: {}", ps_file_path.display()).into());
    }

    fs::write(vs_file_path, vs_content)?;
    fs::write(ps_file_path, ps_content)?;

    log::info!("分析完成");
    Ok(())
}

pub fn build_ksh(
    file_name: &str,
    vs_name: &str,
    vs_content: &str,
    ps_name: &str,
    ps_content: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();

    // 写入文件名
    buffer.extend_from_slice(&(file_name.len() as u32).to_le_bytes());
    buffer.extend_from_slice(file_name.as_bytes());

    // 从着色器文件中解析uniforms
    let vs_uniforms = parse_glsl_uniforms(vs_content)?;
    let ps_uniforms = parse_glsl_uniforms(ps_content)?;
    // v _ps 里如果有重复声明的uniform,那么只保留vs里的, ps里的删除
    // 合并vs_uniforms和ps_uniforms, 重复的只保留vs里的
    let mut uniforms = Vec::new();
    uniforms.extend(&vs_uniforms);
    for ps_uniform in &ps_uniforms {
        if !uniforms.iter().any(|u| u.name == ps_uniform.name) {
            uniforms.push(ps_uniform);
        }
    }
    // 写入uniforms 先写长度, 再写内容
    buffer.extend_from_slice(&(uniforms.len() as u32).to_le_bytes());
    for uniform in uniforms.iter() {
        // 变量名长度
        buffer.extend_from_slice(&(uniform.name.len() as u32).to_le_bytes());
        // 变量名
        buffer.extend_from_slice(uniform.name.as_bytes());
        // 变量作用域
        buffer.extend_from_slice(&(VariableScope::UNIFORM as u32).to_le_bytes());
        // 变量类型
        buffer.extend_from_slice(&uniform.get_type_id().to_le_bytes());
        // 变量数组长度
        buffer.extend_from_slice(&(uniform.array_length.unwrap_or(1)).to_le_bytes());
        if uniform.r#type != TypeSpecifierNonArrayData::Sampler2D {
            if uniform.array_length.is_none() {
                let default_data_length = uniform.default_data_length();
                buffer.extend_from_slice(&(default_data_length as u32).to_le_bytes());
                for _ in 0..default_data_length {
                    buffer.extend_from_slice(&0u32.to_le_bytes());
                }
            } else {
                buffer.extend_from_slice(&(0 as u32).to_le_bytes());
            }
        }
    }

    // 写入顶点着色器长度
    buffer.extend_from_slice(&(vs_name.len() as u32).to_le_bytes());
    // 写入顶点着色器内容
    buffer.extend_from_slice(vs_name.as_bytes());
    // 写入顶点着色器内容长度
    buffer.extend_from_slice(&((vs_content.len() as u32) + 1).to_le_bytes());
    // 写入顶点着色器内容
    buffer.extend_from_slice(vs_content.as_bytes());
    // 写入顶点着色器内容结束符
    buffer.push(0);
    // 写入像素着色器
    buffer.extend_from_slice(&(ps_name.len() as u32).to_le_bytes());
    buffer.extend_from_slice(ps_name.as_bytes());
    buffer.extend_from_slice(&((ps_content.len() as u32) + 1).to_le_bytes());
    buffer.extend_from_slice(ps_content.as_bytes());
    buffer.push(0);
    // 写入顶点着色器的uniforms引用
    buffer.extend_from_slice(&(vs_uniforms.len() as u32).to_le_bytes());
    for uniform in &vs_uniforms {
        if let Some(index) = uniforms.iter().position(|u| u.name == uniform.name) {
            buffer.extend_from_slice(&(index as u32).to_le_bytes());
        } else {
            return Err(format!("Uniform {} not found in uniforms", uniform.name).into());
        }
    }
    // 写入像素着色器的uniforms引用
    buffer.extend_from_slice(&(ps_uniforms.len() as u32).to_le_bytes());
    for uniform in &ps_uniforms {
        if let Some(index) = uniforms.iter().position(|u| u.name == uniform.name) {
            buffer.extend_from_slice(&(index as u32).to_le_bytes());
        } else {
            return Err(format!("Uniform {} not found in uniforms", uniform.name).into());
        }
    }
    Ok(buffer)
}

pub fn get_ps_vs_from_dir(path: &Path) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let mut vs_path = None;
    let mut ps_path = None;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("vs") {
            vs_path = Some(path);
        } else if path.extension().and_then(|s| s.to_str()) == Some("ps") {
            ps_path = Some(path);
        }
    }
    match (vs_path, ps_path) {
        (Some(vs), Some(ps)) => Ok((vs, ps)),
        _ => Err("目录必须包含恰好两个着色器文件（.vs/.ps）".into()),
    }
}

pub fn build_ksh_file_from_dir<'a>(
    dir_path: &'a Path,
    out_path: &'a Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (vs_path, ps_path) = get_ps_vs_from_dir(dir_path)?;
    build_ksh_file(&vs_path, &ps_path, out_path).map_err(|e| format!("构建着色器时出错: {}", e))?;
    Ok(())
}

pub fn build_ksh_file(
    vs_file: &Path,
    ps_file: &Path,
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 读取文件内容，附带自定义错误信息
    let read_file = |path: &Path| -> Result<String, Box<dyn std::error::Error>> {
        let mut content = String::new();
        File::open(path)
            .map_err(|e| format!("无法打开文件 {}: {}", path.display(), e))?
            .read_to_string(&mut content)
            .map_err(|e| format!("读取文件 {} 失败: {}", path.display(), e))?;
        Ok(content)
    };

    let vs_content = read_file(vs_file)?;
    let ps_content = read_file(ps_file)?;

    // 处理文件名，附带自定义错误信息
    let vs_name = vs_file
        .file_name()
        .ok_or_else(|| format!("无效的顶点着色器路径: {}", vs_file.display()))?
        .to_str()
        .ok_or_else(|| format!("顶点着色器文件名包含非法UTF-8字符: {}", vs_file.display()))?;
    let ps_name = ps_file
        .file_name()
        .ok_or_else(|| format!("无效的像素着色器路径: {}", ps_file.display()))?
        .to_str()
        .ok_or_else(|| format!("像素着色器文件名包含非法UTF-8字符: {}", ps_file.display()))?;

    let mut file = File::create(out_path)
        .map_err(|e| format!("创建文件 {} 失败: {}", out_path.display(), e))?;
    // 从out里解析file_name, 不要扩展名
    let file_name = out_path
        .file_stem()
        .ok_or_else(|| format!("无效的输出路径: {}", out_path.display()))?
        .to_str()
        .ok_or_else(|| format!("输出路径包含非法UTF-8字符: {}", out_path.display()))?;
    let buffer = build_ksh(file_name, vs_name, &vs_content, ps_name, &ps_content)?;
    file.write_all(&buffer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::warn;
    use std::env::temp_dir;
    use std::fs::{self, read_dir};
    use std::path::Path;

    // 测试辅助函数
    struct TestContext {
        temp_dir: PathBuf,
    }

    impl TestContext {
        fn new() -> Self {
            let temp_dir = temp_dir().join("ksh_test");
            fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
            Self { temp_dir }
        }

        fn cleanup(&self) {
            if self.temp_dir.exists() {
                fs::remove_dir_all(&self.temp_dir).expect("Failed to remove temp directory");
            }
        }

        fn assert_files_equal(&self, file1: &Path, file2: &Path) {
            let content1 = fs::read(file1).expect("Failed to read file1");
            let content2 = fs::read(file2).expect("Failed to read file2");

            if content1 != content2 {
                let min_len = content1.len().min(content2.len());
                for i in 0..min_len {
                    if content1[i] != content2[i] {
                        let start = if i >= 10 { i - 10 } else { 0 };
                        let end = (i + 10).min(min_len);
                        warn!("Difference found at byte {}:", i);
                        warn!("File 1: {:?}", &content1[start..end]);
                        warn!("File 2: {:?}", &content2[start..end]);
                        break;
                    }
                }
                if content1.len() != content2.len() {
                    warn!(
                        "Files have different lengths: file1 = {}, file2 = {}",
                        content1.len(),
                        content2.len()
                    );
                }
                panic!("Files are not equal: {:?} and {:?}", file1, file2);
            }
        }
    }

    // GLSL 解析测试
    mod glsl_tests {
        use super::*;
        use crate::glsl_parser::parse_glsl_uniforms;

        #[test]
        fn test_parse_glsl() {
            let file_content = fs::read_to_string("C:\\Saved Games\\Steam\\steamapps\\common\\Don't Starve Together\\data\\databundles\\shaders\\out\\anim.ps").unwrap();
            let uniforms = parse_glsl_uniforms(&file_content).unwrap();
            println!("uniforms: {:?}", uniforms);
        }
    }

    // KSH 构建测试
    mod build_tests {
        use super::*;

        #[test]
        fn test_build_ksh_file() {
            let ctx = TestContext::new();
            let vs_file = Path::new(
                "C:\\Users\\Tohsa\\AppData\\Local\\Temp\\ksh_test\\anim_bloom_haunted\\anim.vs",
            );
            let ps_file = Path::new(
                "C:\\Users\\Tohsa\\AppData\\Local\\Temp\\ksh_test\\anim_bloom_haunted\\anim.ps",
            );
            let ksh_file = Path::new("C:\\Saved Games\\Steam\\steamapps\\common\\Don't Starve Together\\data\\databundles\\shaders\\anim_bloom_haunted.ksh");

            let out_path = ctx.temp_dir.join(format!(
                "{}.ksh",
                ksh_file
                    .file_stem()
                    .expect("Invalid file name")
                    .to_str()
                    .expect("Invalid UTF-8 sequence")
            ));
            build_ksh_file(vs_file, ps_file, &out_path).expect("Failed to build ksh file");
            ctx.assert_files_equal(&ksh_file, &out_path);

            ctx.cleanup();
        }

        #[test]
        fn test_build_ksh_file_from_dir() {
            let ctx = TestContext::new();
            let test_dir = ctx.temp_dir.join("test_shader");
            fs::create_dir_all(&test_dir).expect("Failed to create test directory");

            // 创建测试用的着色器文件
            let vs_content = "uniform mat4 u_ModelViewProjectionMatrix;\nvoid main() {}\n";
            let ps_content = "uniform sampler2D u_Texture;\nvoid main() {}\n";
            fs::write(test_dir.join("test.vs"), vs_content).expect("Failed to write vs file");
            fs::write(test_dir.join("test.ps"), ps_content).expect("Failed to write ps file");

            let out_path = ctx.temp_dir.join("test.ksh");
            build_ksh_file_from_dir(&test_dir, &out_path).expect("Failed to build ksh file");

            // 验证输出文件存在
            assert!(out_path.exists(), "Output file should exist");

            ctx.cleanup();
        }
    }

    // KSH 分析测试
    mod analyze_tests {
        use super::*;

        #[test]
        fn test_analyze_ksh_file() {
            let ctx = TestContext::new();
            let ksh_file = Path::new("C:\\Saved Games\\Steam\\steamapps\\common\\Don't Starve Together\\data\\databundles\\shaders\\anim_bloom_haunted.ksh");
            let out_dir = ctx.temp_dir.join("analyze_output");
            fs::create_dir_all(&out_dir).expect("Failed to create temp directory");
            println!("out_dir: {:?}", out_dir);
            analyze_ksh_file(ksh_file, &out_dir, true).expect("Failed to analyze ksh file");

            // 验证输出文件存在
            assert!(out_dir.exists(), "Output directory should exist");
            assert!(
                out_dir.join("anim.vs").exists(),
                "Vertex shader should exist"
            );
            assert!(
                out_dir.join("anim.ps").exists(),
                "Pixel shader should exist"
            );

            ctx.cleanup();
        }
    }

    // 完整转换测试
    mod conversion_tests {
        use super::*;

        #[test]
        fn test_full_conversion() {
            let ctx = TestContext::new();
            let folder_path = Path::new("C:\\Saved Games\\Steam\\steamapps\\common\\Don't Starve Together\\data\\databundles\\shaders");

            for entry in read_dir(folder_path).expect("Failed to read directory") {
                let entry = entry.expect("Failed to read entry");
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ksh") {
                    let file_name = path
                        .file_name()
                        .expect("Failed to get file name")
                        .to_str()
                        .expect("Invalid UTF-8 sequence");
                    println!("now analyzing: {}", file_name);

                    let ps_vs_dir = ctx.temp_dir.join(file_name.replace(".ksh", ""));
                    fs::create_dir_all(&ps_vs_dir).expect("Failed to create temp directory");

                    analyze_ksh_file(&path, &ps_vs_dir, true).expect("Failed to analyze ksh file");
                    let temp_dir = ctx.temp_dir.join(file_name);
                    build_ksh_file_from_dir(&ps_vs_dir, &temp_dir)
                        .expect("Failed to build ksh file");

                    ctx.assert_files_equal(&path, &temp_dir);
                }
            }

            ctx.cleanup();
        }
    }
}
