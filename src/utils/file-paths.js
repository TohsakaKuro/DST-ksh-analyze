export function fileName(path) {
  return String(path || '').split(/[/\\]/).pop() || '';
}

export function directoryName(path) {
  const value = String(path || '');
  return value.slice(0, Math.max(value.lastIndexOf('/'), value.lastIndexOf('\\')) + 1);
}

export function extension(path) {
  const name = fileName(path);
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}

export function pathKey(path) {
  const value = String(path || '').replaceAll('\\', '/');
  return /^(?:[a-z]:\/|\/\/)/i.test(value) ? value.toLowerCase() : value;
}

export function sourceFilePath(path) {
  const value = extension(path) ? path : `${path}.glsl`;
  if (!['vs', 'ps', 'glsl', 'txt'].includes(extension(value))) {
    throw new Error('源码文件应使用 .vs、.ps、.glsl 或 .txt 扩展名');
  }
  return value;
}

export function stageFileName(value, stage) {
  let name = String(value || '').trim();
  if (!name.toLowerCase().endsWith(`.${stage}`)) {
    if (/\.(vs|ps)$/i.test(name)) throw new Error(`${stage.toUpperCase()} 名称应使用 .${stage} 扩展名`);
    name += `.${stage}`;
  }
  const stem = name.slice(0, -3);
  if (!stem || /[<>:"/\\|?*\x00-\x1f]/.test(name) || /[. ]$/.test(stem)
    || /^(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(?:\.|$)/i.test(stem)) {
    throw new Error(`${stage.toUpperCase()} 文件名无效`);
  }
  return name;
}
