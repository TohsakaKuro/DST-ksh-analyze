import assert from 'node:assert/strict';
import { test } from 'node:test';
import { useEditorWorkspace } from '../src/composables/useEditorWorkspace.js';
import { useDocumentActions } from '../src/composables/useDocumentActions.js';

const ksh = () => ({ vs: { name: 'original.vs', content: 'vertex' }, ps: { name: 'original.ps', content: 'pixel' },
  metadata: { effect_name: 'original', uniforms: [{ name: 'X', default_data: [0x80000000] }] } });
function deferred() { let resolve; const promise = new Promise(accept => { resolve = accept; }); return { promise, resolve }; }
async function reach(predicate) {
  for (let i = 0; i < 50; i++) { if (predicate()) return; await new Promise(resolve => setImmediate(resolve)); }
  assert.ok(predicate(), 'expected action state');
}
async function answer(actions, kind, result) { await reach(() => actions.dialog.value?.kind === kind); actions.finishDialog(result); }
function harness(overrides = {}) {
  const workspace = useEditorWorkspace();
  const defaults = { openFileDialog: () => null, saveFileDialog: () => null, readFile: path => `source: ${path}`, analyzeKsh: ksh,
    pathExists: () => false, saveEditorSource: () => {}, buildKsh: () => {} };
  const calls = Object.fromEntries(Object.keys(defaults).map(name => [name, []]));
  const io = Object.fromEntries(Object.entries(defaults).map(([name, fallback]) => [name, async (...args) => {
    calls[name].push(structuredClone(args)); return await (overrides[name] || fallback)(...args);
  }]));
  return { workspace, calls, actions: useDocumentActions(workspace, io) };
}
function pair(workspace) { return [workspace.addSource({ path: 'C:/a.vs', content: 'vertex' }), workspace.addSource({ path: 'C:/b.ps', content: 'pixel' })]; }

test('new and open append without asking to discard existing unsaved files', async () => {
  const { workspace, actions } = harness({ openFileDialog: () => ['C:/a.vs', 'C:/b.vs', 'C:/effect.ksh'] });
  const first = workspace.addSource({ content: 'unsaved' });
  assert.equal(await actions.newFile(), true);
  assert.equal(await actions.openFiles(), true);
  assert.equal(workspace.state.documents.length, 6);
  assert.equal(first.content, 'unsaved');
  assert.equal(actions.dialog.value, null);
});

test('reopening existing source never rereads or replaces its edits', async () => {
  const { workspace, actions, calls } = harness({ openFileDialog: () => ['c:\\A.PS', 'C:/a.ps'] });
  const document = workspace.addSource({ path: 'C:/a.ps', content: 'old' });
  workspace.setContent(document.id, 'edited');
  await actions.openFiles();
  assert.equal(calls.readFile.length, 0);
  assert.equal(workspace.state.documents.length, 1);
  assert.equal(document.content, 'edited');
});

test('batch open is transactional when a later file fails', async () => {
  const { workspace, actions } = harness({ openFileDialog: () => ['C:/a.ps', 'C:/b.ps'], readFile: path => {
    if (path.includes('b.ps')) throw new Error('read failed'); return 'one';
  } });
  const original = workspace.addSource({ content: 'unsaved' });
  assert.equal(await actions.openFiles(), false);
  assert.deepEqual(workspace.state.documents.map(document => document.id), [original.id]);
});

test('editing while files load preserves current text and adds the loaded file', async () => {
  const read = deferred();
  const { workspace, actions, calls } = harness({ openFileDialog: () => 'C:/other.glsl', readFile: () => read.promise });
  const original = workspace.addSource({ content: 'old' });
  const opening = actions.openFiles();
  await reach(() => calls.readFile.length === 1);
  workspace.setContent(original.id, 'new');
  read.resolve('loaded');
  assert.equal(await opening, true);
  assert.equal(original.content, 'new');
  assert.equal(workspace.state.documents.length, 2);
});

test('save current writes only that file and never exports', async () => {
  const { workspace, actions, calls } = harness();
  const [vs, ps] = pair(workspace);
  workspace.setContent(vs.id, 'dirty vertex');
  workspace.setContent(ps.id, 'dirty pixel');
  assert.equal(await actions.saveFile(), true);
  assert.deepEqual(calls.saveEditorSource[0][0], { path: 'C:/b.ps', content: 'dirty pixel', force: true });
  assert.equal(workspace.isDirty(vs), true);
  assert.equal(workspace.isDirty(ps), false);
  assert.equal(calls.buildKsh.length, 0);
});

