use clap::{Arg, Command};
use log::{debug, info, warn, error};
use serde::{Deserialize, Serialize};
use std::fs::{self, write, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::convert::{TryFrom, TryInto};

#[derive(Debug, Deserialize, Serialize)]
struct UniformConfig {
    name: String,
    #[serde(rename = "type")]
    type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    array_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_data: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ShaderStageConfig {
    file: String,
    uniforms: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ShaderConfig {
    vs: ShaderStageConfig,
    ps: ShaderStageConfig,
    uniforms: Vec<UniformConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    shaders: Vec<ShaderConfig>,
}

#[derive(Debug)]
enum VariableScope {
    UNIFORM,
}

impl VariableScope {
    fn from_u32(value: u32) -> Result<Self, String> {
        match value {
            0 => Ok(VariableScope::UNIFORM),
            _ => Err(format!("Invalid scope: {}", value)),
        }
    }
}

#[derive(Debug, PartialEq)]
enum VariableType {
    Mat4,
    Vec4,
    Vec3,
    Vec2,
    Float,
    Sampler2D,
}

impl TryFrom<&str> for VariableType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "float" => Ok(VariableType::Float),
            "vec2" => Ok(VariableType::Vec2),
            "vec3" => Ok(VariableType::Vec3),
            "vec4" => Ok(VariableType::Vec4),
            "mat4" => Ok(VariableType::Mat4),
            "sampler2D" => Ok(VariableType::Sampler2D),
            _ => Err(format!("Invalid type: {}", value)),
        }
    }
}

impl TryFrom<u32> for VariableType {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(VariableType::Float),
            2 => Ok(VariableType::Vec2),
            3 => Ok(VariableType::Vec3),
            4 => Ok(VariableType::Vec4),
            20 => Ok(VariableType::Mat4),
            43 => Ok(VariableType::Sampler2D),
            _ => Err(format!("Invalid type: {}", value)),
        }
    }
}

impl From<&VariableType> for u32 {
    fn from(var_type: &VariableType) -> Self {
        match var_type {
            VariableType::Float => 0,
            VariableType::Vec2 => 2,
            VariableType::Vec3 => 3,
            VariableType::Vec4 => 4,
            VariableType::Mat4 => 20,
            VariableType::Sampler2D => 43,
        }
    }
}

impl From<&VariableType> for String {
    fn from(var_type: &VariableType) -> Self {
        match var_type {
            VariableType::Float => "float".to_string(),
            VariableType::Vec2 => "vec2".to_string(),
            VariableType::Vec3 => "vec3".to_string(),
            VariableType::Vec4 => "vec4".to_string(),
            VariableType::Mat4 => "mat4".to_string(),
            VariableType::Sampler2D => "sampler2D".to_string(),
        }
    }
}

impl VariableType {
    fn default_data_length(&self) -> usize {
        match self {
            VariableType::Float => 1,
            VariableType::Vec2 => 2,
            VariableType::Vec3 => 3,
            VariableType::Vec4 => 4,
            VariableType::Mat4 => 16,
            VariableType::Sampler2D => 0,
        }
    }
}

#[derive(Debug)]
struct Variable {
    name: String,
    type_: VariableType,
    default_data: Vec<u32>,
    scope: VariableScope,
    array_length: u32,
}

impl Variable {
    fn new() -> Self {
        Variable {
            name: String::new(),
            type_: VariableType::Float,
            default_data: vec![],
            scope: VariableScope::UNIFORM,
            array_length: 1,
        }
    }

    fn set_type(&mut self, value: u32) {
        self.type_ = value.try_into().expect("Invalid type");
    }

    fn set_scope(&mut self, value: u32) {
        self.scope = VariableScope::from_u32(value).expect("Invalid scope");
    }
}

