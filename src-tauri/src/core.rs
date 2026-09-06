use crate::glsl_parser::parse_glsl_uniforms;
use crate::types::{
    KshFile, KshStage, KshUniform, KshUniformType, UniformDeclaration, VariableScope,
};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MIN_UNIFORM_BYTES: usize = 16;
const MAX_SAMPLER_COUNT: u32 = 8;
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KshError {
    offset: Option<usize>,
    message: String,
}

impl KshError {
    fn at(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset: Some(offset),
            message: message.into(),
        }
    }

    fn message(message: impl Into<String>) -> Self {
        Self {
            offset: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for KshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.offset {
            Some(offset) => write!(formatter, "KSH 格式错误 @ 0x{offset:X}: {}", self.message),
            None => write!(formatter, "KSH 错误: {}", self.message),
        }
    }
}

impl std::error::Error for KshError {}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }

    fn take(&mut self, size: usize, field: &str) -> Result<&'a [u8], KshError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| KshError::at(self.offset, format!("{field} 长度溢出")))?;
        if end > self.data.len() {
            return Err(KshError::at(
                self.offset,
                format!(
                    "读取 {field} 越过文件末尾，需要 {size} 字节，仅剩 {} 字节",
                    self.remaining()
                ),
            ));
        }
        let result = &self.data[self.offset..end];
        self.offset = end;
        Ok(result)
    }

    fn u32(&mut self, field: &str) -> Result<u32, KshError> {
        let offset = self.offset;
        let bytes = self.take(4, field)?;
        let array: [u8; 4] = bytes
            .try_into()
            .map_err(|_| KshError::at(offset, format!("{field} 不是 4 字节整数")))?;
        Ok(u32::from_le_bytes(array))
    }

    fn count(&mut self, field: &str, minimum_item_size: usize) -> Result<usize, KshError> {
        let count_offset = self.offset;
        let raw = self.u32(field)?;
        let count = usize::try_from(raw)
            .map_err(|_| KshError::at(count_offset, format!("{field} 无法转换为内存长度")))?;
        let minimum_bytes = count
            .checked_mul(minimum_item_size)
            .ok_or_else(|| KshError::at(count_offset, format!("{field} 乘法溢出")))?;
        if minimum_bytes > self.remaining() {
            return Err(KshError::at(
                count_offset,
                format!(
                    "{field}={count} 不可能容纳在剩余 {} 字节中",
                    self.remaining()
                ),
            ));
        }
        Ok(count)
    }

    fn text(&mut self, field: &str) -> Result<String, KshError> {
        let length = self.count(&format!("{field} 字节长度"), 1)?;
        let offset = self.offset;
        let bytes = self.take(length, field)?;
        if bytes.contains(&0) {
            return Err(KshError::at(offset, format!("{field} 含有 NUL 字节")));
        }
        let text = std::str::from_utf8(bytes).map_err(|error| {
            KshError::at(offset + error.valid_up_to(), format!("{field} 不是 UTF-8"))
        })?;
        Ok(text.to_owned())
    }

    fn source(&mut self, field: &str) -> Result<String, KshError> {
        let length_offset = self.offset;
        let length = self.count(&format!("{field} 字节长度"), 1)?;
        if length == 0 {
            return Err(KshError::at(length_offset, format!("{field} 缺少末尾 NUL")));
        }
        let offset = self.offset;
        let bytes = self.take(length, field)?;
        if bytes.last() != Some(&0) {
            return Err(KshError::at(
                offset + length - 1,
                format!("{field} 不是以 NUL 结尾"),
            ));
        }
        let body = &bytes[..length - 1];
        if let Some(index) = body.iter().position(|byte| *byte == 0) {
            return Err(KshError::at(
                offset + index,
                format!("{field} 含有内嵌 NUL"),
            ));
        }
        let text = std::str::from_utf8(body).map_err(|error| {
            KshError::at(offset + error.valid_up_to(), format!("{field} 不是 UTF-8"))
        })?;
        Ok(text.to_owned())
    }
}

fn read_uniform(reader: &mut Reader<'_>, index: usize) -> Result<KshUniform, KshError> {
    let name = reader.text(&format!("uniform[{index}].name"))?;
    let scope_offset = reader.offset;
    let scope_code = reader.u32(&format!("uniform[{index}].scope"))?;
    let scope = VariableScope::from_u32(scope_code)
        .map_err(|message| KshError::at(scope_offset, message))?;
    let type_code = reader.u32(&format!("uniform[{index}].type"))?;
    let uniform_type = KshUniformType::try_from(type_code)
        .map_err(|message| KshError::at(reader.offset - 4, message))?;
    let array_offset = reader.offset;
    let array_count = reader.u32(&format!("uniform[{index}].array_count"))?;
    if array_count == 0 {
        return Err(KshError::at(
            array_offset,
            format!("uniform[{index}] 的 array_count 必须大于 0"),
        ));
    }

    let default_data = if uniform_type.is_sampler() {
        Vec::new()
    } else {
        let count = reader.count(&format!("uniform[{index}].default_count"), 4)?;
        let mut values = Vec::with_capacity(count);
        for value_index in 0..count {
            values.push(reader.u32(&format!("uniform[{index}].defaults[{value_index}]"))?);
        }
        values
    };

    Ok(KshUniform {
        name,
        scope,
        uniform_type,
        array_count,
        default_data,
    })
}

fn read_uniform_indices(
    reader: &mut Reader<'_>,
    stage: &str,
    uniform_count: usize,
) -> Result<Vec<u32>, KshError> {
    let count = reader.count(&format!("{stage} uniform 数量"), 4)?;
    let mut indices = Vec::with_capacity(count);
    for item in 0..count {
        let offset = reader.offset;
        let index = reader.u32(&format!("{stage} uniform 索引[{item}]"))?;
        let index_usize = usize::try_from(index)
            .map_err(|_| KshError::at(offset, format!("{stage} uniform 索引无法转换")))?;
        if index_usize >= uniform_count {
            return Err(KshError::at(
                offset,
                format!("{stage} 引用了 uniform 索引 {index}，但表中只有 {uniform_count} 项"),
            ));
        }
        indices.push(index);
    }
    Ok(indices)
}

pub fn parse_ksh(content: &[u8]) -> Result<KshFile, KshError> {
    let mut reader = Reader::new(content);
    let effect_name = reader.text("effect name")?;
    let uniform_count = reader.count("uniform 数量", MIN_UNIFORM_BYTES)?;
    let mut uniforms = Vec::with_capacity(uniform_count);
    for index in 0..uniform_count {
        uniforms.push(read_uniform(&mut reader, index)?);
    }

    let vertex_source_name = reader.text("VS 文件名")?;
    let vertex_source = reader.source("VS 源码")?;
    let pixel_source_name = reader.text("PS 文件名")?;
    let pixel_source = reader.source("PS 源码")?;
    let vertex_uniform_indices = read_uniform_indices(&mut reader, "VS", uniforms.len())?;
    let pixel_uniform_indices = read_uniform_indices(&mut reader, "PS", uniforms.len())?;

    if reader.offset != content.len() {
        return Err(KshError::at(
            reader.offset,
            format!("文件末尾还有 {} 个未解析字节", reader.remaining()),
        ));
    }

    Ok(KshFile {
        effect_name,
        uniforms,
        vertex: KshStage {
            source_name: vertex_source_name,
            source: vertex_source,
            uniform_indices: vertex_uniform_indices,
        },
        pixel: KshStage {
            source_name: pixel_source_name,
            source: pixel_source,
            uniform_indices: pixel_uniform_indices,
        },
    })
}

fn checked_u32(value: usize, field: &str) -> Result<u32, KshError> {
    u32::try_from(value).map_err(|_| KshError::message(format!("{field} 超过 u32 可表示范围")))
}