test('save unnamed uses the native filename, with GLSL as extension default', async () => {
  const { workspace, actions, calls } = harness({ saveFileDialog: () => 'C:/saved/draft' });
  const document = workspace.addSource({ content: 'unfinished (' });
  assert.equal(await actions.saveFile(), true);
  assert.equal(document.path, 'C:/saved/draft.glsl');
  assert.equal(document.name, 'draft.glsl');
  assert.equal(calls.saveEditorSource[0][0].force, false);
});

test('save as refuses paths owned by another open buffer, even case variants', async () => {
  const { workspace, actions, calls } = harness({ saveFileDialog: () => 'c:\\A.VS' });
  const [vs, ps] = pair(workspace);
  assert.equal(await actions.saveFile(ps.id, true), false);
  assert.equal(calls.saveEditorSource.length, 0);
  assert.equal(ps.path, 'C:/b.ps');
  assert.equal(vs.content, 'vertex');
});

test('imported unusual source names still allow a native save dialog', async () => {
  const { workspace, actions, calls } = harness({ saveFileDialog: () => 'C:/saved/source.vs' });
  const result = ksh();
  result.vs.name = 'original.cg';
  const [vs] = workspace.addKsh('C:/original.ksh', result);
  assert.equal(await actions.saveFile(vs.id), true);
  assert.equal(calls.saveFileDialog[0][0].defaultPath, 'C:/original.cg.vs');
  assert.equal(vs.path, 'C:/saved/source.vs');
});

test('canceling native save leaves buffer and dirty state unchanged', async () => {
  const { workspace, actions, calls } = harness();
  const document = workspace.addSource({ content: 'unsaved' });
  assert.equal(await actions.saveFile(), false);
  assert.equal(document.path, '');
  assert.equal(workspace.isDirty(document), true);
  assert.equal(calls.saveEditorSource.length, 0);
  assert.equal(actions.busy.value, '');
});

test('save-all saves multiple same-stage files and stops on cancel', async () => {
  const { workspace, actions, calls } = harness();
  const a = workspace.addSource({ path: 'C:/a.ps', content: 'a' });
  const b = workspace.addSource({ path: 'C:/b.ps', content: 'b' });
  workspace.setContent(a.id, 'a edit');
  workspace.setContent(b.id, 'b edit');
  workspace.addSource({ content: 'untitled' });
  assert.equal(await actions.saveAll(), false);
  assert.equal(calls.saveEditorSource.length, 2);
  assert.equal(workspace.isDirty(a), false);
  assert.equal(workspace.isDirty(b), false);
  assert.equal(workspace.dirtyDocuments.value.length, 1);
});

test('save failure keeps earlier save point and releases busy state', async () => {
  const { workspace, actions } = harness({ saveEditorSource: () => { throw new Error('write failed'); } });
  const [vs] = pair(workspace);
  workspace.setContent(vs.id, 'new');
  assert.equal(await actions.saveFile(vs.id), false);
  assert.equal(vs.savedContent, 'vertex');
  assert.equal(workspace.isDirty(vs), true);
  assert.equal(actions.busy.value, '');
});

test('edits during save or the native chooser remain dirty', async () => {
  for (const phase of ['picker', 'write']) {
    const wait = deferred();
    const { workspace, actions, calls } = harness({
      saveFileDialog: () => phase === 'picker' ? wait.promise : 'C:/new.glsl',
      saveEditorSource: () => phase === 'write' ? wait.promise : undefined,
    });
    const document = workspace.addSource({ content: 'old' });
    const saving = actions.saveFile();
    await reach(() => calls[phase === 'picker' ? 'saveFileDialog' : 'saveEditorSource'].length === 1);
    workspace.setContent(document.id, 'new');
    wait.resolve(phase === 'picker' ? 'C:/new.glsl' : undefined);
    assert.equal(await saving, true);
    assert.equal(document.content, 'new');
    assert.equal(document.savedContent, 'old');
    assert.equal(workspace.isDirty(document), true);
  }
});

test('source overwrite is explicit and cancel makes no write', async () => {
  const { workspace, actions, calls } = harness({ saveFileDialog: () => 'C:/existing.ps', pathExists: () => true });
  workspace.addSource({ content: 'new' });
  const canceled = actions.saveFile();
  await answer(actions, 'overwrite', null);
  assert.equal(await canceled, false);
  assert.equal(calls.saveEditorSource.length, 0);
  const accepted = actions.saveFile();
  await answer(actions, 'overwrite', true);
  assert.equal(await accepted, true);
  assert.equal(calls.saveEditorSource[0][0].force, true);
});

