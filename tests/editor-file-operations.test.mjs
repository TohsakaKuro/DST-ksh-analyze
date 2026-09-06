import assert from 'node:assert/strict';
import { test } from 'node:test';
import { useEditorWorkspace } from '../src/composables/useEditorWorkspace.js';
import { extension, sourceFilePath, pathKey, stageFileName } from '../src/utils/file-paths.js';

export function fixture() {
  return {
    vs: { name: 'effect.vs', content: 'vertex source' },
    ps: { name: 'effect.ps', content: 'pixel source' },
    metadata: { effect_name: 'original', uniforms: [{ name: 'OPAQUE', default_data: [0x80000000, 0x7fc01234, 0xffffffff] }],
      vertex: { source_name: 'effect.vs', source: 'vertex source', uniform_indices: [0] },
      pixel: { source_name: 'effect.ps', source: 'pixel source', uniform_indices: [] } },
  };
}

test('workspace starts completely empty', () => {
  const workspace = useEditorWorkspace();
  assert.deepEqual(workspace.state.documents, []);
  assert.equal(workspace.active.value, null);
  assert.equal(workspace.hasUnsavedChanges.value, false);
  assert.equal(workspace.split.value, false);
});

test('new files are independent, unnamed and initially clean', () => {
  const workspace = useEditorWorkspace();
  const first = workspace.addSource();
  const second = workspace.addSource();
  assert.equal(first.name, '未命名-1');
  assert.equal(second.name, '未命名-2');
  assert.equal(workspace.stageHint(first), null);
  assert.equal(workspace.isDirty(first), false);
  workspace.setContent(first.id, 'unfinished');
  assert.equal(workspace.isDirty(first), true);
  assert.equal(second.content, '');
  assert.equal(workspace.active.value.id, second.id);
});

test('existing source files are clean and new files never replace them', () => {
  const workspace = useEditorWorkspace();
  const first = workspace.addSource({ path: 'C:/src/a.ps', content: 'saved' });
  workspace.addSource();
  assert.equal(workspace.isDirty(first), false);
  assert.equal(workspace.state.documents.length, 2);
  workspace.setContent(first.id, 'edited');
  workspace.setContent(first.id, 'saved');
  assert.equal(workspace.hasUnsavedChanges.value, false);
});

test('opening equivalent Windows paths activates the same buffer without replacing edits', () => {
  const workspace = useEditorWorkspace();
  const first = workspace.addSource({ path: 'C:/src/effect.vs', content: 'original' });
  workspace.setContent(first.id, 'edited');
  const reopened = workspace.addSource({ path: 'c:\\SRC\\EFFECT.VS', content: 'disk' });
  assert.equal(reopened.id, first.id);
  assert.equal(reopened.content, 'edited');
  assert.equal(workspace.state.documents.length, 1);
});

test('same filenames in different locations remain independent and distinguishable', () => {
  const workspace = useEditorWorkspace();
  const a = workspace.addSource({ path: 'C:/one/effect.ps', content: 'one' });
  const b = workspace.addSource({ path: 'C:/two/effect.ps', content: 'two' });
  assert.notEqual(a.id, b.id);
  assert.equal(workspace.tabDetail(a), 'one');
  assert.equal(workspace.tabDetail(b), 'two');
});

test('KSH opens as two unsaved source tabs with hidden shared provenance', () => {
  const workspace = useEditorWorkspace();
  const result = fixture();
  const [vs, ps] = workspace.addKsh('C:/input/effect.ksh', result);
  assert.equal(vs.path, '');
  assert.equal(ps.path, '');
  assert.equal(workspace.dirtyDocuments.value.length, 2);
  const exported = workspace.exportSnapshot(vs.id, ps.id);
  assert.deepEqual(exported.metadata, result.metadata);
  assert.equal(exported.rebuildMetadata, false);
  result.metadata.uniforms[0].default_data[0] = 0;
  exported.metadata.uniforms[0].default_data[1] = 0;
  assert.deepEqual(workspace.exportSnapshot(vs.id, ps.id).metadata.uniforms[0].default_data, [0x80000000, 0x7fc01234, 0xffffffff]);
});

test('reopening KSH does not replace current buffers or duplicate its stages', () => {
  const workspace = useEditorWorkspace();
  const [vs, ps] = workspace.addKsh('C:/input/effect.ksh', fixture());
  workspace.setContent(vs.id, 'unsaved');
  workspace.addKsh('c:\\INPUT\\EFFECT.KSH', fixture());
  assert.equal(workspace.state.documents.length, 2);
  assert.equal(vs.content, 'unsaved');
  assert.ok(workspace.exportSnapshot(vs.id, ps.id).metadata);
});

test('reopening a missing stage uses a fresh origin, avoiding stale metadata pairing', () => {
  const workspace = useEditorWorkspace();
  const [vs, ps] = workspace.addKsh('C:/input/effect.ksh', fixture());
  workspace.remove([ps.id]);
  const [, replacement] = workspace.addKsh('C:/input/effect.ksh', fixture());
  const exported = workspace.exportSnapshot(vs.id, replacement.id);
  assert.equal(exported.metadata, null);
  assert.equal(exported.rebuildMetadata, true);
});

test('cross-KSH and mixed imported/plain pairs explicitly rebuild metadata', () => {
  const workspace = useEditorWorkspace();
  const [vs] = workspace.addKsh('C:/a.ksh', fixture());
  const [, ps] = workspace.addKsh('C:/b.ksh', fixture());
  const plain = workspace.addSource({ path: 'C:/plain.ps', content: 'pixel' });
  for (const id of [ps.id, plain.id]) {
    const result = workspace.exportSnapshot(vs.id, id);
    assert.equal(result.metadata, null);
    assert.equal(result.rebuildMetadata, true);
  }
});