fn write_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn write_text(buffer: &mut Vec<u8>, value: &str, field: &str) -> Result<(), KshError> {
    if value.as_bytes().contains(&0) {
        return Err(KshError::message(format!("{field} 含有 NUL 字节")));
    }
    write_u32(
        buffer,
        checked_u32(value.len(), &format!("{field} 字节长度"))?,
    );
    buffer.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_source(buffer: &mut Vec<u8>, value: &str, field: &str) -> Result<(), KshError> {
    if value.as_bytes().contains(&0) {
        return Err(KshError::message(format!("{field} 含有 NUL 字节")));
    }
    let length = value
        .len()
        .checked_add(1)
        .ok_or_else(|| KshError::message(format!("{field} 字节长度溢出")))?;
    write_u32(buffer, checked_u32(length, &format!("{field} 字节长度"))?);
    buffer.extend_from_slice(value.as_bytes());
    buffer.push(0);
    Ok(())
}

fn validate_stage_indices(
    stage: &KshStage,
    stage_name: &str,
    uniform_count: usize,
) -> Result<(), KshError> {
    checked_u32(
        stage.uniform_indices.len(),
        &format!("{stage_name} uniform 数量"),
    )?;
    for (item, index) in stage.uniform_indices.iter().copied().enumerate() {
        if usize::try_from(index).map_or(true, |value| value >= uniform_count) {
            return Err(KshError::message(format!(
                "{stage_name} uniform 索引[{item}]={index} 越界，表中只有 {uniform_count} 项"
            )));
        }
    }
    Ok(())
}

pub fn encode_ksh(ksh: &KshFile) -> Result<Vec<u8>, KshError> {
    checked_u32(ksh.uniforms.len(), "uniform 数量")?;
    validate_stage_indices(&ksh.vertex, "VS", ksh.uniforms.len())?;
    validate_stage_indices(&ksh.pixel, "PS", ksh.uniforms.len())?;

    let mut buffer = Vec::new();
    write_text(&mut buffer, &ksh.effect_name, "effect name")?;
    write_u32(
        &mut buffer,
        checked_u32(ksh.uniforms.len(), "uniform 数量")?,
    );
    for (index, uniform) in ksh.uniforms.iter().enumerate() {
        if uniform.array_count == 0 {
            return Err(KshError::message(format!(
                "uniform[{index}] {} 的 array_count 必须大于 0",
                uniform.name
            )));
        }
        if uniform.uniform_type.is_sampler() && !uniform.default_data.is_empty() {
            return Err(KshError::message(format!(
                "sampler uniform {} 不能包含默认值块",
                uniform.name
            )));
        }
        write_text(
            &mut buffer,
            &uniform.name,
            &format!("uniform[{index}].name"),
        )?;
        write_u32(&mut buffer, uniform.scope.code());
        write_u32(&mut buffer, uniform.uniform_type.id());
        write_u32(&mut buffer, uniform.array_count);
        if !uniform.uniform_type.is_sampler() {
            write_u32(
                &mut buffer,
                checked_u32(
                    uniform.default_data.len(),
                    &format!("uniform[{index}].default_count"),
                )?,
            );
            for value in &uniform.default_data {
                write_u32(&mut buffer, *value);
            }
        }
    }

    write_text(&mut buffer, &ksh.vertex.source_name, "VS 文件名")?;
    write_source(&mut buffer, &ksh.vertex.source, "VS 源码")?;
    write_text(&mut buffer, &ksh.pixel.source_name, "PS 文件名")?;
    write_source(&mut buffer, &ksh.pixel.source, "PS 源码")?;
    write_u32(
        &mut buffer,
        checked_u32(ksh.vertex.uniform_indices.len(), "VS uniform 数量")?,
    );
    for index in &ksh.vertex.uniform_indices {
        write_u32(&mut buffer, *index);
    }
    write_u32(
        &mut buffer,
        checked_u32(ksh.pixel.uniform_indices.len(), "PS uniform 数量")?,
    );
    for index in &ksh.pixel.uniform_indices {
        write_u32(&mut buffer, *index);
    }
    Ok(buffer)
}

/// 解析 KSH 文件内容，返回着色器内容
pub fn analyze_ksh(
    content: &[u8],
) -> Result<(String, String, String, String), Box<dyn std::error::Error>> {
    let ksh = parse_ksh(content)?;
    Ok((
        ksh.vertex.source_name,
        ksh.vertex.source,
        ksh.pixel.source_name,
        ksh.pixel.source,
    ))
}

fn is_windows_reserved_file_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default();
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
    ) || upper
        .strip_prefix("COM")
        .or_else(|| upper.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}

fn safe_stage_file_name(name: &str, expected_extension: &str) -> Result<OsString, KshError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(' ')
        || name.ends_with('.')
        || name.chars().any(|character| {
            character <= '\u{1f}'
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        || is_windows_reserved_file_name(name)
    {
        return Err(KshError::message(format!("不安全的着色器文件名: {name:?}")));
    }
    let path = Path::new(name);
    let mut components = path.components();
    let file_name = match (components.next(), components.next()) {
        (Some(Component::Normal(value)), None) => value,
        _ => return Err(KshError::message(format!("不安全的着色器文件名: {name:?}"))),
    };
    let extension = path.extension().and_then(OsStr::to_str).ok_or_else(|| {
        KshError::message(format!(
            "着色器文件名缺少 .{expected_extension} 扩展名: {name}"
        ))
    })?;
    if !extension.eq_ignore_ascii_case(expected_extension) {
        return Err(KshError::message(format!(
            "着色器文件扩展名应为 .{expected_extension}: {name}"
        )));
    }
    Ok(file_name.to_os_string())
}

fn sidecar_path(path: &Path, marker: &str) -> io::Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "输出路径没有文件名"))?;
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut sidecar_name = file_name.to_os_string();
    sidecar_name.push(format!(".{marker}.{}.{}", std::process::id(), sequence));
    Ok(parent.join(sidecar_name))
}

fn validate_replacement_target(out_path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(out_path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("输出路径已存在且不是普通文件: {}", out_path.display()),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn write_sibling_temp(out_path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let (temp_path, mut temp_file) = loop {
        let candidate = sidecar_path(out_path, "tmp")?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    if let Err(error) = temp_file
        .write_all(bytes)
        .and_then(|_| temp_file.sync_all())
    {
        drop(temp_file);
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    drop(temp_file);
    Ok(temp_path)
}

fn write_bytes_atomically(out_path: &Path, bytes: &[u8]) -> io::Result<()> {
    validate_replacement_target(out_path)?;
    let temp_path = write_sibling_temp(out_path, bytes)?;

    match fs::rename(&temp_path, out_path) {
        Ok(()) => return Ok(()),
        Err(error) if !out_path.exists() => {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        Err(_) => {}
    }

    if !out_path.is_file() {
        let _ = fs::remove_file(&temp_path);
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出路径已存在且不是普通文件: {}", out_path.display()),
        ));
    }

    // Windows cannot rename over an existing file. Keep the old file as a
    // recoverable sibling until the fully-written replacement is in place.
    let backup_path = loop {
        let candidate = sidecar_path(out_path, "backup")?;
        if !candidate.exists() {
            break candidate;
        }
    };
    if let Err(error) = fs::rename(out_path, &backup_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp_path, out_path) {
        let restore_result = fs::rename(&backup_path, out_path);
        let _ = fs::remove_file(&temp_path);
        return match restore_result {
            Ok(()) => Err(error),
            Err(restore_error) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "替换输出失败 ({error})，恢复旧文件也失败 ({restore_error})；旧文件位于 {}",
                    backup_path.display()
                ),
            )),
        };
    }
    if let Err(error) = fs::remove_file(&backup_path) {
        log::warn!(
            "新文件已写入，但无法删除备份 {}: {error}",
            backup_path.display()
        );
    }
    Ok(())
}

struct StagedReplacement {
    target: PathBuf,
    temp: PathBuf,
    backup: Option<PathBuf>,
    installed: bool,
}

