<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { FilePlus2, FolderOpen, Save, SaveAll, PackageOpen, Undo2, Redo2, Search, Columns2, X, CircleAlert, CircleCheck, Info, FileCode2, Minus, Square, Copy } from '@lucide/vue';
import CodeEditor from './components/CodeEditor.vue';
import ToolButton from './components/ToolButton.vue';
import DocumentDialog from './components/DocumentDialog.vue';
import AboutDialog from './components/AboutDialog.vue';
import { useEditorWorkspace } from './composables/useEditorWorkspace.js';
import { useDocumentActions } from './composables/useDocumentActions.js';
import appIcon from '../src-tauri/icons/64x64.png';

const workspace = useEditorWorkspace();
const { state, active, dirtyDocuments, hasUnsavedChanges, split } = workspace;
const actions = useDocumentActions(workspace);
const { busy, notice, dialog } = actions;
const showAbout = ref(false);
const desktop = isTauri();
const appWindow = desktop ? getCurrentWindow() : null;
const maximized = ref(false);
const positions = reactive({});
const cursor = computed(() => positions[state.activeId] || { line: 1, column: 1 });
const editors = {};
let unlistenClose;
let unlistenResize;
let closeApproved = false;
let unmounted = false;
const visible = id => id === state.primaryId || id === state.secondaryId;

async function syncWindowState() {
  try {
    const value = await appWindow.isMaximized();
    if (!unmounted) maximized.value = value;
  } catch (error) { if (!unmounted) actions.notify(String(error.message || error), 'error'); }
}
async function windowAction(action) {
  if (!appWindow) return;
  try {
    await appWindow[action]();
    if (action === 'toggleMaximize') await syncWindowState();
  } catch (error) { actions.notify(String(error.message || error), 'error'); }
}
function dragTitlebar(event) {
  if (event.button !== 0 || !(event.target === event.currentTarget || event.target.matches('.header-spacer, .app-icon'))) return;
  event.preventDefault();
  windowAction(event.detail === 2 ? 'toggleMaximize' : 'startDragging');
}

async function activate(id) {
  workspace.activate(id);
  await nextTick();
  editors[id]?.focus();
}
function editorAction(action) { editors[state.activeId]?.runAction(action); }
function tabKey(event, index) {
  let target;
  if (event.key === 'ArrowLeft') target = (index - 1 + state.documents.length) % state.documents.length;
  else if (event.key === 'ArrowRight') target = (index + 1) % state.documents.length;
  else if (event.key === 'Home') target = 0;
  else if (event.key === 'End') target = state.documents.length - 1;
  if (target == null) return;
  event.preventDefault();
  workspace.activate(state.documents[target].id);
  nextTick(() => window.document.getElementById(`file-tab-${state.activeId}`)?.focus());
}
function handleKeys(event) {
  if (!(event.ctrlKey || event.metaKey) || event.altKey || dialog.value || showAbout.value || event.isComposing) return;
  const key = event.key.toLowerCase();
  if (key === 's') { event.preventDefault(); actions.saveFile(state.activeId, event.shiftKey); }
  else if (key === 'o') { event.preventDefault(); actions.openFiles(); }
  else if (key === 'n') { event.preventDefault(); actions.newFile(); }
  else if (key === 'w' && !event.shiftKey) { event.preventDefault(); actions.closeFile(); }
  else if (key === 'tab') { event.preventDefault(); workspace.cycle(event.shiftKey ? -1 : 1); }
  else if (key === 'e' && event.shiftKey) { event.preventDefault(); actions.exportKsh(); }
}
function beforeUnload(event) {
  if (hasUnsavedChanges.value) { event.preventDefault(); event.returnValue = ''; }
}
watch(() => state.activeId, async id => {
  await nextTick();
  if (id == null || unmounted) return;
  const tab = window.document.getElementById(`file-tab-${id}`);
  tab?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  if (!window.document.activeElement?.closest('.file-tabs')) editors[id]?.focus();
});
watch(() => state.documents.map(document => document.id), ids => {
  for (const id of Object.keys(positions)) if (!ids.includes(Number(id))) delete positions[id];
  for (const id of Object.keys(editors)) if (!ids.includes(Number(id))) delete editors[id];
});
onMounted(async () => {
  window.addEventListener('keydown', handleKeys);
  window.addEventListener('beforeunload', beforeUnload);
  if (!desktop) return;
  try {
    const stopListening = await appWindow.onCloseRequested(async event => {
      if (closeApproved) return;
      event.preventDefault();
      try {
        if (await actions.canLeave()) { closeApproved = true; await appWindow.close(); }
      } catch (error) { closeApproved = false; actions.notify(String(error.message || error), 'error'); }
    });
    if (unmounted) stopListening(); else unlistenClose = stopListening;
    const stopResize = await appWindow.onResized(syncWindowState);
    if (unmounted) stopResize(); else unlistenResize = stopResize;
    await syncWindowState();
  } catch (error) { actions.notify(String(error.message || error), 'error'); }
});
onBeforeUnmount(() => {
  unmounted = true;
  window.removeEventListener('keydown', handleKeys);
  window.removeEventListener('beforeunload', beforeUnload);
  unlistenClose?.();
  unlistenResize?.();
});
</script>

