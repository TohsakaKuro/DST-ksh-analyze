import { ref, shallowRef } from 'vue';
import * as desktop from '../utils/tauri-commands.js';
import { directoryName, extension, fileName, pathKey, sourceFilePath, stageFileName } from '../utils/file-paths.js';

const SOURCE_FILTERS = [
  { name: '着色器源码', extensions: ['glsl', 'vs', 'ps'] },
  { name: '文本文件', extensions: ['txt'] },
];

export function useDocumentActions(workspace, io = desktop) {
  const busy = ref('');
  const notice = shallowRef(null);
  const dialog = shallowRef(null);
  let resolveDialog;
  let nextDialogId = 0;
  function ask(kind, values = {}) {
    return new Promise(resolve => { resolveDialog = resolve; dialog.value = { kind, ...values, id: ++nextDialogId }; });
  }
  function finishDialog(result = null) {
    const resolve = resolveDialog;
    resolveDialog = null;
    dialog.value = null;
    resolve?.(result);
  }
  function notify(text, intent = 'info') { notice.value = { text, intent }; }
  async function run(label, action, errorTitle = null) {
    if (busy.value) return false;
    busy.value = label;
    notice.value = null;
    try { return await action(); }
    catch (error) {
      const message = String(error.message || error);
      if (errorTitle) await ask('error', { title: errorTitle, message });
      else notify(message, 'error');
      return false;
    }
    finally { busy.value = ''; }
  }
  async function confirmOverwrite(path) {
    if (!await io.pathExists(path)) return { force: false };
    return await ask('overwrite', { paths: [path] }) ? { force: true } : null;
  }
  function unchanged(snapshots) {
    if (snapshots.every(workspace.matches)) return true;
    throw new Error('操作期间文件已改变，本次操作已取消，请重新导出');
  }

  async function saveInternal(id, saveAs = false) {
    const snapshot = workspace.snapshot(id);
    if (!snapshot) return false;
    let path = snapshot.path;
    let force = true;
    if (!path || saveAs) {
      const defaultName = ['vs', 'ps', 'glsl', 'txt'].includes(extension(snapshot.name))
        ? snapshot.name : `${snapshot.name}.${snapshot.origin?.stage || 'glsl'}`;
      const selected = await io.saveFileDialog({
        title: '保存文件', defaultPath: path || directoryName(snapshot.origin?.path) + defaultName, filters: SOURCE_FILTERS,
      });
      if (!selected) return false;
      path = sourceFilePath(selected);
      const owner = workspace.findPath(path);
      if (owner && owner.id !== id) throw new Error('目标文件已在另一个标签中打开，请切换到该标签后保存');
      if (pathKey(path) !== pathKey(snapshot.path)) {
        const overwrite = await confirmOverwrite(path);
        if (!overwrite) return false;
        force = overwrite.force;
      }
    }
    // Capture at the command's start: edits made during a dialog/write stay unsaved.
    if (!workspace.get(id)) return false;
    await io.saveEditorSource({ path, content: snapshot.content, force });
    workspace.markSaved(snapshot, path);
    return true;
  }

  async function saveAllInternal(ids) {
    for (const id of ids) {
      const document = workspace.get(id);
      if (document && (workspace.isDirty(document) || !document.path) && !await saveInternal(id)) return false;
    }
    return true;
  }

  async function guardChanges(ids) {
    for (;;) {
      const documents = ids.map(workspace.get).filter(Boolean);
      const dirty = documents.filter(workspace.isDirty);
      if (!dirty.length) return true;
      const snapshots = documents.map(document => workspace.snapshot(document.id));
      const result = await ask('unsaved', { names: dirty.map(workspace.sourceLabel) });
      if (!result) return false;
      if (result === 'discard' && snapshots.every(workspace.matches)) return true;
      if (result === 'save' && !await saveAllInternal(dirty.map(document => document.id))) return false;
    }
  }

  function openFiles() {
    return run('打开文件', async () => {
      const selected = await io.openFileDialog({
        title: '打开文件', multiple: true,
        filters: [{ name: '着色器与源码', extensions: ['ksh', 'vs', 'ps', 'glsl', 'txt'] }],
      });
      if (!selected) return false;
      const prepared = [];
      const seen = new Set();
      for (const path of Array.isArray(selected) ? selected : [selected]) {
        if (seen.has(pathKey(path))) continue;
        seen.add(pathKey(path));
        const existing = workspace.findPath(path);
        if (existing) prepared.push({ existing: existing.id });
        else if (extension(path) === 'ksh') prepared.push({ path, ksh: await io.analyzeKsh(path) });
        else {
          sourceFilePath(path);
          prepared.push({ path, content: await io.readFile(path) });
        }
      }
      // Apply only after every selected file has been read successfully.
      for (const entry of prepared) {
        if (entry.existing) workspace.activate(entry.existing);
        else if (entry.ksh) workspace.addKsh(entry.path, entry.ksh);
        else workspace.addSource(entry);
      }
      return prepared.length > 0;
    });
  }

  function closeFiles(ids) {
    return run('关闭文件', async () => {
      if (!await guardChanges(ids)) return false;
      workspace.remove(ids);
      return true;
    });
  }

  function exportKsh() {
    return run('导出 KSH', async () => {
      if (workspace.state.documents.length < 2) return false;
      const previous = workspace.state.lastExport;
      const candidates = Object.fromEntries(['vs', 'ps'].map(stage => [stage, workspace.candidates(stage).map(document => ({
        id: document.id, name: document.name, label: workspace.sourceLabel(document), hint: workspace.stageHint(document),
      }))]));
      const selected = await ask('export', { candidates, previous });
      if (!selected) return false;
      const snapshot = workspace.exportSnapshot(selected.vsId, selected.psId);
      if (snapshot.rebuildMetadata && !await ask('metadata')) return false;
      unchanged([snapshot.vs, snapshot.ps]);
      await io.checkKsh({ base_ksh: snapshot.metadata, vs_content: snapshot.vs.content, ps_content: snapshot.ps.content });
      unchanged([snapshot.vs, snapshot.ps]);
      const defaultPath = previous?.vsId === selected.vsId && previous?.psId === selected.psId
        ? previous.path : `${fileName(snapshot.ps.name).replace(/\.[^.]+$/, '')}.ksh`;
      let path = await io.saveFileDialog({ title: '导出 KSH', defaultPath, filters: [{ name: 'KSH', extensions: ['ksh'] }] });
      if (!path) return false;
      if (!/\.ksh$/i.test(path)) path += '.ksh';
      const overwrite = await confirmOverwrite(path);
      if (!overwrite || !unchanged([snapshot.vs, snapshot.ps])) return false;
      const stem = fileName(path).slice(0, -4);
      await io.buildKsh({
        output_path: path, force: overwrite.force, name_from_output: true,
        base_ksh: snapshot.metadata, base_ksh_path: null,
        vs_name: stageFileName(`${stem}.vs`, 'vs'), vs_content: snapshot.vs.content,
        ps_name: stageFileName(`${stem}.ps`, 'ps'), ps_content: snapshot.ps.content,
      });
      workspace.state.lastExport = { ...selected, path };
      notify(`已导出 ${fileName(path)}`, 'success');
      return true;
    }, '导出 KSH 失败');
  }

  return {
    busy, notice, dialog, finishDialog, notify, openFiles, exportKsh,
    newFile: () => run('新建文件', () => { workspace.addSource(); return true; }),
    saveFile: (id = workspace.state.activeId, saveAs = false) => run('保存文件', async () => {
      const saved = await saveInternal(id, saveAs);
      if (saved) notify('文件已保存', 'success');
      return saved;
    }),
    saveAll: () => run('全部保存', async () => {
      const saved = await saveAllInternal(workspace.state.documents.map(document => document.id));
      if (saved) notify('文件已保存', 'success');
      return saved;
    }),
    closeFile: (id = workspace.state.activeId) => closeFiles(id == null ? [] : [id]),
    closeAll: () => closeFiles(workspace.state.documents.map(document => document.id)),
    canLeave: () => run('关闭窗口', () => guardChanges(workspace.state.documents.map(document => document.id))),
  };
}