fn rollback_replacements(entries: &mut [StagedReplacement]) -> Vec<String> {
    let mut errors = Vec::new();
    for entry in entries.iter_mut().rev() {
        let mut target_clear = !entry.target.exists();
        if entry.installed {
            match fs::remove_file(&entry.target) {
                Ok(()) => target_clear = true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => target_clear = true,
                Err(error) => errors.push(format!(
                    "无法移除新文件 {}: {error}",
                    entry.target.display()
                )),
            }
        }

        if let Some(backup) = &entry.backup {
            if target_clear {
                if let Err(error) = fs::rename(backup, &entry.target) {
                    errors.push(format!(
                        "无法从 {} 恢复旧文件 {}: {error}",
                        backup.display(),
                        entry.target.display()
                    ));
                }
            } else {
                errors.push(format!(
                    "旧文件仍保存在 {}，因为目标 {} 无法清理",
                    backup.display(),
                    entry.target.display()
                ));
            }
        }

        if let Err(error) = fs::remove_file(&entry.temp) {
            if error.kind() != io::ErrorKind::NotFound {
                errors.push(format!(
                    "无法删除临时文件 {}: {error}",
                    entry.temp.display()
                ));
            }
        }
    }
    errors
}

fn commit_replacements(entries: &mut [StagedReplacement], force: bool) -> io::Result<()> {
    for index in 0..entries.len() {
        if force && entries[index].target.exists() {
            let backup = loop {
                let candidate = sidecar_path(&entries[index].target, "backup")?;
                if !candidate.exists() {
                    break candidate;
                }
            };
            if let Err(error) = fs::rename(&entries[index].target, &backup) {
                let rollback_errors = rollback_replacements(entries);
                let suffix = if rollback_errors.is_empty() {
                    String::new()
                } else {
                    format!("；回滚时又发生错误: {}", rollback_errors.join("；"))
                };
                return Err(io::Error::new(
                    error.kind(),
                    format!(
                        "无法暂存旧文件 {}: {error}{suffix}",
                        entries[index].target.display()
                    ),
                ));
            }
            entries[index].backup = Some(backup);
        }
    }

    for index in 0..entries.len() {
        // A hard link installs the complete temporary file without overwriting a target
        // created after preflight. Both names are on the same filesystem.
        let result = if force {
            fs::rename(&entries[index].temp, &entries[index].target)
        } else {
            fs::hard_link(&entries[index].temp, &entries[index].target)
        };
        if let Err(error) = result {
            let rollback_errors = rollback_replacements(entries);
            let suffix = if rollback_errors.is_empty() {
                String::new()
            } else {
                format!("；回滚时又发生错误: {}", rollback_errors.join("；"))
            };
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "无法提交新文件 {}: {error}{suffix}",
                    entries[index].target.display()
                ),
            ));
        }
        entries[index].installed = true;
    }

    for entry in entries {
        if !force {
            if let Err(error) = fs::remove_file(&entry.temp) {
                log::warn!(
                    "新文件已写入，但无法删除临时文件 {}: {error}",
                    entry.temp.display()
                );
            }
        }
        if let Some(backup) = &entry.backup {
            if let Err(error) = fs::remove_file(backup) {
                log::warn!("新文件已写入，但无法删除备份 {}: {error}", backup.display());
            }
        }
    }
    Ok(())
}

fn write_pair_atomically(
    first_path: &Path,
    first_bytes: &[u8],
    second_path: &Path,
    second_bytes: &[u8],
    force: bool,
) -> io::Result<()> {
    validate_replacement_target(first_path)?;
    validate_replacement_target(second_path)?;

    let first_temp = write_sibling_temp(first_path, first_bytes)?;
    let second_temp = match write_sibling_temp(second_path, second_bytes) {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_file(&first_temp);
            return Err(error);
        }
    };
    let mut entries = [
        StagedReplacement {
            target: first_path.to_owned(),
            temp: first_temp,
            backup: None,
            installed: false,
        },
        StagedReplacement {
            target: second_path.to_owned(),
            temp: second_temp,
            backup: None,
            installed: false,
        },
    ];
    commit_replacements(&mut entries, force)
}

pub fn write_file_atomic(out_path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_bytes_atomically(out_path, bytes)
}

/// Save one source or a VS/PS pair after the caller validates stages and source text.
pub fn write_source_files_atomic(sources: &[(&Path, &[u8])], force: bool) -> io::Result<()> {
    if !(1..=2).contains(&sources.len()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "一次只能保存 1 或 2 个着色器源码文件",
        ));
    }

    let mut resolved_paths = Vec::with_capacity(2);
    for (path, _) in sources {
        let file_name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "输出路径没有文件名"))?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let resolved_path = fs::canonicalize(parent)?.join(file_name);
        let duplicate = resolved_paths.iter().any(|previous: &PathBuf| {
            if cfg!(windows) {
                previous.to_string_lossy().to_lowercase()
                    == resolved_path.to_string_lossy().to_lowercase()
            } else {
                previous == &resolved_path
            }
        });
        if duplicate {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "着色器输出路径不能重复",
            ));
        }
        resolved_paths.push(resolved_path);

        match fs::symlink_metadata(path) {
            Ok(_) if !force => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("输出文件已存在: {}", path.display()),
                ));
            }
            Ok(_) => validate_replacement_target(path)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    match sources {
        [(path, bytes)] if force => write_file_atomic(path, bytes),
        [(path, bytes)] => {
            let temp = write_sibling_temp(path, bytes)?;
            commit_replacements(
                &mut [StagedReplacement {
                    target: path.to_path_buf(),
                    temp,
                    backup: None,
                    installed: false,
                }],
                false,
            )
        }
        [(first_path, first_bytes), (second_path, second_bytes)] => {
            write_pair_atomically(first_path, first_bytes, second_path, second_bytes, force)
        }
        _ => unreachable!("source count was checked above"),
    }
}

/// Safely replace a KSH file with bytes that have already been built.
pub fn write_ksh_file(out_path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    write_bytes_atomically(out_path, bytes)?;
    Ok(())
}

/// 分析 KSH 文件并输出着色器文件
pub fn analyze_ksh_file(
    file_path: &Path,
    out_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("分析文件: {:?}", file_path);
    let content = fs::read(file_path)?;
    let ksh = parse_ksh(&content)?;
    let vs_name = safe_stage_file_name(&ksh.vertex.source_name, "vs")?;
    let ps_name = safe_stage_file_name(&ksh.pixel.source_name, "ps")?;
    if vs_name
        .to_string_lossy()
        .eq_ignore_ascii_case(&ps_name.to_string_lossy())
    {
        return Err("VS 与 PS 的输出文件名不能相同".into());
    }

    let vs_file_path = out_path.join(vs_name);
    if !force && vs_file_path.exists() {
        return Err(format!("输出文件已存在: {}", vs_file_path.display()).into());
    }
    let ps_file_path = out_path.join(ps_name);
    if !force && ps_file_path.exists() {
        return Err(format!("输出文件已存在: {}", ps_file_path.display()).into());
    }

    write_pair_atomically(
        &vs_file_path,
        ksh.vertex.source.as_bytes(),
        &ps_file_path,
        ksh.pixel.source.as_bytes(),
        force,
    )?;

    log::info!("分析完成");
    Ok(())
}

