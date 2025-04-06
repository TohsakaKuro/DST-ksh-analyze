<script setup>
import { ref, onMounted, watch, nextTick, shallowRef, onUnmounted } from "vue";
import * as monaco from 'monaco-editor';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution';
import 'monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution';
import 'monaco-editor/esm/vs/editor/contrib/find/browser/findController';
import { analyzeKsh, buildKsh, openFileDialog, saveFileDialog, readFile, writeFile } from './utils/tauri-commands';
import ErrorDialog from './components/ErrorDialog.vue';
import ConfirmDialog from './components/ConfirmDialog.vue';
import AboutDialog from './components/AboutDialog.vue';

// 设置 worker
self.MonacoEnvironment = {
  getWorker: function (workerId, label) {
    return new EditorWorker();
  },
};

const psEditor = shallowRef(null);
const vsEditor = shallowRef(null);
const psEditorContainer = ref(null);
const vsEditorContainer = ref(null);

// 其他状态
const activeTab = ref('ps');
const psName = ref('shader');
const vsName = ref('shader');
const editingFileName = ref(false);
const psModified = ref(false);
const vsModified = ref(false);
const showAboutDialog = ref(false);

// 添加文件操作相关的状态
const currentKshPath = ref('');
const currentVsPath = ref('');
const currentPsPath = ref('');

// 添加错误提示状态
const showError = ref(false);
const errorMessage = ref('');

// 添加确认对话框状态
const showConfirm = ref(false);
const confirmTitle = ref('');
const confirmMessage = ref('');
const confirmCallback = ref(null);

// 添加文件路径提示状态
const showPsPathTooltip = ref(false);

// 添加保存队列状态
const saveQueue = ref([]);
const currentSaveFile = ref(null);

// 添加待执行操作状态
const pendingOperation = ref(null);

// 添加保存点状态
const psSavePoint = ref('');
const vsSavePoint = ref('');

// 初始化编辑器
const initEditor = (container, content) => {
  const editor = monaco.editor.create(container, {
    value: content,
    language: 'glsl',
    theme: 'vs-dark',
    minimap: { enabled: true },
    scrollBeyondLastLine: false,
    fontSize: 14,
    fontFamily: 'Consolas, "Courier New", monospace',
    fontLigatures: false,
    lineNumbers: 'on',
    renderWhitespace: 'selection',
    tabSize: 2,
    automaticLayout: true,
    fixedOverflowWidgets: true,
    useTabStops: true,
    renderControlCharacters: true,
    renderIndentGuides: true,
    wordWrap: 'off',
    cursorStyle: 'line',
    cursorWidth: 2,
    cursorBlinking: 'blink',
    mouseWheelZoom: false,
    letterSpacing: 0,
    lineHeight: 20,
  });

  // 设置编辑器的字体特性
  editor.updateOptions({
    fontFamily: 'Consolas, "Courier New", monospace',
    fontSize: 14,
    lineHeight: 20,
  });

  // 添加注释快捷键
  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Slash, () => {
    toggleComment();
  });

  window.addEventListener('resize', () => {
    editor.layout();
  });

  return editor;
};

// 修改切换标签页函数
const switchTab = (tab) => {
  // 如果正在编辑文件名，不要切换标签页
  if (editingFileName.value) return;
  
  activeTab.value = tab;
  nextTick(() => {
    if (tab === 'ps' && psEditor.value) {
      psEditor.value.layout();
    } else if (tab === 'vs' && vsEditor.value) {
      vsEditor.value.layout();
    }
  });
};

// 修改开始编辑函数
const startEditing = (event) => {
  // 阻止事件冒泡，防止触发标签切换
  event?.stopPropagation();
  editingFileName.value = true;
  // 在下一个 tick 聚焦输入框
  nextTick(() => {
    const input = document.querySelector('.filename-input');
    input?.focus();
  });
};

// 修改结束编辑函数
const finishEditing = () => {
  editingFileName.value = false;
};

// 修改保存函数
async function handleSave() {
  const isPs = activeTab.value === 'ps';
  const currentPath = isPs ? currentPsPath.value : currentVsPath.value;
  
  if (!currentPath) {
    return handleSaveAs();
  }
  
  try {
    const content = (isPs ? psEditor.value : vsEditor.value)?.getValue() || '';
    await writeFile(currentPath, content);
    if (isPs) {
      psModified.value = false;
      psSavePoint.value = content; // 记录保存点
    } else {
      vsModified.value = false;
      vsSavePoint.value = content; // 记录保存点
    }
  } catch (error) {
    showError.value = true;
    errorMessage.value = `保存文件失败: ${error}`;
  }
}