fn read_u32(file: &mut File) -> io::Result<u32> {
    let mut buffer = [0; 4];
    file.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

fn read_string(file: &mut File) -> io::Result<String> {
    let length = read_u32(file)? as usize;
    let mut buffer = vec![0; length];
    file.read_exact(&mut buffer)?;
    Ok(String::from_utf8(buffer).expect("Invalid UTF-8 sequence"))
}

fn read_variable(file: &mut File) -> io::Result<Variable> {
    let mut var = Variable::new();
    var.name = read_string(file)?;
    var.set_scope(read_u32(file)?);
    var.set_type(read_u32(file)?);
    var.array_length = read_u32(file)?;
    if var.type_ != VariableType::Sampler2D {
        let data_length = read_u32(file)?;
        var.default_data = (0..data_length).map(|_| read_u32(file).unwrap()).collect();
    }
    Ok(var)
}

fn variable_to_uniform_config(var: Variable) -> UniformConfig {
    let default_data = if var.default_data.iter().all(|&x| x == 0)
        && var.default_data.len() == var.type_.default_data_length()
    {
        None
    } else {
        Some(var.default_data)
    };

    UniformConfig {
        name: var.name,
        type_: (&var.type_).into(),
        array_length: if var.array_length == 1 {
            None
        } else {
            Some(var.array_length)
        },
        default_data,
    }
}

fn analyze_ksh(file_path: &Path, out_path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    info!("Analyzing file: {:?}", file_path);
    let mut file = File::open(file_path)?;
    let file_name = read_string(&mut file)?;
    debug!("File name: {}", file_name);
    let uniforms_count = read_u32(&mut file)?;
    debug!("Uniforms count: {}", uniforms_count);

    let uniforms: Vec<Variable> = (0..uniforms_count)
        .map(|_| read_variable(&mut file).expect("Failed to read variable"))
        .collect();

    let vs_name = read_string(&mut file)?;
    let mut vs_content = read_string(&mut file)?;
    vs_content.pop(); // 移除file_content最后的u8 0

    let ps_name = read_string(&mut file)?;
    let mut ps_content = read_string(&mut file)?;
    ps_content.pop();

    let vs_uniforms: Result<Vec<String>, io::Error> = (0..read_u32(&mut file)?)
        .map(|_| {
            let index = read_u32(&mut file)?;
            Ok(uniforms[index as usize].name.clone())
        })
        .collect();

    let ps_uniforms: Result<Vec<String>, io::Error> = (0..read_u32(&mut file)?)
        .map(|_| {
            let index = read_u32(&mut file)?;
            Ok(uniforms[index as usize].name.clone())
        })
        .collect();

    let shader_config = ShaderConfig {
        vs: ShaderStageConfig {
            file: vs_name.clone(),
            uniforms: vs_uniforms?,
        },
        ps: ShaderStageConfig {
            file: ps_name.clone(),
            uniforms: ps_uniforms?,
        },
        uniforms: uniforms
            .into_iter()
            .map(variable_to_uniform_config)
            .collect(),
    };

    let config = Config {
        shaders: vec![shader_config],
    };

    let yaml_file_name = if file_name.ends_with(".ksh") {
        file_name.trim_end_matches(".ksh")
    } else {
        &file_name
    };
    let yaml_file_path = out_path.join(format!("{}.yaml", yaml_file_name));
    let yaml_file = File::create(&yaml_file_path)?;
    serde_yaml::to_writer(yaml_file, &config)?;

    write(out_path.join(vs_name), vs_content)?;
    write(out_path.join(ps_name), ps_content)?;

    debug!(
        "Uniform pointers: {:?}",
        (0..)
            .map(|_| read_u32(&mut file))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default()
    );
    info!("Analysis complete");
    Ok(yaml_file_path)
}

fn build_ksh(config_file: &Path, out_path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_content = fs::read_to_string(config_file)?;
    let mut config: Config = serde_yaml::from_str(&config_content)?;

    for shader in &mut config.shaders {
        for uniform in &mut shader.uniforms {
            if uniform.array_length.is_none() {
                uniform.array_length = Some(1);
            }
        }
    }

    let mut ksh_file_path = PathBuf::new();
    for shader in config.shaders {
        let vs_file = config_file.parent().unwrap().join(&shader.vs.file);
        let ps_file = config_file.parent().unwrap().join(&shader.ps.file);

        debug!("VS file path: {:?}", vs_file);
        debug!("PS file path: {:?}", ps_file);

        if !vs_file.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("VS file not found: {:?}", vs_file),
            )
            .into());
        }
        if !ps_file.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("PS file not found: {:?}", ps_file),
            )
            .into());
        }

        let mut vs_content = String::new();
        let mut ps_content = String::new();
        File::open(&vs_file)?.read_to_string(&mut vs_content)?;
        File::open(&ps_file)?.read_to_string(&mut ps_content)?;

        let file_name = config_file
            .file_stem()
            .expect("Invalid file name")
            .to_str()
            .expect("Invalid UTF-8 sequence");
        ksh_file_path = out_path.join(format!("{}.ksh", file_name));
        let mut file = File::create(&ksh_file_path)?;

        file.write_all(&(file_name.len() as u32).to_le_bytes())?;
        file.write_all(file_name.as_bytes())?;

        file.write_all(&(shader.uniforms.len() as u32).to_le_bytes())?;
        for uniform in &shader.uniforms {
            file.write_all(&(uniform.name.len() as u32).to_le_bytes())?;
            file.write_all(uniform.name.as_bytes())?;
            file.write_all(&(VariableScope::UNIFORM as u32).to_le_bytes())?;
            let var_type: VariableType = uniform.type_.as_str().try_into()?;
            file.write_all(&u32::from(&var_type).to_le_bytes())?;
            file.write_all(&(uniform.array_length.unwrap_or(1)).to_le_bytes())?;
            if var_type != VariableType::Sampler2D {
                let default_data = uniform
                    .default_data
                    .clone()
                    .unwrap_or_else(|| vec![0; var_type.default_data_length()]);
                file.write_all(&(default_data.len() as u32).to_le_bytes())?;
                for value in default_data {
                    file.write_all(&value.to_le_bytes())?;
                }
            }
        }

        file.write_all(
            &(vs_file.file_name().unwrap().to_str().unwrap().len() as u32).to_le_bytes(),
        )?;
        file.write_all(vs_file.file_name().unwrap().to_str().unwrap().as_bytes())?;
        file.write_all(&((vs_content.len() as u32) + 1).to_le_bytes())?;
        file.write_all(vs_content.as_bytes())?;
        file.write_all(&[0])?;

        file.write_all(
            &(ps_file.file_name().unwrap().to_str().unwrap().len() as u32).to_le_bytes(),
        )?;
        file.write_all(ps_file.file_name().unwrap().to_str().unwrap().as_bytes())?;
        file.write_all(&((ps_content.len() as u32) + 1).to_le_bytes())?;
        file.write_all(ps_content.as_bytes())?;
        file.write_all(&[0])?;

        let uniform_map: std::collections::HashMap<_, _> = shader
            .uniforms
            .iter()
            .enumerate()
            .map(|(i, u)| (&u.name, i as u32))
            .collect();

        let vs_uniform_count = shader.vs.uniforms.len() as u32;
        file.write_all(&vs_uniform_count.to_le_bytes())?;
        for uniform_name in &shader.vs.uniforms {
            if let Some(&index) = uniform_map.get(uniform_name) {
                file.write_all(&index.to_le_bytes())?;
            } else {
                return Err(format!("Uniform {} not found in vs uniforms", uniform_name).into());
            }
        }

        let ps_uniform_count = shader.ps.uniforms.len() as u32;
        file.write_all(&ps_uniform_count.to_le_bytes())?;
        for uniform_name in &shader.ps.uniforms {
            if let Some(&index) = uniform_map.get(uniform_name) {
                file.write_all(&index.to_le_bytes())?;
            } else {
                return Err(format!("Uniform {} not found in ps uniforms", uniform_name).into());
            }
        }
    }

    Ok(ksh_file_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs::{self, read_dir};
    use std::path::Path;

    fn assert_files_equal(file1: &Path, file2: &Path) {
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

    #[test]
    fn test_conversion() {
        let folder_path = Path::new("C:\\Saved Games\\Steam\\steamapps\\common\\Don't Starve Together\\data\\databundles\\shaders");
        let temp_dir = temp_dir().join("ksh_test");
        fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

        for entry in read_dir(folder_path).expect("Failed to read directory") {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ksh") {
                let temp_yaml_path = analyze_ksh(&path, &temp_dir).expect("Failed to analyze ksh");
                let temp_ksh_path =
                    build_ksh(&temp_yaml_path, &temp_dir).expect("Failed to build ksh");

                // Compare original ksh with converted ksh
                assert_files_equal(&path, &temp_ksh_path);

                // Clean up temporary files
                fs::remove_file(temp_yaml_path).expect("Failed to remove temp yaml file");
                fs::remove_file(temp_ksh_path).expect("Failed to remove temp ksh file");
            }
        }

        // Clean up temporary directory
        fs::remove_dir_all(&temp_dir).expect("Failed to remove temp directory");
    }
}