test('closing a clean/empty tab never asks, dirty tab cancel keeps everything', async () => {
  const { workspace, actions } = harness();
  const empty = workspace.addSource();
  assert.equal(await actions.closeFile(empty.id), true);
  const document = workspace.addSource({ content: 'dirty' });
  const closing = actions.closeFile();
  await answer(actions, 'unsaved', null);
  assert.equal(await closing, false);
  assert.equal(workspace.get(document.id).content, 'dirty');
});

test('close-current prompts only for that file, not other dirty tabs', async () => {
  const { workspace, actions } = harness();
  const a = workspace.addSource({ name: 'a.ps', content: 'a dirty' });
  const b = workspace.addSource({ name: 'b.ps', content: 'b dirty' });
  const closing = actions.closeFile(a.id);
  await reach(() => actions.dialog.value?.kind === 'unsaved');
  assert.deepEqual(actions.dialog.value.names, ['a.ps']);
  actions.finishDialog('discard');
  assert.equal(await closing, true);
  assert.equal(workspace.get(a.id), undefined);
  assert.equal(workspace.get(b.id).content, 'b dirty');
});

test('close-all saves each dirty file then returns to an empty workspace', async () => {
  const { workspace, actions, calls } = harness();
  const [vs, ps] = pair(workspace);
  workspace.setContent(vs.id, 'vs edit');
  workspace.setContent(ps.id, 'ps edit');
  const closing = actions.closeAll();
  await answer(actions, 'unsaved', 'save');
  assert.equal(await closing, true);
  assert.equal(calls.saveEditorSource.length, 2);
  assert.equal(workspace.state.documents.length, 0);
});

test('close-save cancel leaves all tabs open', async () => {
  const { workspace, actions } = harness();
  workspace.addSource({ content: 'dirty' });
  const closing = actions.closeAll();
  await answer(actions, 'unsaved', 'save');
  assert.equal(await closing, false);
  assert.equal(workspace.state.documents.length, 1);
});

test('close-all rechecks edits, including a previously clean tab becoming dirty', async () => {
  const { workspace, actions } = harness();
  const [vs, ps] = pair(workspace);
  workspace.setContent(vs.id, 'first edit');
  const closing = actions.closeAll();
  await reach(() => actions.dialog.value?.kind === 'unsaved');
  const id = actions.dialog.value.id;
  workspace.setContent(ps.id, 'new edit');
  actions.finishDialog('discard');
  await reach(() => actions.dialog.value?.id > id);
  actions.finishDialog(null);
  assert.equal(await closing, false);
  assert.equal(workspace.state.documents.length, 2);
});

test('close-save checks again when edits arrive during writing', async () => {
  const write = deferred();
  const { workspace, actions, calls } = harness({ saveEditorSource: () => write.promise });
  const [vs] = pair(workspace);
  workspace.setContent(vs.id, 'saved snapshot');
  const closing = actions.closeFile(vs.id);
  await answer(actions, 'unsaved', 'save');
  await reach(() => calls.saveEditorSource.length === 1);
  workspace.setContent(vs.id, 'later edit');
  write.resolve();
  await answer(actions, 'unsaved', null);
  assert.equal(await closing, false);
  assert.equal(vs.content, 'later edit');
});

test('export uses selected current buffers and output-derived names, never saves sources', async () => {
  const { workspace, actions, calls } = harness({ saveFileDialog: () => 'C:/effects/glow.ksh' });
  const [vs, ps] = workspace.addKsh('C:/original.ksh', ksh());
  workspace.setContent(ps.id, 'edited pixel');
  const exporting = actions.exportKsh();
  await answer(actions, 'export', { vsId: vs.id, psId: ps.id });
  assert.equal(await exporting, true);
  const params = calls.buildKsh[0][0];
  assert.equal(params.output_path, 'C:/effects/glow.ksh');
  assert.equal(params.name_from_output, true);
  assert.equal(params.vs_name, 'glow.vs');
  assert.equal(params.ps_name, 'glow.ps');
  assert.equal(params.ps_content, 'edited pixel');
  assert.deepEqual(params.base_ksh, ksh().metadata);
  assert.equal(params.force, false);
  assert.equal(calls.saveEditorSource.length, 0);
  assert.equal(workspace.dirtyDocuments.value.length, 2);
  assert.equal(vs.name, 'original.vs');
  assert.equal(ps.name, 'original.ps');
});

