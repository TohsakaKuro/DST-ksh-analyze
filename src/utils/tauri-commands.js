import { invoke, isTauri } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { readTextFile, exists } from '@tauri-apps/plugin-fs';

function requireDesktop() {
  if (!isTauri()) throw new Error('当前环境无法访问本地文件');
}

/**
 * 分析 KSH 文件
 * @param {string} filePath - KSH 文件路径
 * @returns {Promise<{metadata: Object, effect: string, uniforms: Array, vs: Object, ps: Object}>}
 */
export async function analyzeKsh(filePath) {
  requireDesktop();
  return await invoke('analyze_ksh', { filePath });
}

/**
 * 构建 KSH 文件
 * @param {Object} params - 构建参数
 * @param {string} params.output_path - 输出 KSH 文件路径
 * @param {string|null} params.base_ksh_path - 可选的原始 KSH，用于保留可兼容元数据
 * @param {Object|null} params.base_ksh - 优先使用的完整 KSH 元数据快照
 * @param {string} params.vs_name - 顶点着色器名称
 * @param {string} params.vs_content - 顶点着色器内容
 * @param {string} params.ps_name - 像素着色器名称
 * @param {string} params.ps_content - 像素着色器内容
 * @returns {Promise<void>}
 */
export async function buildKsh(params) {
  requireDesktop();
  return await invoke('build_ksh', { params });
}

export async function checkKsh(params) {
  requireDesktop();
  return invoke('check_ksh', { params });
}

/**
 * 打开文件对话框
 * @param {Object} options - 对话框选项
 * @returns {Promise<string>} 选中的文件路径
 */
export async function openFileDialog(options = {}) {
  requireDesktop();
  return await open({
    multiple: false,
    filters: [{
      name: 'KSH',
      extensions: ['ksh']
    }],
    ...options
  });
}

/**
 * 保存文件对话框
 * @param {Object} options - 对话框选项
 * @returns {Promise<string>} 保存的文件路径
 */
export async function saveFileDialog(options = {}) {
  requireDesktop();
  return await save({
    filters: [{
      name: 'KSH',
      extensions: ['ksh']
    }],
    ...options
  });
}

/**
 * 读取文本文件
 * @param {string} filePath - 文件路径
 * @returns {Promise<string>} 文件内容
 */
export async function readFile(filePath) {
  requireDesktop();
  return await readTextFile(filePath);
}

/**
 * 写入文本文件
 * @param {string} filePath - 文件路径
 * @param {string} content - 文件内容
 * @param {'vs'|'ps'} stage - 内容所属的着色器阶段
 * @returns {Promise<void>}
 */
export async function writeFile(filePath, content, stage) {
  requireDesktop();
  return await invoke('write_shader_source', { filePath, content, stage });
}

export async function saveSources(params) {
  requireDesktop();
  return invoke('save_shader_sources', { params });
}

export async function pathExists(path) {
  requireDesktop();
  return exists(path);
}

export async function saveEditorSource(params) {
  requireDesktop();
  return invoke('save_editor_source', { params });
}
