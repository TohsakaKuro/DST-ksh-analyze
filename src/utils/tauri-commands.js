import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { readTextFile } from '@tauri-apps/plugin-fs';

/**
 * 分析 KSH 文件
 * @param {string} filePath - KSH 文件路径
 * @returns {Promise<{vs: {name: string, content: string}, ps: {name: string, content: string}}>}
 */
export async function analyzeKsh(filePath) {
  return await invoke('analyze_ksh', { filePath });
}

/**
 * 构建 KSH 文件
 * @param {Object} params - 构建参数
 * @param {string} params.output_path - 输出 KSH 文件路径
 * @param {string|null} params.base_ksh_path - 可选的原始 KSH，用于保留可兼容元数据
 * @param {string} params.vs_name - 顶点着色器名称
 * @param {string} params.vs_content - 顶点着色器内容
 * @param {string} params.ps_name - 像素着色器名称
 * @param {string} params.ps_content - 像素着色器内容
 * @returns {Promise<void>}
 */
export async function buildKsh(params) {
  return await invoke('build_ksh', { params });
}

/**
 * 打开文件对话框
 * @param {Object} options - 对话框选项
 * @returns {Promise<string>} 选中的文件路径
 */
export async function openFileDialog(options = {}) {
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
  return await invoke('write_shader_source', { filePath, content, stage });
}