fn validate_declarations(stage: &str, declarations: &[UniformDeclaration]) -> Result<(), KshError> {
    let mut sampler_count = 0u32;
    for (index, declaration) in declarations.iter().enumerate() {
        if declaration.name.is_empty() || declaration.name.as_bytes().contains(&0) {
            return Err(KshError::message(format!(
                "{stage} uniform[{index}] 的名称为空或包含 NUL"
            )));
        }
        if declaration.array_count == 0 {
            return Err(KshError::message(format!(
                "{stage} uniform {} 的数组长度必须大于 0",
                declaration.name
            )));
        }
        if declaration.uniform_type.is_sampler() && declaration.array_count > MAX_SAMPLER_COUNT {
            return Err(KshError::message(format!(
                "{stage} sampler {} 需要 {} 个纹理槽，DST 上限为 {}",
                declaration.name, declaration.array_count, MAX_SAMPLER_COUNT
            )));
        }
        if declaration.uniform_type.is_sampler() {
            sampler_count = sampler_count
                .checked_add(declaration.array_count)
                .ok_or_else(|| KshError::message(format!("{stage} sampler 纹理槽数量溢出")))?;
        }
        let expected_defaults = if declaration.uniform_type.is_sampler() || declaration.is_array {
            0
        } else {
            declaration
                .uniform_type
                .default_data_length()
                .ok_or_else(|| {
                    KshError::message(format!(
                        "{stage} uniform {} 的类型 {} 没有已确认的默认值宽度",
                        declaration.name,
                        declaration.uniform_type.name()
                    ))
                })?
        };
        if let Some(default_data) = &declaration.default_data {
            if default_data.len() != expected_defaults {
                return Err(KshError::message(format!(
                    "{stage} uniform {} 的默认值有 {} 项，预期 {expected_defaults} 项",
                    declaration.name,
                    default_data.len()
                )));
            }
        }
        if declarations[..index]
            .iter()
            .any(|other| other.name == declaration.name)
        {
            return Err(KshError::message(format!(
                "{stage} 重复声明 uniform {}",
                declaration.name
            )));
        }
    }
    if sampler_count > MAX_SAMPLER_COUNT {
        return Err(KshError::message(format!(
            "{stage} 共需要 {sampler_count} 个 sampler 纹理槽，DST 上限为 {MAX_SAMPLER_COUNT}"
        )));
    }
    Ok(())
}

fn compiled_uniform(declaration: &UniformDeclaration) -> Result<KshUniform, KshError> {
    let default_data = if declaration.uniform_type.is_sampler() || declaration.is_array {
        Vec::new()
    } else if let Some(default_data) = &declaration.default_data {
        default_data.clone()
    } else {
        let width = declaration
            .uniform_type
            .default_data_length()
            .ok_or_else(|| {
                KshError::message(format!(
                    "uniform {} 的类型 {} 没有已确认的默认值宽度",
                    declaration.name,
                    declaration.uniform_type.name()
                ))
            })?;
        vec![0; width]
    };
    Ok(KshUniform {
        name: declaration.name.clone(),
        scope: VariableScope::Uniform,
        uniform_type: declaration.uniform_type,
        array_count: declaration.array_count,
        default_data,
    })
}

fn encoded_signature_matches(uniform: &KshUniform, declaration: &UniformDeclaration) -> bool {
    uniform.uniform_type == declaration.uniform_type
        && uniform.array_count == declaration.array_count
}

fn declaration_signatures_match(left: &UniformDeclaration, right: &UniformDeclaration) -> bool {
    left.uniform_type == right.uniform_type
        && left.array_count == right.array_count
        && left.is_array == right.is_array
}

fn stage_indices(
    stage: &str,
    declarations: &[UniformDeclaration],
    uniforms: &[KshUniform],
) -> Result<Vec<u32>, KshError> {
    declarations
        .iter()
        .map(|declaration| {
            let index = uniforms
                .iter()
                .position(|uniform| uniform.name == declaration.name)
                .ok_or_else(|| {
                    KshError::message(format!(
                        "{stage} uniform {} 未出现在全局表中",
                        declaration.name
                    ))
                })?;
            checked_u32(index, &format!("{stage} uniform 索引"))
        })
        .collect()
}

pub fn build_ksh(
    file_name: &str,
    vs_name: &str,
    vs_content: &str,
    ps_name: &str,
    ps_content: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    safe_stage_file_name(vs_name, "vs")?;
    safe_stage_file_name(ps_name, "ps")?;
    let vs_declarations = parse_glsl_uniforms(vs_content)?;
    let ps_declarations = parse_glsl_uniforms(ps_content)?;
    validate_declarations("VS", &vs_declarations)?;
    validate_declarations("PS", &ps_declarations)?;

    let mut uniforms = Vec::with_capacity(vs_declarations.len() + ps_declarations.len());
    for declaration in &vs_declarations {
        uniforms.push(compiled_uniform(declaration)?);
    }
    for declaration in &ps_declarations {
        if let Some(existing) = vs_declarations
            .iter()
            .find(|existing| existing.name == declaration.name)
        {
            if !declaration_signatures_match(existing, declaration) {
                return Err(KshError::message(format!(
                    "VS/PS 同名 uniform {} 的类型、数组形式或长度不一致",
                    declaration.name
                ))
                .into());
            }
            // Matching declarations share one global record. The official compiler processes VS
            // first, so its default block wins even when PS spells a different default.
        } else {
            uniforms.push(compiled_uniform(declaration)?);
        }
    }

    let ksh = KshFile {
        effect_name: file_name.to_owned(),
        vertex: KshStage {
            source_name: vs_name.to_owned(),
            source: vs_content.to_owned(),
            uniform_indices: stage_indices("VS", &vs_declarations, &uniforms)?,
        },
        pixel: KshStage {
            source_name: ps_name.to_owned(),
            source: ps_content.to_owned(),
            uniform_indices: stage_indices("PS", &ps_declarations, &uniforms)?,
        },
        uniforms,
    };
    Ok(encode_ksh(&ksh)?)
}

fn stage_declarations_match(
    ksh: &KshFile,
    stage: &KshStage,
    declarations: &[UniformDeclaration],
    previous_declarations: Option<&[UniformDeclaration]>,
) -> bool {
    stage.uniform_indices.len() == declarations.len()
        && stage
            .uniform_indices
            .iter()
            .zip(declarations)
            .enumerate()
            .all(|(position, (index, declaration))| {
                usize::try_from(*index)
                    .ok()
                    .and_then(|index| ksh.uniforms.get(index))
                    .is_some_and(|uniform| {
                        if uniform.name != declaration.name
                            || !encoded_signature_matches(uniform, declaration)
                        {
                            return false;
                        }

                        // array_count=1 cannot distinguish `float X` from `float X[1]` in
                        // the binary record. Consult the old source or rebuild conservatively.
                        uniform.array_count != 1
                            || previous_declarations
                                .and_then(|previous| previous.get(position))
                                .is_some_and(|previous| {
                                    previous.name == uniform.name
                                        && encoded_signature_matches(uniform, previous)
                                        && declaration_signatures_match(previous, declaration)
                                })
                    })
            })
}

fn first_declaration<'a>(
    vertex: &'a [UniformDeclaration],
    pixel: &'a [UniformDeclaration],
    name: &str,
) -> Option<&'a UniformDeclaration> {
    vertex
        .iter()
        .chain(pixel)
        .find(|declaration| declaration.name == name)
}

fn previous_default_was_explicit(
    previous: Option<&(Vec<UniformDeclaration>, Vec<UniformDeclaration>)>,
    name: &str,
) -> bool {
    previous
        .and_then(|(vertex, pixel)| first_declaration(vertex, pixel, name))
        .is_some_and(|declaration| declaration.default_data.is_some())
}

fn metadata_signature_matches(
    uniform: &KshUniform,
    declaration: &UniformDeclaration,
    previous: Option<&(Vec<UniformDeclaration>, Vec<UniformDeclaration>)>,
) -> bool {
    if !encoded_signature_matches(uniform, declaration) {
        return false;
    }
    if uniform.array_count != 1 {
        return declaration.is_array;
    }

    previous
        .and_then(|(vertex, pixel)| first_declaration(vertex, pixel, &uniform.name))
        .is_some_and(|old_declaration| {
            encoded_signature_matches(uniform, old_declaration)
                && declaration_signatures_match(old_declaration, declaration)
        })
}