<template>
  <div class="app-shell">
    <header class="app-header" :class="{ 'desktop-titlebar': desktop }" @mousedown="dragTitlebar">
      <img class="app-icon" :src="appIcon" alt="DST KSH Analyze" width="22" height="22" />
      <nav class="app-menu" aria-label="主菜单">
        <fluent-menu>
          <fluent-menu-button slot="trigger" appearance="subtle" size="small">文件</fluent-menu-button>
          <fluent-menu-list>
            <fluent-menu-item :disabled.prop="Boolean(busy)" @change="actions.newFile()"><FilePlus2 slot="start" />新建文件<kbd slot="end">Ctrl+N</kbd></fluent-menu-item>
            <fluent-menu-item :disabled.prop="Boolean(busy)" @change="actions.openFiles()"><FolderOpen slot="start" />打开文件…<kbd slot="end">Ctrl+O</kbd></fluent-menu-item>
            <fluent-divider />
            <fluent-menu-item :disabled.prop="Boolean(busy) || !active" @change="actions.saveFile()"><Save slot="start" />保存<kbd slot="end">Ctrl+S</kbd></fluent-menu-item>
            <fluent-menu-item :disabled.prop="Boolean(busy) || !active" @change="actions.saveFile(state.activeId, true)">另存为…<kbd slot="end">Ctrl+Shift+S</kbd></fluent-menu-item>
            <fluent-menu-item :disabled.prop="Boolean(busy) || !state.documents.length" @change="actions.saveAll()"><SaveAll slot="start" />全部保存</fluent-menu-item>
            <fluent-divider />
            <fluent-menu-item :disabled.prop="Boolean(busy) || state.documents.length < 2" @change="actions.exportKsh()"><PackageOpen slot="start" />导出 KSH…</fluent-menu-item>
            <fluent-divider />
            <fluent-menu-item :disabled.prop="Boolean(busy) || !active" @change="actions.closeFile()">关闭文件<kbd slot="end">Ctrl+W</kbd></fluent-menu-item>
            <fluent-menu-item :disabled.prop="Boolean(busy) || !state.documents.length" @change="actions.closeAll()">关闭全部</fluent-menu-item>
          </fluent-menu-list>
        </fluent-menu>
        <fluent-menu>
          <fluent-menu-button slot="trigger" appearance="subtle" size="small">编辑</fluent-menu-button>
          <fluent-menu-list>
            <fluent-menu-item :disabled.prop="!active" @change="editorAction('undo')"><Undo2 slot="start" />撤销</fluent-menu-item>
            <fluent-menu-item :disabled.prop="!active" @change="editorAction('redo')"><Redo2 slot="start" />重做</fluent-menu-item>
            <fluent-divider />
            <fluent-menu-item :disabled.prop="!active" @change="editorAction('find')"><Search slot="start" />查找</fluent-menu-item>
            <fluent-menu-item :disabled.prop="!active" @change="editorAction('replace')">替换</fluent-menu-item>
            <fluent-menu-item :disabled.prop="!active" @change="editorAction('comment')">切换行注释</fluent-menu-item>
          </fluent-menu-list>
        </fluent-menu>
      </nav>
      <span class="header-spacer"></span>
      <div class="document-actions" role="toolbar" aria-label="文件操作">
        <ToolButton :icon="FilePlus2" label="新建文件" :disabled="Boolean(busy)" @click="actions.newFile()" />
        <ToolButton :icon="FolderOpen" label="打开文件" :disabled="Boolean(busy)" @click="actions.openFiles()" />
        <ToolButton :icon="Save" label="保存" :disabled="Boolean(busy) || !active" @click="actions.saveFile()" />
        <ToolButton :icon="SaveAll" label="全部保存" :disabled="Boolean(busy) || !state.documents.length" @click="actions.saveAll()" />
        <span class="toolbar-divider"></span>
        <fluent-button appearance="primary" size="small" :disabled-focusable.camel.prop="Boolean(busy)" :disabled.prop="state.documents.length < 2" @click="actions.exportKsh()"><PackageOpen slot="start" :size="16" aria-hidden="true" />导出 KSH</fluent-button>
      </div>
      <div class="layout-tools" role="toolbar" aria-label="编辑布局">
        <ToolButton :icon="Columns2" label="并排编辑" :selected="split" :disabled="state.documents.length < 2" @click="workspace.toggleSplit()" />
        <ToolButton :icon="Info" label="关于" @click="showAbout = true" />
      </div>
      <div v-if="desktop" class="window-controls" role="group" aria-label="窗口控制">
        <button type="button" class="window-control" title="最小化" aria-label="最小化" @click="windowAction('minimize')"><Minus aria-hidden="true" /></button>
        <button type="button" class="window-control" :title="maximized ? '还原' : '最大化'" :aria-label="maximized ? '还原' : '最大化'" @click="windowAction('toggleMaximize')"><component :is="maximized ? Copy : Square" aria-hidden="true" /></button>
        <button type="button" class="window-control window-close" title="关闭窗口" aria-label="关闭窗口" @click="windowAction('close')"><X aria-hidden="true" /></button>
      </div>
    </header>
    <main class="editor-workspace" :data-layout="split ? 'split' : 'single'" aria-label="源码编辑器">
      <div v-if="state.documents.length" class="file-tabs" role="tablist" aria-label="已打开文件">
        <div v-for="(document, index) in state.documents" :key="document.id" class="file-tab" role="presentation"
          :class="{ 'tab-active': state.activeId === document.id, 'tab-visible': visible(document.id) }"
          @auxclick.middle.prevent="actions.closeFile(document.id)">
          <button :id="`file-tab-${document.id}`" class="file-tab-button" type="button" role="tab" :title="workspace.sourceLabel(document)"
            :aria-selected="state.activeId === document.id" :aria-controls="`file-panel-${document.id}`" :tabindex="state.activeId === document.id ? 0 : -1"
            @click="activate(document.id)" @keydown="tabKey($event, index)">
            <FileCode2 aria-hidden="true" /><span class="tab-name">{{ document.name }}</span><small v-if="workspace.tabDetail(document)" class="tab-detail">{{ workspace.tabDetail(document) }}</small>
            <span v-if="workspace.isDirty(document)" class="modified-dot" aria-label="未保存"></span>
          </button>
          <ToolButton class="tab-close" :icon="X" :label="`关闭 ${document.name}`" tabindex="-1" :disabled="Boolean(busy)" @click="actions.closeFile(document.id)" />
        </div>
      </div>
      <div v-if="state.documents.length" class="editor-panes">
        <section v-for="document in state.documents" :id="`file-panel-${document.id}`" :key="document.id" v-show="visible(document.id)" class="editor-pane"
          role="tabpanel" :aria-labelledby="`file-tab-${document.id}`" :data-document-id="document.id" :data-right="document.id === state.secondaryId"
          :style="{ '--pane-column': split && document.id === state.secondaryId ? 2 : 1 }">
          <CodeEditor :ref="editor => editors[document.id] = editor" :model-value="document.content" :filename="document.name" :document-id="document.id" :active="visible(document.id)"
            @update:model-value="workspace.setContent(document.id, $event)" @focus="workspace.activate(document.id)" @cursor="positions[document.id] = $event" />
        </section>
      </div>
      <div v-else class="empty-workspace">
        <div class="empty-actions"><fluent-button appearance="subtle" @click="actions.newFile()"><FilePlus2 slot="start" />新建文件</fluent-button><fluent-button appearance="subtle" @click="actions.openFiles()"><FolderOpen slot="start" />打开文件…</fluent-button></div>
      </div>
    </main>
    <fluent-message-bar v-if="notice" :intent="notice.intent" class="operation-notice" :role="notice.intent === 'error' ? 'alert' : 'status'">
      <component :is="notice.intent === 'error' ? CircleAlert : notice.intent === 'success' ? CircleCheck : Info" slot="icon" :size="18" aria-hidden="true" />
      <span>{{ notice.text }}</span><ToolButton slot="dismiss" :icon="X" label="关闭消息" @click="notice = null" />
    </fluent-message-bar>
    <footer class="status-bar">
      <span class="status-summary">{{ busy || (dirtyDocuments.length ? `${dirtyDocuments.length} 个未保存` : '就绪') }}</span>
      <span class="status-path" :title="active ? workspace.sourceLabel(active) : ''">{{ active?.path || active?.name }}</span>
      <div v-if="active" class="status-cursor"><span>行 {{ cursor.line }}，列 {{ cursor.column }}</span><span>GLSL</span><span>UTF-8</span></div>
    </footer>
    <DocumentDialog v-if="dialog" :key="dialog.id" :model="dialog" @resolve="actions.finishDialog" />
    <AboutDialog v-if="showAbout" @close="showAbout = false" />
  </div>
</template>