fn main() {
    let matches = Command::new("ksh-analyzer")
        .version("0.1.0")
        .author("TohsakaKuro<tohsakakuro@outlook.com>")
        .about("A tool to analyze and build Don't Starve Together shader files")
        .arg(
            Arg::new("input")
                .help("The input ksh or yaml file to analyze or build. \
                       If the input file is a .ksh file, it will be analyzed and converted to a .yaml file. \
                       If the input file is a .yaml file, it will be built into a .ksh file.")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("output")
                .help("The output path to write the result. \
                       If not specified, the current directory will be used.")
                .required(false)
                .index(2),
        )
        .arg(
            Arg::new("debug")
                .help("Enable debug logging for more detailed output.")
                .required(false)
                .long("debug")
                .short('d')
                .action(clap::ArgAction::SetTrue),
        )
        .after_help("EXAMPLES:\n\
                     \n\
                     Analyze a .ksh file and output the result to the current directory:\n\
                     \tksh-analyzer input.ksh\n\
                     \n\
                     Analyze a .ksh file and specify the output directory:\n\
                     \tksh-analyzer input.ksh output_dir\n\
                     \n\
                     Build a .yaml file into a .ksh file and output the result to the current directory:\n\
                     \tksh-analyzer input.yaml\n\
                     \n\
                     Build a .yaml file into a .ksh file and specify the output directory:\n\
                     \tksh-analyzer input.yaml output_dir\n\
                     \n\
                     Enable debug logging:\n\
                     \tksh-analyzer input.ksh --debug")
        .get_matches();

    if matches.contains_id("debug") {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .format(|buf, record| {
                writeln!(buf, "{}: {}", record.level(), record.args())
            })
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .format(|buf, record| {
                writeln!(buf, "{}: {}", record.level(), record.args())
            })
            .init();
    }

    let input = matches
        .get_one::<String>("input")
        .expect("Input file is required");
    let input_path = Path::new(input);
    let output = matches
        .get_one::<String>("output")
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));

    if !input_path.is_file() {
        error!("Input path is not a file");
        return;
    }

    if !output.exists() {
        fs::create_dir_all(output).expect("Failed to create output directory");
    } else if !output.is_dir() {
        error!("Output path is not a directory");
        return;
    }

    if input_path.extension().and_then(|s| s.to_str()) == Some("ksh") {
        if let Err(e) = analyze_ksh(input_path, output) {
            error!("Error analyzing file: {}", e);
        }
    } else if input_path.extension().and_then(|s| s.to_str()) == Some("yaml") {
        if let Err(e) = build_ksh(input_path, output) {
            error!("Error building file: {}", e);
        }
    } else {
        error!("Invalid input file type. Expected .ksh or .yaml");
    }

    info!("All tasks completed");
}