fn apply_source_defaults(
    ksh: &mut KshFile,
    indices: &[u32],
    declarations: &[UniformDeclaration],
    seen: &mut HashSet<String>,
    previous: Option<&(Vec<UniformDeclaration>, Vec<UniformDeclaration>)>,
) -> Result<(), KshError> {
    for (index, declaration) in indices.iter().zip(declarations) {
        if !seen.insert(declaration.name.clone()) {
            continue;
        }
        let default_data = if let Some(default_data) = &declaration.default_data {
            Some(default_data.clone())
        } else if previous_default_was_explicit(previous, &declaration.name) {
            Some(compiled_uniform(declaration)?.default_data)
        } else {
            None
        };
        let Some(default_data) = default_data else {
            continue;
        };
        let index = usize::try_from(*index)
            .map_err(|_| KshError::message("uniform 索引无法转换为内存索引"))?;
        let uniform = ksh
            .uniforms
            .get_mut(index)
            .ok_or_else(|| KshError::message(format!("uniform 索引 {index} 越界")))?;
        uniform.default_data = default_data;
    }
    Ok(())
}

/// Rebuild edited sources while retaining metadata from an imported KSH wherever signatures match.
pub fn build_ksh_preserving_metadata(
    base: &KshFile,
    vs_name: &str,
    vs_content: &str,
    ps_name: &str,
    ps_content: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    safe_stage_file_name(vs_name, "vs")?;
    safe_stage_file_name(ps_name, "ps")?;

    // This path also supports containers whose source language or type codes this GLSL front end
    // cannot reconstruct. Renaming stages alone must not destroy otherwise valid KSH metadata.
    if base.vertex.source == vs_content && base.pixel.source == ps_content {
        let mut preserved = base.clone();
        preserved.vertex.source_name = vs_name.to_owned();
        preserved.pixel.source_name = ps_name.to_owned();
        return Ok(encode_ksh(&preserved)?);
    }

    let vs_declarations = parse_glsl_uniforms(vs_content)?;
    let ps_declarations = parse_glsl_uniforms(ps_content)?;
    validate_declarations("VS", &vs_declarations)?;
    validate_declarations("PS", &ps_declarations)?;
    let previous_declarations = match (
        parse_glsl_uniforms(&base.vertex.source),
        parse_glsl_uniforms(&base.pixel.source),
    ) {
        (Ok(vertex), Ok(pixel)) => Some((vertex, pixel)),
        _ => None,
    };

    if stage_declarations_match(
        base,
        &base.vertex,
        &vs_declarations,
        previous_declarations
            .as_ref()
            .map(|(vertex, _)| vertex.as_slice()),
    ) && stage_declarations_match(
        base,
        &base.pixel,
        &ps_declarations,
        previous_declarations
            .as_ref()
            .map(|(_, pixel)| pixel.as_slice()),
    ) {
        let mut preserved = base.clone();
        preserved.vertex.source_name = vs_name.to_owned();
        preserved.vertex.source = vs_content.to_owned();
        preserved.pixel.source_name = ps_name.to_owned();
        preserved.pixel.source = ps_content.to_owned();

        let vertex_indices = preserved.vertex.uniform_indices.clone();
        let pixel_indices = preserved.pixel.uniform_indices.clone();
        let mut seen = HashSet::new();
        apply_source_defaults(
            &mut preserved,
            &vertex_indices,
            &vs_declarations,
            &mut seen,
            previous_declarations.as_ref(),
        )?;
        apply_source_defaults(
            &mut preserved,
            &pixel_indices,
            &ps_declarations,
            &mut seen,
            previous_declarations.as_ref(),
        )?;
        return Ok(encode_ksh(&preserved)?);
    }

    let rebuilt_bytes = build_ksh(&base.effect_name, vs_name, vs_content, ps_name, ps_content)?;
    let mut rebuilt = parse_ksh(&rebuilt_bytes)?;
    for uniform in &mut rebuilt.uniforms {
        let declaration = first_declaration(&vs_declarations, &ps_declarations, &uniform.name);
        let Some(declaration) = declaration else {
            continue;
        };
        let Some(original) = base.uniforms.iter().find(|original| {
            original.name == uniform.name
                && metadata_signature_matches(original, declaration, previous_declarations.as_ref())
        }) else {
            continue;
        };

        uniform.scope = original.scope;
        if declaration.default_data.is_none()
            && !previous_default_was_explicit(previous_declarations.as_ref(), &uniform.name)
        {
            uniform.default_data.clone_from(&original.default_data);
        }
    }
    Ok(encode_ksh(&rebuilt)?)
}

pub fn get_ps_vs_from_dir(path: &Path) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let mut vs_paths = Vec::new();
    let mut ps_paths = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path.extension().and_then(OsStr::to_str).unwrap_or_default();
        if extension.eq_ignore_ascii_case("vs") {
            vs_paths.push(path);
        } else if extension.eq_ignore_ascii_case("ps") {
            ps_paths.push(path);
        }
    }
    vs_paths.sort();
    ps_paths.sort();
    if vs_paths.len() != 1 || ps_paths.len() != 1 {
        return Err(format!(
            "目录必须恰好包含一个 .vs 和一个 .ps 文件，实际为 {} 个 .vs、{} 个 .ps",
            vs_paths.len(),
            ps_paths.len()
        )
        .into());
    }
    Ok((vs_paths.remove(0), ps_paths.remove(0)))
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
    first_file: &Path,
    second_file: &Path,
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let extension = |path: &Path| {
        path.extension()
            .and_then(OsStr::to_str)
            .map(|value| value.to_ascii_lowercase())
    };
    let (vs_file, ps_file) = match (
        extension(first_file).as_deref(),
        extension(second_file).as_deref(),
    ) {
        (Some("vs"), Some("ps")) => (first_file, second_file),
        (Some("ps"), Some("vs")) => (second_file, first_file),
        _ => return Err("需要恰好一个 .vs 和一个 .ps 文件（顺序任意）".into()),
    };

    let vs_content = fs::read_to_string(vs_file)
        .map_err(|error| format!("读取顶点着色器 {} 失败: {error}", vs_file.display()))?;
    let ps_content = fs::read_to_string(ps_file)
        .map_err(|error| format!("读取像素着色器 {} 失败: {error}", ps_file.display()))?;

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

    let file_name = out_path
        .file_stem()
        .ok_or_else(|| format!("无效的输出路径: {}", out_path.display()))?
        .to_str()
        .ok_or_else(|| format!("输出路径包含非法UTF-8字符: {}", out_path.display()))?;
    let buffer = build_ksh(file_name, vs_name, &vs_content, ps_name, &ps_content)?;
    write_ksh_file(out_path, &buffer)?;

    Ok(())
}

#[cfg(test)]
mod codec_tests {
    use super::*;

