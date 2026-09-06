import { computed, reactive } from 'vue';
import { directoryName, extension, fileName, pathKey } from '../utils/file-paths.js';

export function useEditorWorkspace() {
  const state = reactive({ documents: [], activeId: null, primaryId: null, secondaryId: null, lastExport: null });
  const origins = new Map();
  let nextId = 0;
  let nextOrigin = 0;
  let nextUntitled = 0;
  const get = id => state.documents.find(document => document.id === id);
  const findPath = path => state.documents.find(document => document.path && pathKey(document.path) === pathKey(path));
  const isDirty = document => document.path ? document.content !== document.savedContent : document.content !== '';
  const active = computed(() => get(state.activeId) || null);
  const dirtyDocuments = computed(() => state.documents.filter(isDirty));
  const hasUnsavedChanges = computed(() => dirtyDocuments.value.length > 0);
  const split = computed(() => state.secondaryId !== null);

  function activate(id) {
    if (!get(id)) return;
    if (id !== state.primaryId && id !== state.secondaryId) {
      if (state.secondaryId !== null && state.activeId === state.secondaryId) state.secondaryId = id;
      else state.primaryId = id;
    }
    state.activeId = id;
  }

  function addSource(entry = {}) {
    const existing = entry.path && findPath(entry.path);
    if (existing) { activate(existing.id); return existing; }
    const document = reactive({
      id: ++nextId,
      name: entry.name || fileName(entry.path) || `未命名-${++nextUntitled}`,
      path: entry.path || '',
      content: entry.content || '',
      savedContent: entry.path ? entry.content || '' : '',
      origin: entry.origin || null,
    });
    state.documents.push(document);
    activate(document.id);
    return document;
  }

  function addKsh(path, result) {
    const originId = ++nextOrigin;
    origins.set(originId, structuredClone(result.metadata));
    const added = [];
    for (const stage of ['vs', 'ps']) {
      const existing = state.documents.find(document => document.origin?.stage === stage && pathKey(document.origin.path) === pathKey(path));
      if (existing) { activate(existing.id); added.push(existing); continue; }
      added.push(addSource({
        name: fileName(result[stage].name) || `${fileName(path).replace(/\.ksh$/i, '')}.${stage}`,
        content: result[stage].content,
        origin: { id: originId, path, stage },
      }));
    }
    if (!added.some(document => document.origin.id === originId)) origins.delete(originId);
    return added;
  }

  function snapshot(id) {
    const document = get(id);
    return document ? { id, name: document.name, path: document.path, content: document.content,
      origin: document.origin ? { ...document.origin } : null } : null;
  }
  function matches(saved) {
    const current = saved && get(saved.id);
    return Boolean(current && current.name === saved.name && current.path === saved.path && current.content === saved.content);
  }
  function markSaved(saved, path) {
    const current = saved && get(saved.id);
    if (!current) return false;
    current.path = path;
    current.name = fileName(path);
    current.savedContent = saved.content;
    return true;
  }
  function setContent(id, value) { const document = get(id); if (document) document.content = value; }

  function remove(ids) {
    const targets = new Set(ids);
    const activeIndex = state.documents.findIndex(document => document.id === state.activeId);
    state.documents = state.documents.filter(document => !targets.has(document.id));
    const nearest = state.documents[Math.min(Math.max(activeIndex, 0), state.documents.length - 1)]?.id ?? null;
    if (!get(state.primaryId)) state.primaryId = get(state.secondaryId)?.id ?? nearest;
    if (!get(state.secondaryId) || state.secondaryId === state.primaryId) state.secondaryId = null;
    if (!get(state.activeId)) state.activeId = state.primaryId;
    for (const id of origins.keys()) if (!state.documents.some(document => document.origin?.id === id)) origins.delete(id);
    if (state.lastExport && (!get(state.lastExport.vsId) || !get(state.lastExport.psId))) state.lastExport = null;
  }

  function toggleSplit() {
    if (split.value) { state.primaryId = state.activeId; state.secondaryId = null; return; }
    const other = state.documents.find(document => document.id !== state.activeId);
    if (other) { state.primaryId = state.activeId; state.secondaryId = other.id; }
  }
  function cycle(offset) {
    const count = state.documents.length;
    if (count) activate(state.documents[(state.documents.findIndex(document => document.id === state.activeId) + offset + count) % count].id);
  }
  function stageHint(document) {
    const ext = extension(document.name);
    return ['vs', 'ps'].includes(ext) ? ext : document.origin?.stage || null;
  }
  function candidates(stage) { return state.documents.filter(document => !stageHint(document) || stageHint(document) === stage); }
  function sourceLabel(document) { return document.path || (document.origin ? `${document.origin.path} / ${document.name}` : document.name); }
  function tabDetail(document) {
    if (!state.documents.some(other => other.id !== document.id && other.name === document.name)) return '';
    return document.origin ? fileName(document.origin.path) : directoryName(document.path).replace(/[/\\]$/, '').split(/[/\\]/).pop() || `#${document.id}`;
  }
  // Metadata belongs to an imported pair, not to similarly named tabs or an output filename.
  function exportSnapshot(vsId, psId) {
    const vs = snapshot(vsId);
    const ps = snapshot(psId);
    if (!vs || !ps || vsId === psId || !candidates('vs').some(document => document.id === vsId)
      || !candidates('ps').some(document => document.id === psId)) throw new Error('请选择不同的 VS 和 PS 源码文件');
    const sameOrigin = vs.origin && ps.origin && vs.origin.id === ps.origin.id && vs.origin.stage === 'vs' && ps.origin.stage === 'ps';
    const metadata = sameOrigin ? origins.get(vs.origin.id) : null;
    return { vs, ps, metadata: metadata ? structuredClone(metadata) : null,
      rebuildMetadata: Boolean((vs.origin || ps.origin) && !sameOrigin) };
  }

  return { state, active, dirtyDocuments, hasUnsavedChanges, split, get, findPath, isDirty, activate, addSource, addKsh,
    snapshot, matches, markSaved, setContent, remove, toggleSplit, cycle, stageHint, candidates, sourceLabel, tabDetail, exportSnapshot };
}