// 修改另存为函数
async function handleSaveAs() {
  const isPs = activeTab.value === 'ps';
  const currentPath = isPs ? currentPsPath.value : currentVsPath.value;
  const fileName = isPs ? psName.value : vsName.value;
  const extension = isPs ? 'ps' : 'vs';
  
  try {
    const filePath = await saveFileDialog({
      title: `保存 ${extension.toUpperCase()} 文件`,
      defaultPath: currentPath || `${fileName}.${extension}`,
      filters: [{
        name: extension.toUpperCase(),
        extensions: [extension]
      }]
    });
    
    if (!filePath) return;
    
    const content = (isPs ? psEditor.value : vsEditor.value)?.getValue() || '';
    await writeFile(filePath, content);
    
    if (isPs) {
      currentPsPath.value = filePath;
      psName.value = processFileName(filePath);
      psModified.value = false;
    } else {
      currentVsPath.value = filePath;
      vsName.value = processFileName(filePath);
      vsModified.value = false;
    }
  } catch (error) {
    showError.value = true;
    errorMessage.value = `保存文件失败: ${error}`;
  }
}

// 处理双击事件
const handleDoubleClick = (event) => {
  startEditing(event);
};

// 修改编辑器内容变化监听
const setupEditorChangeListener = (editor, isPs) => {
  if (!editor) return;
  
  editor.onDidChangeModelContent(() => {
    const content = editor.getValue();
    if (isPs) {
      psModified.value = content !== psSavePoint.value;
    } else {
      vsModified.value = content !== vsSavePoint.value;
    }
  });
};