test('cross-origin export requires permission before rebuilding metadata', async () => {
  const { workspace, actions, calls } = harness({ saveFileDialog: () => 'C:/combined.ksh' });
  const [vs] = workspace.addKsh('C:/a.ksh', ksh());
  const [, ps] = workspace.addKsh('C:/b.ksh', ksh());
  const canceled = actions.exportKsh();
  await answer(actions, 'export', { vsId: vs.id, psId: ps.id });
  await answer(actions, 'metadata', null);
  assert.equal(await canceled, false);
  assert.equal(calls.saveFileDialog.length, 0);
  const accepted = actions.exportKsh();
  await answer(actions, 'export', { vsId: vs.id, psId: ps.id });
  await answer(actions, 'metadata', true);
  assert.equal(await accepted, true);
  assert.equal(calls.buildKsh[0][0].base_ksh, null);
});

test('output stems ending in stage extensions still generate names from the full stem', async () => {
  for (const stem of ['glow.vs', 'glow.ps']) {
    const { workspace, actions, calls } = harness({ saveFileDialog: () => `C:/exports/${stem}.ksh` });
    const [vs, ps] = pair(workspace);
    const exporting = actions.exportKsh();
    await answer(actions, 'export', { vsId: vs.id, psId: ps.id });
    assert.equal(await exporting, true);
    assert.equal(calls.buildKsh[0][0].vs_name, `${stem}.vs`);
    assert.equal(calls.buildKsh[0][0].ps_name, `${stem}.ps`);
  }
});

test('canceling either export step does not write or rename sources', async () => {
  const { workspace, actions, calls } = harness();
  const [vs, ps] = pair(workspace);
  const canceled = actions.exportKsh();
  await answer(actions, 'export', null);
  assert.equal(await canceled, false);
  const pickerCanceled = actions.exportKsh();
  await answer(actions, 'export', { vsId: vs.id, psId: ps.id });
  assert.equal(await pickerCanceled, false);
  assert.equal(calls.buildKsh.length, 0);
  assert.equal(workspace.state.lastExport, null);
});

test('export rejects stale selected contents but ignores unrelated file edits', async () => {
  for (const related of [true, false]) {
    const picker = deferred();
    const { workspace, actions, calls } = harness({ saveFileDialog: () => picker.promise });
    const [vs, ps] = pair(workspace);
    const other = workspace.addSource();
    const exporting = actions.exportKsh();
    await answer(actions, 'export', { vsId: vs.id, psId: ps.id });
    await reach(() => calls.saveFileDialog.length === 1);
    workspace.setContent(related ? ps.id : other.id, 'later edit');
    picker.resolve('C:/out.ksh');
    assert.equal(await exporting, !related);
    assert.equal(calls.buildKsh.length, related ? 0 : 1);
  }
});

test('export overwrite propagates explicit force and failures do not save sources', async () => {
  const { workspace, actions, calls } = harness({ saveFileDialog: () => 'C:/out.ksh', pathExists: () => true,
    buildKsh: () => { throw new Error('build failed'); } });
  const [vs, ps] = pair(workspace);
  const exporting = actions.exportKsh();
  await answer(actions, 'export', { vsId: vs.id, psId: ps.id });
  await answer(actions, 'overwrite', true);
  assert.equal(await exporting, false);
  assert.equal(calls.buildKsh[0][0].force, true);
  assert.equal(workspace.state.lastExport, null);
  assert.equal(calls.saveEditorSource.length, 0);
});

test('busy gate blocks overlapping new/open/save/close operations', async () => {
  const { workspace, actions, calls } = harness();
  const [vs, ps] = pair(workspace);
  const exporting = actions.exportKsh();
  await reach(() => actions.dialog.value?.kind === 'export');
  for (const operation of [() => actions.newFile(), () => actions.openFiles(), () => actions.saveFile(), () => actions.closeAll(), () => actions.canLeave()]) {
    assert.equal(await operation(), false);
  }
  assert.equal(workspace.state.documents.length, 2);
  assert.equal(calls.saveEditorSource.length, 0);
  assert.equal(calls.openFileDialog.length, 0);
  actions.finishDialog(null);
  assert.equal(await exporting, false);
  assert.ok(workspace.get(vs.id) && workspace.get(ps.id));
});