test('GLSL and unnamed tabs can be explicitly assigned either export stage', () => {
  const workspace = useEditorWorkspace();
  const a = workspace.addSource({ path: 'C:/generic.glsl', content: 'one' });
  const b = workspace.addSource({ content: 'two' });
  const vs = workspace.addSource({ path: 'C:/only.vs', content: 'three' });
  assert.deepEqual(workspace.candidates('ps').map(document => document.id), [a.id, b.id]);
  assert.equal(workspace.exportSnapshot(a.id, b.id).rebuildMetadata, false);
  assert.throws(() => workspace.exportSnapshot(a.id, a.id));
  assert.throws(() => workspace.exportSnapshot(a.id, vs.id));
  assert.throws(() => workspace.exportSnapshot(a.id, -1));
});

test('saving a snapshot during edits updates its path without marking newer text saved', () => {
  const workspace = useEditorWorkspace();
  const document = workspace.addSource({ content: 'old' });
  const snapshot = workspace.snapshot(document.id);
  workspace.setContent(document.id, 'new');
  workspace.markSaved(snapshot, 'C:/saved/new.ps');
  assert.equal(document.name, 'new.ps');
  assert.equal(document.content, 'new');
  assert.equal(document.savedContent, 'old');
  assert.equal(workspace.isDirty(document), true);
  assert.equal(workspace.stageHint(document), 'ps');
});

test('saving imported sources preserves provenance and compatible defaults', () => {
  const workspace = useEditorWorkspace();
  const [vs, ps] = workspace.addKsh('C:/input/effect.ksh', fixture());
  workspace.markSaved(workspace.snapshot(vs.id), 'C:/saved/renamed.vs');
  assert.equal(workspace.isDirty(vs), false);
  assert.equal(workspace.isDirty(ps), true);
  assert.ok(workspace.exportSnapshot(vs.id, ps.id).metadata);
});

test('export history never clears source dirty state', () => {
  const workspace = useEditorWorkspace();
  const [vs, ps] = workspace.addKsh('C:/input.ksh', fixture());
  workspace.state.lastExport = { vsId: vs.id, psId: ps.id, path: 'C:/out.ksh' };
  assert.equal(workspace.hasUnsavedChanges.value, true);
  assert.equal(workspace.dirtyDocuments.value.length, 2);
});

test('closed document snapshots cannot update another document', () => {
  const workspace = useEditorWorkspace();
  const document = workspace.addSource({ content: 'original' });
  const snapshot = workspace.snapshot(document.id);
  workspace.remove([document.id]);
  const other = workspace.addSource({ content: 'new' });
  assert.equal(workspace.markSaved(snapshot, 'C:/saved.ps'), false);
  assert.equal(workspace.matches(snapshot), false);
  assert.equal(other.path, '');
});

test('closing active/inactive tabs selects a remaining neighbour and empty closes cleanly', () => {
  const workspace = useEditorWorkspace();
  const a = workspace.addSource();
  const b = workspace.addSource();
  const c = workspace.addSource();
  workspace.activate(b.id);
  workspace.remove([a.id]);
  assert.equal(workspace.state.activeId, b.id);
  workspace.remove([b.id]);
  assert.equal(workspace.state.activeId, c.id);
  workspace.remove([c.id]);
  assert.equal(workspace.state.primaryId, null);
  assert.equal(workspace.state.activeId, null);
});

test('split editing follows focused pane without binding any shader pair', () => {
  const workspace = useEditorWorkspace();
  const a = workspace.addSource();
  workspace.toggleSplit();
  assert.equal(workspace.split.value, false);
  const b = workspace.addSource();
  const c = workspace.addSource();
  workspace.activate(a.id);
  workspace.toggleSplit();
  assert.equal(workspace.state.primaryId, a.id);
  assert.equal(workspace.state.secondaryId, b.id);
  workspace.activate(b.id);
  workspace.activate(c.id);
  assert.equal(workspace.state.secondaryId, c.id);
  assert.equal(workspace.state.primaryId, a.id);
  workspace.remove([a.id]);
  assert.equal(workspace.state.primaryId, c.id);
  assert.equal(workspace.split.value, false);
});

test('tab cycling wraps and removing export members invalidates remembered selection', () => {
  const workspace = useEditorWorkspace();
  const a = workspace.addSource();
  const b = workspace.addSource();
  workspace.cycle(1);
  assert.equal(workspace.state.activeId, a.id);
  workspace.cycle(-1);
  assert.equal(workspace.state.activeId, b.id);
  workspace.state.lastExport = { vsId: a.id, psId: b.id, path: 'C:/out.ksh' };
  workspace.remove([a.id]);
  assert.equal(workspace.state.lastExport, null);
});

test('source paths support stage-free text and generated KSH names remain safe', () => {
  assert.equal(extension('untitled'), '');
  assert.equal(extension('shader.VS'), 'vs');
  assert.equal(sourceFilePath('C:/untitled'), 'C:/untitled.glsl');
  assert.equal(sourceFilePath('C:/notes.txt'), 'C:/notes.txt');
  assert.throws(() => sourceFilePath('C:/effect.ksh'));
  assert.equal(pathKey('C:\\ONE\\test.ps'), 'c:/one/test.ps');
  for (const invalid of ['../escape', 'NUL', 'COM1', '', 'bad\0']) assert.throws(() => stageFileName(invalid, 'vs'));
});