// 初始化 GLSL 语言支持
const initGlslLanguage = () => {
  monaco.languages.register({ id: 'glsl' });
  monaco.languages.setMonarchTokensProvider('glsl', {
    tokenizer: {
      root: [
        [/\/\*/, 'comment', '@comment'],
        [/\/\/.*$/, 'comment'],
        [/#\w+/, 'preprocessor'],
        [/\b(attribute|const|uniform|varying|buffer|shared|coherent|volatile|restrict|readonly|writeonly|atomic_uint|break|continue|do|for|while|if|else|in|out|inout|float|int|void|bool|true|false|invariant|precise|discard|return|mat2|mat3|mat4|vec2|vec3|vec4|ivec2|ivec3|ivec4|bvec2|bvec3|bvec4|uvec2|uvec3|uvec4|lowp|mediump|highp|precision|sampler2D|sampler3D|samplerCube|struct)\b/, 'keyword'],
        [/[0-9]+\.[0-9]*|\.[0-9]+|[0-9]+/, 'number'],
        [/[=<>!]=|[+\-*/%]|\+\+|--|\|\||&&|[<>]/, 'operator'],
        [/[a-zA-Z_]\w*/, {
          cases: {
            '@default': 'identifier'
          }
        }],
      ],
      comment: [
        [/[^/*]+/, 'comment'],
        [/\*\//, 'comment', '@pop'],
        [/[/*]/, 'comment']
      ],
    }
  });
};

// 添加新的功能函数
const toggleSearch = () => {
  const editor = activeTab.value === 'ps' ? psEditor.value : vsEditor.value;
  editor?.trigger('', 'actions.find');
};

const toggleReplace = () => {
  const editor = activeTab.value === 'ps' ? psEditor.value : vsEditor.value;
  editor?.trigger('', 'editor.action.startFindReplaceAction');
};

const toggleComment = () => {
  const editor = activeTab.value === 'ps' ? psEditor.value : vsEditor.value;
  if (!editor) return;
  
  const selection = editor.getSelection();
  if (!selection) return;
  
  const model = editor.getModel();
  const startLineNumber = selection.startLineNumber;
  const endLineNumber = selection.endLineNumber;
  
  let hasUncommentedLine = false;
  
  // 检查是否有未注释的行
  for (let i = startLineNumber; i <= endLineNumber; i++) {
    const lineContent = model.getLineContent(i).trim();
    if (!lineContent.startsWith('//')) {
      hasUncommentedLine = true;
      break;
    }
  }
  
  // 创建一个编辑操作
  const edits = [];
  for (let i = startLineNumber; i <= endLineNumber; i++) {
    const lineContent = model.getLineContent(i);
    const trimmedLine = lineContent.trimLeft();
    const indentation = lineContent.length - trimmedLine.length;
    
    if (hasUncommentedLine) {
      // 添加注释
      if (!trimmedLine.startsWith('//')) {
        edits.push({
          range: new monaco.Range(i, 1, i, lineContent.length + 1),
          text: ' '.repeat(indentation) + '//' + (trimmedLine.length > 0 ? ' ' : '') + trimmedLine
        });
      }
    } else {
      // 移除注释
      if (trimmedLine.startsWith('//')) {
        const textAfterComment = trimmedLine.slice(2).trimLeft();
        edits.push({
          range: new monaco.Range(i, 1, i, lineContent.length + 1),
          text: ' '.repeat(indentation) + textAfterComment
        });
      }
    }
  }
  
  // 作为单个可撤销的操作执行所有编辑
  if (edits.length > 0) {
    editor.pushUndoStop();
    editor.executeEdits('toggle-comment', edits);
    editor.pushUndoStop();
  }
};

// 修改处理快捷键函数
const handleKeyDown = (event) => {
  // 检查是否按下 Ctrl 键
  if (event.ctrlKey) {
    switch (event.key.toLowerCase()) {
      case 's':
        event.preventDefault();
        handleSave();
        break;
      case 'o':
        event.preventDefault();
        if (activeTab.value === 'ps') {
          handleOpenPs();
        } else {
          handleOpenVs();
        }
        break;
      case '/':
        event.preventDefault();
        toggleComment();
        break;
    }
  }
};

// 修改文件名处理函数
function processFileName(name) {
  // 处理 Windows 和 Unix 风格的路径
  const fileName = name.split(/[/\\]/).pop();
  // 移除扩展名
  return fileName.replace(/\.(ps|vs)$/i, '');
}

// 添加通用的保存提示方法
const showSaveConfirm = (fileName, callback) => {
  showConfirm.value = true;
  confirmTitle.value = `是否要保存对 ${fileName} 的更改？`;
  confirmMessage.value = '如果不保存，你的更改将丢失';
  confirmCallback.value = callback;
};

// 修改处理保存队列的函数
const processSaveQueue = async () => {
  if (saveQueue.value.length === 0) {
    if (pendingOperation.value) {
      const operation = pendingOperation.value;
      pendingOperation.value = null;
      await operation();
    }
    return;
  }

  const file = saveQueue.value[0];
  currentSaveFile.value = file;
  showSaveConfirm(file, async (action) => {
    if (action === 'save') {
      await handleSaveConfirm();
    } else if (action === 'discard') {
      await handleDiscardConfirm();
    } else if (action === 'cancel') {
      handleCancelConfirm();
    }
  });
};

const handleSaveConfirm = async () => {
  showConfirm.value = false;
  // 等待动画完成
  await new Promise(resolve => setTimeout(resolve, 200));
  
  const file = saveQueue.value[0];
  if (file === 'ps') {
    await handleSave();
  } else if (file === 'vs') {
    await handleSave();
  }
  
  saveQueue.value.shift();
  // 等待一小段时间再显示下一个对话框
  await new Promise(resolve => setTimeout(resolve, 100));
  processSaveQueue();
};

const handleDiscardConfirm = () => {
  showConfirm.value = false;
  // 等待动画完成
  setTimeout(() => {
    saveQueue.value.shift();
    // 等待一小段时间再显示下一个对话框
    setTimeout(() => {
      processSaveQueue();
    }, 100);
  }, 200);
};

const handleCancelConfirm = () => {
  showConfirm.value = false;
  // 等待动画完成
  setTimeout(() => {
    saveQueue.value = [];
    pendingOperation.value = null;
  }, 200);
};

// 修改打开 KSH 函数
async function handleOpenKsh() {
  const unsavedFiles = [];
  if (psModified.value) unsavedFiles.push(`${psName.value}.ps`);
  if (vsModified.value) unsavedFiles.push(`${vsName.value}.vs`);
  
  if (unsavedFiles.length > 0) {
    saveQueue.value = [...unsavedFiles];
    currentSaveFile.value = saveQueue.value[0];
    pendingOperation.value = doOpenKsh;
    
    showSaveConfirm(currentSaveFile.value, async (action) => {
      if (action === 'save') {
        await handleSaveConfirm();
      } else if (action === 'discard') {
        await handleDiscardConfirm();
      } else if (action === 'cancel') {
        handleCancelConfirm();
      }
    });
    return;
  }
  
  await doOpenKsh();
}

// 添加实际打开 KSH 函数
async function doOpenKsh() {
  try {
    const filePath = await openFileDialog({
      title: '打开 KSH 文件',
      defaultPath: currentKshPath.value
    });
    
    if (!filePath) return;
    
    const result = await analyzeKsh(filePath);
    currentKshPath.value = filePath;
    
    // 更新编辑器内容和保存点
    if (psEditor.value) {
      psEditor.value.setValue(result.ps.content);
      psSavePoint.value = result.ps.content;
    }
    if (vsEditor.value) {
      vsEditor.value.setValue(result.vs.content);
      vsSavePoint.value = result.vs.content;
    }
    
    // 更新标签页名称
    psName.value = processFileName(result.ps.name);
    vsName.value = processFileName(result.vs.name);
    
    // 重置文件路径
    currentPsPath.value = '';
    currentVsPath.value = '';
    
    // 设置两个文件为未保存状态
    psModified.value = true;
    vsModified.value = true;
  } catch (error) {
    showError.value = true;
    errorMessage.value = `打开 KSH 文件失败: ${error}`;
  }
}

// 修改保存 KSH 函数
async function handleSaveKsh() {
  try {
    // 使用标题作为默认文件名
    const defaultFileName = psName.value || 'untitled';
    const filePath = await saveFileDialog({
      title: '保存 KSH 文件',
      defaultPath: currentKshPath.value || `${defaultFileName}.ksh`
    });
    
    if (!filePath) return;
    
    // 直接获取当前编辑器内容
    const psContent = psEditor.value?.getValue() || '';
    const vsContent = vsEditor.value?.getValue() || '';
    
    // 使用标题作为文件名
    const psShaderName = psName.value || 'untitled.ps';
    const vsShaderName = vsName.value || 'untitled.vs';
    
    // 直接调用后端构建KSH
    await buildKsh({
      output_path: filePath,
      vs_name: vsShaderName,
      vs_content: vsContent,
      ps_name: psShaderName,
      ps_content: psContent
    });
    
    currentKshPath.value = filePath;
  } catch (error) {
    showError.value = true;
    errorMessage.value = `保存 KSH 文件失败: ${error}`;
  }
}

// 修改打开 PS 函数
async function handleOpenPs() {
  if (psModified.value) {
    showSaveConfirm(`${psName.value}.ps`, async (action) => {
      showConfirm.value = false;
      if (action === 'save') {
        await handleSave();
        await doOpenPs();
      } else if (action === 'discard') {
        await doOpenPs();
      }
    });
    return;
  }
  await doOpenPs();
}

// 添加实际打开 PS 函数
async function doOpenPs() {
  try {
    const filePath = await openFileDialog({
      title: '打开 PS 文件',
      defaultPath: currentPsPath.value,
      filters: [{
        name: 'PS',
        extensions: ['ps']
      }]
    });
    
    if (!filePath) return;
    
    const content = await readFile(filePath);
    if (psEditor.value) {
      psEditor.value.setValue(content);
      psSavePoint.value = content; // 设置保存点
    }
    
    currentPsPath.value = filePath;
    psName.value = processFileName(filePath);
    psModified.value = false;
  } catch (error) {
    showError.value = true;
    errorMessage.value = `打开 PS 文件失败: ${error}`;
  }
}

// 修改打开 VS 函数
async function handleOpenVs() {
  if (vsModified.value) {
    showSaveConfirm(`${vsName.value}.vs`, async (action) => {
      showConfirm.value = false;
      if (action === 'save') {
        await handleSave();
        await doOpenVs();
      } else if (action === 'discard') {
        await doOpenVs();
      }
    });
    return;
  }
  await doOpenVs();
}

// 添加实际打开 VS 函数
async function doOpenVs() {
  try {
    const filePath = await openFileDialog({
      title: '打开 VS 文件',
      defaultPath: currentVsPath.value,
      filters: [{
        name: 'VS',
        extensions: ['vs']
      }]
    });
    
    if (!filePath) return;
    
    const content = await readFile(filePath);
    if (vsEditor.value) {
      vsEditor.value.setValue(content);
      vsSavePoint.value = content; // 设置保存点
    }
    
    currentVsPath.value = filePath;
    vsName.value = processFileName(filePath);
    vsModified.value = false;
  } catch (error) {
    showError.value = true;
    errorMessage.value = `打开 VS 文件失败: ${error}`;
  }
}

// 初始化
onMounted(() => {
  initGlslLanguage();
  
  // 初始化 PS 编辑器
  psEditor.value = initEditor(psEditorContainer.value, '// PS 着色器代码');
  setupEditorChangeListener(psEditor.value, true);

  // 初始化 VS 编辑器
  vsEditor.value = initEditor(vsEditorContainer.value, '// VS 着色器代码');
  setupEditorChangeListener(vsEditor.value, false);

  // 添加全局快捷键监听
  window.addEventListener('keydown', handleKeyDown);
});

// 在组件卸载时移除事件监听
onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<template>
  <div class="app-container">
    <div class="main-panel">
      <div class="editor-section">
        <div class="tab-bar">
          <div class="tab-list">
            <!-- PS Tab -->
            <div 
              class="tab" 
              :class="{ 
                active: activeTab === 'ps',
                modified: psModified 
              }"
              @dblclick="handleDoubleClick"
              @mouseenter="showPsPathTooltip = true"
              @mouseleave="showPsPathTooltip = false"
            >
              <div class="tab-content" @click="switchTab('ps')">
                <span v-if="!editingFileName || activeTab !== 'ps'" class="tab-text">
                  {{ psName }}<span class="extension">.ps</span>
                </span>
                <input
                  v-else
                  v-model="psName"
                  class="filename-input"
                  @blur="finishEditing"
                  @keyup.enter="finishEditing"
                />
              </div>
              <div v-if="activeTab === 'ps'" class="tab-actions">
                <button class="icon-button edit" @click="startEditing" title="编辑文件名">
                  <span class="icon">✎</span>
                </button>
              </div>
              <div v-if="showPsPathTooltip && currentPsPath" class="tab-tooltip">
                {{ currentPsPath }}
              </div>
            </div>
            <!-- VS Tab -->
            <div 
              class="tab" 
              :class="{ 
                active: activeTab === 'vs',
                modified: vsModified 
              }"
              @dblclick="handleDoubleClick"
            >
              <div class="tab-content" @click="switchTab('vs')">
                <span v-if="!editingFileName || activeTab !== 'vs'" class="tab-text">
                  {{ vsName }}<span class="extension">.vs</span>
                </span>
                <input
                  v-else
                  v-model="vsName"
                  class="filename-input"
                  @blur="finishEditing"
                  @keyup.enter="finishEditing"
                />
              </div>
              <div v-if="activeTab === 'vs'" class="tab-actions">
                <button class="icon-button edit" @click="startEditing" title="编辑文件名">
                  <span class="icon">✎</span>
                </button>
              </div>
            </div>
          </div>
          <div class="global-actions">
            <button class="tool-button primary" @click="handleOpenKsh">从 KSH 导入</button>
            <button class="tool-button primary" @click="handleSaveKsh">导出到 KSH</button>
            <button class="tool-button" @click="showAboutDialog = true" title="关于 DST-ksh-analyze">
              <span class="icon">ℹ️</span>
            </button>
          </div>
        </div>

        <div class="file-actions-bar">
          <div class="file-actions">
            <button 
              class="icon-button" 
              :title="activeTab === 'ps' ? '打开 PS 文件 (Ctrl+O)' : '打开 VS 文件 (Ctrl+O)'" 
              @click="activeTab === 'ps' ? handleOpenPs() : handleOpenVs()"
            >
              <span class="icon">📂</span>
            </button>
            <button class="icon-button" title="保存文件 (Ctrl+S)" @click="handleSave">
              <span class="icon">💾</span>
            </button>
            <button class="icon-button primary" title="另存为" @click="handleSaveAs">
              <span class="icon">💾</span>
            </button>
            <div class="separator"></div>
            <button class="icon-button" title="撤销 (Ctrl+Z)" @click="undo">
              <span class="icon">↩</span>
            </button>
            <button class="icon-button" title="重做 (Ctrl+Y)" @click="redo">
              <span class="icon">↪</span>
            </button>
            <div class="separator"></div>
            <button class="icon-button" title="查找/替换 (Ctrl+F)" @click="toggleSearch">
              <span class="icon">🔍</span>
            </button>
            <div class="separator"></div>
            <button class="icon-button" title="注释/取消注释 (Ctrl+/)" @click="toggleComment">
              <span class="icon">//</span>
            </button>
          </div>
        </div>

        <div class="editor-container">
          <div 
            v-show="activeTab === 'ps'" 
            ref="psEditorContainer" 
            class="monaco-editor-instance"
          ></div>
          <div 
            v-show="activeTab === 'vs'" 
            ref="vsEditorContainer" 
            class="monaco-editor-instance"
          ></div>
        </div>
      </div>
    </div>

    <!-- 关于对话框 -->
    <AboutDialog
      :show="showAboutDialog"
      @close="showAboutDialog = false"
    />

    <!-- 添加错误对话框 -->
    <ErrorDialog
      :show="showError"
      :message="errorMessage"
      @close="showError = false"
    />
    
    <!-- 修改确认对话框 -->
    <ConfirmDialog
      :show="showConfirm"
      :title="confirmTitle"
      :message="confirmMessage"
      :file="currentSaveFile"
      @save="handleSaveConfirm"
      @discard="handleDiscardConfirm"
      @cancel="handleCancelConfirm"
    />
  </div>
</template>

<style>
/* 基础样式 */
:root {
  --window-bg: #1e1e1e;
  --toolbar-bg: #2d2d2d;
  --tab-bg: #2d2d2d;
  --tab-active-bg: #1e1e1e;
  --tab-hover-bg: #303030;
  --tab-inactive-bg: #252526;
  --editor-bg: #1e1e1e;
  --text-primary: #ffffff;
  --text-secondary: #969696;
  --text-inactive: #858585;
  --border-color: #3c3c3c;
  --accent-color: #0078d4;
  --button-hover: #404040;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  background-color: var(--window-bg);
  color: var(--text-primary);
  height: 100vh;
  overflow: hidden;
  font-size: 13px;
}

.app-container {
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.main-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--window-bg);
}

.editor-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tab-bar {
  background: var(--tab-inactive-bg);
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 38px;
  padding: 0 8px;
  border-bottom: 1px solid var(--border-color);
}

.tab-list {
  display: flex;
  flex-direction: row;
  gap: 1px;
  height: 100%;
  overflow-x: auto;
  overflow-y: hidden;
}

/* 隐藏水平滚动条 */
.tab-list::-webkit-scrollbar {
  display: none;
}

.tab {
  height: 38px;
  background: var(--tab-active-bg);
  border: none;
  color: var(--text-inactive);
  display: flex;
  align-items: center;
  font-size: 13px;
  border-right: 1px solid var(--border-color);
  min-width: 140px;
  max-width: 240px;
  flex-shrink: 0;
  position: relative;
  margin-right: 1px;
  transition: all 0.1s ease;
}

.tab:hover {
  background: var(--tab-hover-bg);
  color: var(--text-secondary);
}

.tab.active {
  background: var(--tab-bg);
  color: var(--text-primary);
  font-weight: 500;
}

.tab.modified .tab-content::before {
  content: '•';
  color: #ff5555;
  font-size: 20px;
  line-height: 0;
  margin-right: 4px;
  position: relative;
  top: -1px;
}

.tab-content {
  flex: 1;
  padding: 0 12px;
  display: flex;
  align-items: center;
  cursor: pointer;
  min-width: 100px;
  white-space: nowrap;
  height: 100%;
}

.tab-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.extension {
  opacity: 0.6;
  margin-left: 2px;
  font-weight: normal;
}

.tab-actions {
  display: flex;
  align-items: center;
  padding-right: 8px;
  opacity: 0.8;
}

.tab:hover .tab-actions {
  opacity: 1;
}

.icon-button {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 3px;
  position: relative;
  transition: all 0.2s ease;
}

.icon-button .icon {
  font-size: 14px;
  width: 14px;
  height: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.filename-input {
  background: var(--editor-bg);
  border: 1px solid var(--accent-color);
  color: var(--text-primary);
  font-size: 12px;
  padding: 2px 4px;
  border-radius: 3px;
  width: 80px;
  outline: none;
}

.filename-input:focus {
  background: var(--window-bg);
}

.editor-container {
  flex: 1;
  overflow: hidden;
  background: var(--editor-bg);
  position: relative;
  min-height: 300px;
}

.monaco-editor-instance {
  width: 100%;
  height: 100%;
  position: absolute;
  left: 0;
  top: 0;
}

.global-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0; /* 防止按钮被压缩 */
}

.file-actions-bar {
  background: var(--toolbar-bg);
  padding: 4px 8px;
  border-bottom: 1px solid var(--border-color);
  height: 32px;
  display: flex;
  align-items: center;
}

.file-actions {
  display: flex;
  gap: 4px;
  align-items: center;
}

.tool-button {
  padding: 4px 8px;
  background: transparent;
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  border-radius: 3px;
  cursor: pointer;
  height: 26px;
  font-size: 12px;
  display: flex;
  align-items: center;
}

.tool-button.primary {
  background: var(--accent-color);
  border: none;
}

.tool-button.primary:hover {
  background: #106ebe;
}

.separator {
  width: 1px;
  height: 16px;
  background: var(--border-color);
  margin: 0 4px;
}

/* 对话框样式保持不变 */
.error-dialog,
.confirm-dialog,
.about-dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background-color: var(--panel-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 16px;
  z-index: 1000;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  animation: dialogFadeIn 0.2s ease-out;
}

@keyframes dialogFadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.error-dialog {
  min-width: 300px;
  max-width: 500px;
}

.error-dialog-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
  color: var(--error-color);
}

.error-dialog-title {
  font-size: 16px;
  font-weight: 500;
}

.error-dialog-content {
  margin-bottom: 16px;
  color: var(--text-secondary);
  font-size: 14px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}

.error-dialog-footer {
  display: flex;
  justify-content: flex-end;
}

.error-dialog-button {
  padding: 6px 12px;
  background-color: var(--accent-color);
  color: white;
  border: none;
  border-radius: 3px;
  cursor: pointer;
  font-size: 13px;
}

.error-dialog-button:hover {
  opacity: 0.9;
}

.confirm-dialog {
  min-width: 300px;
  max-width: 500px;
}

.confirm-dialog-header {
  margin-bottom: 12px;
}

.confirm-dialog-title {
  font-size: 16px;
  font-weight: 500;
  color: var(--text-primary);
}

.confirm-dialog-content {
  margin-bottom: 16px;
  color: var(--text-secondary);
  font-size: 14px;
  line-height: 1.5;
}

.confirm-dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.confirm-dialog-button {
  padding: 6px 12px;
  border-radius: 3px;
  cursor: pointer;
  font-size: 13px;
  border: 1px solid var(--border-color);
}

.confirm-dialog-button.save {
  background-color: var(--accent-color);
  color: white;
  border: none;
}

.confirm-dialog-button.discard {
  background-color: transparent;
  color: var(--text-primary);
}

.confirm-dialog-button.cancel {
  background-color: transparent;
  color: var(--text-secondary);
}

.confirm-dialog-button:hover {
  opacity: 0.9;
}

.about-dialog {
  min-width: 400px;
  max-width: 600px;
}

.about-dialog-header {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
}

.about-dialog-logo {
  width: 64px;
  height: 64px;
}

.about-dialog-title {
  font-size: 24px;
  font-weight: 500;
  color: var(--text-primary);
}

.about-dialog-content {
  margin-bottom: 24px;
  color: var(--text-secondary);
  font-size: 14px;
  line-height: 1.6;
}

.about-dialog-footer {
  display: flex;
  justify-content: flex-end;
}

.about-dialog-button {
  padding: 6px 12px;
  background-color: transparent;
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  border-radius: 3px;
  cursor: pointer;
  font-size: 13px;
}

.about-dialog-button:hover {
  background-color: var(--hover-bg);
}

.tab-tooltip {
  position: absolute;
  background-color: var(--panel-bg);
  border: 1px solid var(--border-color);
  padding: 4px 8px;
  border-radius: 3px;
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  z-index: 1000;
  pointer-events: none;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}
</style>