    fn official_ksh_paths() -> Option<Vec<PathBuf>> {
        let root = Path::new(
            "C:\\Saved Games\\Steam\\steamapps\\common\\Don't Starve Together\\data\\databundles\\shaders",
        );
        if !root.is_dir() {
            eprintln!("跳过官方 KSH 回归：本机未找到 shader 目录");
            return None;
        }
        let mut paths: Vec<_> = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .and_then(OsStr::to_str)
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ksh"))
            })
            .collect();
        paths.sort();
        assert_eq!(paths.len(), 65, "官方 KSH 样本集数量发生变化");
        Some(paths)
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "dst-ksh-analyze-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn sample_ksh() -> KshFile {
        KshFile {
            effect_name: "测试_effect".to_string(),
            uniforms: vec![
                KshUniform {
                    name: "VALUE".to_string(),
                    scope: VariableScope::Uniform,
                    uniform_type: KshUniformType::Float,
                    array_count: 1,
                    default_data: vec![0x8000_0000],
                },
                KshUniform {
                    name: "FUTURE".to_string(),
                    scope: VariableScope::Unknown(7),
                    uniform_type: KshUniformType::Unknown(99),
                    array_count: 3,
                    default_data: vec![0x7fc0_1234, 1, u32::MAX],
                },
                KshUniform {
                    name: "TEXTURE".to_string(),
                    scope: VariableScope::Uniform,
                    uniform_type: KshUniformType::SamplerCube,
                    array_count: 2,
                    default_data: vec![],
                },
            ],
            vertex: KshStage {
                source_name: "sample.vs".to_string(),
                source: "// 中文注释\nvoid main() {}\n".to_string(),
                uniform_indices: vec![0, 1],
            },
            pixel: KshStage {
                source_name: "sample.ps".to_string(),
                source: "void main() {}\n".to_string(),
                uniform_indices: vec![2],
            },
        }
    }

    #[test]
    fn binary_codec_preserves_every_field_and_raw_default_bits() {
        let expected = sample_ksh();
        let bytes = encode_ksh(&expected).unwrap();
        let actual = parse_ksh(&bytes).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(encode_ksh(&actual).unwrap(), bytes);
    }

    #[test]
    fn all_sampler_codes_omit_the_default_block() {
        for uniform_type in [
            KshUniformType::Sampler1D,
            KshUniformType::Sampler2D,
            KshUniformType::Sampler3D,
            KshUniformType::SamplerCube,
        ] {
            let mut sample = sample_ksh();
            sample.uniforms = vec![KshUniform {
                name: "SAMPLER".to_string(),
                scope: VariableScope::Uniform,
                uniform_type,
                array_count: 1,
                default_data: vec![],
            }];
            sample.vertex.uniform_indices.clear();
            sample.pixel.uniform_indices = vec![0];
            let bytes = encode_ksh(&sample).unwrap();
            assert_eq!(parse_ksh(&bytes).unwrap(), sample);
        }
    }

    #[test]
    fn every_truncated_prefix_returns_an_error_without_panicking() {
        let bytes = encode_ksh(&sample_ksh()).unwrap();
        for length in 0..bytes.len() {
            let result = std::panic::catch_unwind(|| parse_ksh(&bytes[..length]));
            assert!(result.is_ok(), "parser panicked at prefix length {length}");
            assert!(
                result.unwrap().is_err(),
                "prefix length {length} was accepted"
            );
        }
    }

    #[test]
    fn trailing_bytes_and_out_of_range_indices_are_rejected() {
        let mut trailing = encode_ksh(&sample_ksh()).unwrap();
        trailing.push(0xaa);
        assert!(parse_ksh(&trailing).is_err());

        let mut bad_index = encode_ksh(&sample_ksh()).unwrap();
        let end = bad_index.len();
        bad_index[end - 4..].copy_from_slice(&99u32.to_le_bytes());
        assert!(parse_ksh(&bad_index).is_err());
    }

    #[test]
    fn invalid_utf8_and_missing_source_nul_are_rejected() {
        let mut invalid_utf8 = encode_ksh(&sample_ksh()).unwrap();
        // First string starts at byte four and contains a multibyte character.
        invalid_utf8[4] = 0xff;
        assert!(parse_ksh(&invalid_utf8).is_err());

        let mut missing_nul = encode_ksh(&sample_ksh()).unwrap();
        let needle = b"void main() {}\n\0";
        let terminator = missing_nul
            .windows(needle.len())
            .position(|window| window == needle)
            .map(|start| start + needle.len() - 1)
            .unwrap();
        missing_nul[terminator] = b'!';
        assert!(parse_ksh(&missing_nul).is_err());
    }

    #[test]
    fn impossible_counts_are_rejected_before_allocation() {
        assert!(parse_ksh(&u32::MAX.to_le_bytes()).is_err());

        let mut bytes = Vec::new();
        write_text(&mut bytes, "effect", "effect").unwrap();
        write_u32(&mut bytes, u32::MAX);
        assert!(parse_ksh(&bytes).is_err());
    }

    #[test]
    fn encoder_rejects_bad_model_values() {
        let mut zero_array = sample_ksh();
        zero_array.uniforms[0].array_count = 0;
        assert!(encode_ksh(&zero_array).is_err());

        let mut sampler_defaults = sample_ksh();
        sampler_defaults.uniforms[2].default_data.push(0);
        assert!(encode_ksh(&sampler_defaults).is_err());

        let mut bad_index = sample_ksh();
        bad_index.vertex.uniform_indices.push(50);
        assert!(encode_ksh(&bad_index).is_err());
    }

    #[test]
    fn unsafe_extracted_file_names_are_rejected() {
        for name in [
            "../escape.vs",
            "..\\escape.vs",
            "C:\\escape.vs",
            "/escape.vs",
            "bad?.vs",
            "bad*.vs",
            "bad|name.vs",
            "bad<name>.vs",
            "NUL.vs",
            "con.VS",
            "COM1.vs",
            "lpt9.vs",
            "COM¹.vs",
            "lpt³.vs",
            "trailing.vs ",
            "trailing.vs.",
        ] {
            assert!(safe_stage_file_name(name, "vs").is_err(), "accepted {name}");
        }
        assert!(safe_stage_file_name("shader.VS", "vs").is_ok());
        assert!(safe_stage_file_name("着色器.vs", "vs").is_ok());
        assert!(safe_stage_file_name("shader.ps", "vs").is_err());
    }

    #[test]
    fn invalid_pixel_name_is_rejected_before_vertex_is_written() {
        let directory = unique_test_dir("invalid-stage-name");
        let output_directory = directory.join("output");
        let input_path = directory.join("input.ksh");
        fs::create_dir(&output_directory).unwrap();

        let mut sample = sample_ksh();
        sample.vertex.source_name = "valid.vs".to_string();
        sample.pixel.source_name = "NUL.ps".to_string();
        fs::write(&input_path, encode_ksh(&sample).unwrap()).unwrap();

        assert!(analyze_ksh_file(&input_path, &output_directory, false).is_err());
        assert!(!output_directory.join("valid.vs").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_compiler_emits_matrix_and_sampler_codes() {
        let vertex = r#"
            uniform mat2 MATRIX2;
            uniform mat3 MATRIX3;
            uniform mat4 MATRIX4;
            void main() {
                mat2 used2 = MATRIX2;
                mat3 used3 = MATRIX3;
                gl_Position = MATRIX4 * vec4(used2[0], used3[0].x, 1.0);
            }
        "#;
        let pixel = r#"
            uniform samplerCube CUBE_TEXTURE;
            void main() {
                gl_FragColor = textureCube(CUBE_TEXTURE, vec3(1.0));
            }
        "#;

        let parsed = parse_ksh(
            &build_ksh("type_test", "type_test.vs", vertex, "type_test.ps", pixel).unwrap(),
        )
        .unwrap();
        let codes: Vec<_> = parsed
            .uniforms
            .iter()
            .map(|uniform| uniform.uniform_type.id())
            .collect();
        assert_eq!(codes, [10, 15, 20, 45]);
    }

    #[test]
    fn source_compiler_serializes_explicit_defaults_as_float_bits() {
        let vertex = r#"
            uniform int COUNT = 7;
            uniform vec4 TINT = vec4(0.25, -0.0, 2.5, 4.0);
            uniform mat3x2 SHAPE = mat3x2(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
            void main() {
                COUNT; TINT; SHAPE;
                gl_Position = vec4(0.0);
            }
        "#;
        let parsed = parse_ksh(
            &build_ksh(
                "defaults",
                "defaults.vs",
                vertex,
                "defaults.ps",
                "void main() { gl_FragColor = vec4(1.0); }",
            )
            .unwrap(),
        )
        .unwrap();

        let bits = |values: &[f32]| {
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(parsed.uniforms[0].default_data, bits(&[7.0]));
        assert_eq!(
            parsed.uniforms[1].default_data,
            bits(&[0.25, -0.0, 2.5, 4.0])
        );
        assert_eq!(
            parsed.uniforms[2].default_data,
            bits(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
        );
    }

    #[test]
    fn source_compiler_uses_vertex_default_for_shared_uniform() {
        let vertex = r#"
            uniform vec4 SHARED = vec4(1.0, 2.0, 3.0, 4.0);
            void main() { gl_Position = SHARED; }
        "#;
        let pixel = r#"
            uniform vec4 SHARED = vec4(5.0, 6.0, 7.0, 8.0);
            void main() { gl_FragColor = SHARED; }
        "#;
        let parsed =
            parse_ksh(&build_ksh("shared", "shared.vs", vertex, "shared.ps", pixel).unwrap())
                .unwrap();

        assert_eq!(parsed.uniforms.len(), 1);
        assert_eq!(
            parsed.uniforms[0].default_data,
            [1.0f32, 2.0, 3.0, 4.0]
                .into_iter()
                .map(f32::to_bits)
                .collect::<Vec<_>>()
        );
        assert_eq!(parsed.vertex.uniform_indices, [0]);
        assert_eq!(parsed.pixel.uniform_indices, [0]);
    }

    #[test]
    fn metadata_preserving_build_is_byte_exact_when_sources_are_unchanged() {
        let original = sample_ksh();
        let original_bytes = encode_ksh(&original).unwrap();
        let rebuilt = build_ksh_preserving_metadata(
            &original,
            &original.vertex.source_name,
            &original.vertex.source,
            &original.pixel.source_name,
            &original.pixel.source,
        )
        .unwrap();

        assert_eq!(rebuilt, original_bytes);
    }

    #[test]
    fn metadata_preserving_build_keeps_compatible_records_after_source_edits() {
        let vertex = "uniform vec4 VALUE; void main() { gl_Position = VALUE; }";
        let pixel = "void main() { gl_FragColor = vec4(1.0); }";
        let mut original = parse_ksh(
            &build_ksh(
                "original_effect",
                "original.vs",
                vertex,
                "original.ps",
                pixel,
            )
            .unwrap(),
        )
        .unwrap();
        original.uniforms[0].scope = VariableScope::Unknown(7);
        original.uniforms[0].default_data = vec![
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            3.0f32.to_bits(),
            4.0f32.to_bits(),
        ];
        original.uniforms.push(KshUniform {
            name: "OPAQUE_METADATA".to_string(),
            scope: VariableScope::Unknown(99),
            uniform_type: KshUniformType::Unknown(1234),
            array_count: 1,
            default_data: vec![0x7fc0_1234],
        });

        let edited_vertex =
            "// body edit\nuniform vec4 VALUE; void main() { gl_Position = VALUE * 1.0; }";
        let rebuilt = parse_ksh(
            &build_ksh_preserving_metadata(
                &original,
                "renamed.vs",
                edited_vertex,
                "renamed.ps",
                pixel,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(rebuilt.effect_name, "original_effect");
        assert_eq!(rebuilt.uniforms, original.uniforms);
        assert_eq!(
            rebuilt.vertex.uniform_indices,
            original.vertex.uniform_indices
        );
        assert_eq!(
            rebuilt.pixel.uniform_indices,
            original.pixel.uniform_indices
        );
        assert_eq!(rebuilt.vertex.source_name, "renamed.vs");
        assert_eq!(rebuilt.vertex.source, edited_vertex);
    }

    #[test]
    fn metadata_preserving_build_reconciles_changed_uniform_signatures() {
        let vertex = "uniform vec4 VALUE; void main() { gl_Position = VALUE; }";
        let pixel = "void main() { gl_FragColor = vec4(1.0); }";
        let mut original =
            parse_ksh(&build_ksh("kept_effect", "base.vs", vertex, "base.ps", pixel).unwrap())
                .unwrap();
        original.uniforms[0].scope = VariableScope::Unknown(5);
        original.uniforms[0].default_data = vec![0x3f00_0000; 4];
        original.uniforms.push(KshUniform {
            name: "OLD_ONLY".to_string(),
            scope: VariableScope::Unknown(6),
            uniform_type: KshUniformType::Unknown(600),
            array_count: 1,
            default_data: vec![9],
        });

        let edited_vertex = r#"
            uniform vec4 VALUE;
            uniform float NEW_VALUE = 3.5;
            void main() { gl_Position = VALUE + vec4(NEW_VALUE); }
        "#;
        let rebuilt = parse_ksh(
            &build_ksh_preserving_metadata(
                &original,
                "edited.vs",
                edited_vertex,
                "edited.ps",
                pixel,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(rebuilt.effect_name, "kept_effect");
        assert_eq!(rebuilt.uniforms.len(), 2);
        assert_eq!(rebuilt.uniforms[0].name, "VALUE");
        assert_eq!(rebuilt.uniforms[0].scope, VariableScope::Unknown(5));
        assert_eq!(rebuilt.uniforms[0].default_data, vec![0x3f00_0000; 4]);
        assert_eq!(rebuilt.uniforms[1].name, "NEW_VALUE");
        assert_eq!(rebuilt.uniforms[1].default_data, vec![3.5f32.to_bits()]);
        assert_eq!(rebuilt.vertex.uniform_indices, [0, 1]);
    }

    #[test]
    fn metadata_preserving_build_applies_new_explicit_defaults() {
        let vertex = "uniform vec3 VALUE; void main() { gl_Position = vec4(VALUE, 1.0); }";
        let pixel = "void main() { gl_FragColor = vec4(1.0); }";
        let mut original =
            parse_ksh(&build_ksh("effect", "base.vs", vertex, "base.ps", pixel).unwrap()).unwrap();
        original.uniforms[0].default_data = vec![9.0f32.to_bits(); 3];

        let edited_vertex =
            "uniform vec3 VALUE = vec3(2.0); void main() { gl_Position = vec4(VALUE, 1.0); }";
        let rebuilt = parse_ksh(
            &build_ksh_preserving_metadata(&original, "base.vs", edited_vertex, "base.ps", pixel)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(rebuilt.uniforms[0].default_data, vec![2.0f32.to_bits(); 3]);
    }

    #[test]
    fn metadata_preserving_build_clears_a_removed_initializer() {
        let vertex = "uniform float VALUE = 5.0; void main() { gl_Position = vec4(VALUE); }";
        let pixel = "void main() { gl_FragColor = vec4(1.0); }";
        let original =
            parse_ksh(&build_ksh("effect", "base.vs", vertex, "base.ps", pixel).unwrap()).unwrap();

        for edited_vertex in [
            "uniform float VALUE; void main() { gl_Position = vec4(VALUE); }",
            "uniform float VALUE; uniform float EXTRA; void main() { gl_Position = vec4(VALUE + EXTRA); }",
        ] {
            let rebuilt = parse_ksh(
                &build_ksh_preserving_metadata(
                    &original,
                    "base.vs",
                    edited_vertex,
                    "base.ps",
                    pixel,
                )
                .unwrap(),
            )
            .unwrap();
            let value = rebuilt
                .uniforms
                .iter()
                .find(|uniform| uniform.name == "VALUE")
                .unwrap();
            assert_eq!(value.default_data, vec![0]);
        }
    }

    #[test]
    fn metadata_preserving_build_distinguishes_scalar_and_single_element_array() {
        let pixel = "void main() { gl_FragColor = vec4(1.0); }";
        let cases = [
            (
                "uniform float VALUE = 4.0; void main() { gl_Position = vec4(VALUE); }",
                "uniform float VALUE[1]; void main() { gl_Position = vec4(VALUE[0]); }",
                Vec::new(),
            ),
            (
                "uniform float VALUE[1]; void main() { gl_Position = vec4(VALUE[0]); }",
                "uniform float VALUE; void main() { gl_Position = vec4(VALUE); }",
                vec![0],
            ),
        ];

        for (original_vertex, edited_vertex, expected_defaults) in cases {
            let mut original = parse_ksh(
                &build_ksh("effect", "base.vs", original_vertex, "base.ps", pixel).unwrap(),
            )
            .unwrap();
            original.uniforms[0].scope = VariableScope::Unknown(7);

            let rebuilt = parse_ksh(
                &build_ksh_preserving_metadata(
                    &original,
                    "base.vs",
                    edited_vertex,
                    "base.ps",
                    pixel,
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(rebuilt.uniforms[0].array_count, 1);
            assert_eq!(rebuilt.uniforms[0].default_data, expected_defaults);
            assert_eq!(rebuilt.uniforms[0].scope, VariableScope::Uniform);
        }
    }

    #[test]
    fn source_compiler_rejects_cross_stage_signature_conflicts() {
        let vertex = "uniform vec3 SHARED; void main() { gl_Position = vec4(SHARED, 1.0); }";
        let pixel = "uniform vec4 SHARED; void main() { gl_FragColor = SHARED; }";
        let error = build_ksh("conflict", "conflict.vs", vertex, "conflict.ps", pixel)
            .unwrap_err()
            .to_string();
        assert!(error.contains("类型、数组形式或长度不一致"), "{error}");

        let scalar = "uniform float SHARED; void main() { gl_Position = vec4(SHARED); }";
        let single_element_array =
            "uniform float SHARED[1]; void main() { gl_FragColor = vec4(SHARED[0]); }";
        let error = build_ksh(
            "array-conflict",
            "array-conflict.vs",
            scalar,
            "array-conflict.ps",
            single_element_array,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("数组形式"), "{error}");
    }

    #[test]
    fn source_compiler_limits_total_sampler_slots_per_stage() {
        let pixel = r#"
            uniform sampler2D FIRST[5], SECOND[4];
            void main() {
                gl_FragColor = texture2D(FIRST[0], vec2(0.0))
                    + texture2D(SECOND[0], vec2(0.0));
            }
        "#;
        let error = build_ksh(
            "sampler_limit",
            "sampler_limit.vs",
            "void main() { gl_Position = vec4(0.0); }",
            "sampler_limit.ps",
            pixel,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("共需要 9 个 sampler"), "{error}");
    }

    #[test]
    fn failed_source_build_does_not_replace_existing_output() {
        let directory = unique_test_dir("preserve-output");
        let vertex_path = directory.join("invalid.vs");
        let pixel_path = directory.join("valid.ps");
        let output_path = directory.join("existing.ksh");
        fs::write(
            &vertex_path,
            "uniform bool BAD; void main() { gl_Position = vec4(BAD); }",
        )
        .unwrap();
        fs::write(&pixel_path, "void main() { gl_FragColor = vec4(1.0); }").unwrap();
        fs::write(&output_path, b"keep this file").unwrap();

        assert!(build_ksh_file(&vertex_path, &pixel_path, &output_path).is_err());
        assert_eq!(fs::read(&output_path).unwrap(), b"keep this file");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn writer_rejects_a_directory_target_without_moving_it() {
        let directory = unique_test_dir("directory-output");
        let output_path = directory.join("must-stay.ksh");
        let sentinel_path = output_path.join("sentinel.txt");
        fs::create_dir(&output_path).unwrap();
        fs::write(&sentinel_path, b"still here").unwrap();

        let error = write_file_atomic(&output_path, b"replacement").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(output_path.is_dir());
        assert_eq!(fs::read(&sentinel_path).unwrap(), b"still here");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_writer_saves_single_or_pair_and_requires_force_to_replace() {
        let directory = unique_test_dir("source-batch");
        let vertex = directory.join("draft.vs");
        let pixel = directory.join("draft.ps");
        write_source_files_atomic(&[(&vertex, b"incomplete vertex (")], false).unwrap();
        let pair: &[(&Path, &[u8])] = &[(&vertex, b"new vertex"), (&pixel, b"new pixel")];

        let error = write_source_files_atomic(pair, false).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&vertex).unwrap(), b"incomplete vertex (");
        assert!(!pixel.exists());

        write_source_files_atomic(pair, true).unwrap();
        assert_eq!(fs::read(&vertex).unwrap(), b"new vertex");
        assert_eq!(fs::read(&pixel).unwrap(), b"new pixel");
        write_source_files_atomic(&[(&pixel, b"single replacement")], true).unwrap();
        assert_eq!(fs::read(&pixel).unwrap(), b"single replacement");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_writer_rejects_duplicate_paths_and_preflights_both_targets() {
        let directory = unique_test_dir("source-preflight");
        let vertex = directory.join("draft.vs");
        let alias = directory.join(".").join("draft.vs");
        let pixel = directory.join("draft.ps");
        let error = write_source_files_atomic(&[(&vertex, b"first"), (&alias, b"second")], true)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);

        fs::write(&pixel, b"old pixel").unwrap();
        let error = write_source_files_atomic(&[(&vertex, b"first"), (&pixel, b"second")], false)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(!vertex.exists());
        assert_eq!(fs::read(&pixel).unwrap(), b"old pixel");

        let missing_pixel = directory.join("missing").join("draft.ps");
        assert!(
            write_pair_atomically(&pixel, b"replacement", &missing_pixel, b"draft", true).is_err()
        );
        assert_eq!(fs::read(&pixel).unwrap(), b"old pixel");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn non_forced_pair_rolls_back_without_overwriting_a_newly_created_target() {
        let directory = unique_test_dir("source-no-clobber");
        let vertex = directory.join("draft.vs");
        let pixel = directory.join("draft.ps");
        let mut entries = [
            StagedReplacement {
                target: vertex.clone(),
                temp: write_sibling_temp(&vertex, b"new vertex").unwrap(),
                backup: None,
                installed: false,
            },
            StagedReplacement {
                target: pixel.clone(),
                temp: write_sibling_temp(&pixel, b"new pixel").unwrap(),
                backup: None,
                installed: false,
            },
        ];
        fs::write(&pixel, b"created while save was pending").unwrap();

        assert!(commit_replacements(&mut entries, false).is_err());
        assert!(!vertex.exists());
        assert_eq!(fs::read(&pixel).unwrap(), b"created while save was pending");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn paired_writer_rolls_back_both_files_when_second_commit_fails() {
        let directory = unique_test_dir("pair-rollback");
        let first_target = directory.join("pair.vs");
        let second_target = directory.join("pair.ps");
        fs::write(&first_target, b"old vertex").unwrap();
        fs::write(&second_target, b"old pixel").unwrap();

        let first_temp = write_sibling_temp(&first_target, b"new vertex").unwrap();
        let second_temp = write_sibling_temp(&second_target, b"new pixel").unwrap();
        fs::remove_file(&second_temp).unwrap();
        let mut entries = [
            StagedReplacement {
                target: first_target.clone(),
                temp: first_temp,
                backup: None,
                installed: false,
            },
            StagedReplacement {
                target: second_target.clone(),
                temp: second_temp,
                backup: None,
                installed: false,
            },
        ];

        assert!(commit_replacements(&mut entries, true).is_err());
        assert_eq!(fs::read(&first_target).unwrap(), b"old vertex");
        assert_eq!(fs::read(&second_target).unwrap(), b"old pixel");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn official_ksh_binary_codec_round_trips_byte_for_byte() {
        let Some(paths) = official_ksh_paths() else {
            return;
        };
        for path in paths {
            let bytes = fs::read(&path).unwrap();
            let parsed = parse_ksh(&bytes)
                .unwrap_or_else(|error| panic!("解析 {} 失败: {error}", path.display()));
            let rebuilt = encode_ksh(&parsed)
                .unwrap_or_else(|error| panic!("编码 {} 失败: {error}", path.display()));
            assert_eq!(rebuilt, bytes, "{} 未能字节级 round-trip", path.display());
        }
    }

    #[test]
    fn official_ksh_source_compiler_round_trips_byte_for_byte() {
        let Some(paths) = official_ksh_paths() else {
            return;
        };
        for path in paths {
            let bytes = fs::read(&path).unwrap();
            let parsed = parse_ksh(&bytes)
                .unwrap_or_else(|error| panic!("解析 {} 失败: {error}", path.display()));
            let rebuilt = build_ksh(
                &parsed.effect_name,
                &parsed.vertex.source_name,
                &parsed.vertex.source,
                &parsed.pixel.source_name,
                &parsed.pixel.source,
            )
            .unwrap_or_else(|error| panic!("从源码构建 {} 失败: {error}", path.display()));
            assert_eq!(
                rebuilt,
                bytes,
                "{} 从源码构建后未能字节级 round-trip",
                path.display()
            );
        }
    }
}
