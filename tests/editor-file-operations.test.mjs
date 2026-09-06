import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { babelParse, parse } from '@vue/compiler-sfc';

const appPath = new URL('../src/App.vue', import.meta.url);
const { descriptor, errors } = parse(readFileSync(appPath, 'utf8'));
assert.equal(errors.length, 0);
const source = descriptor.scriptSetup.content;
const ast = babelParse(source, { sourceType: 'module' });
// Execute the real component functions, replacing only browser/Tauri dependencies.
const executable = ast.program.body
  .filter(node => node.type !== 'ImportDeclaration')
  .map(node => source.slice(node.start, node.end)).join('\n');
const exposedNames = [
  'psEditor', 'vsEditor', 'psName', 'vsName', 'psModified', 'vsModified',
  'psSavePoint', 'vsSavePoint', 'psNameSavePoint', 'vsNameSavePoint',
  'currentPsPath', 'currentVsPath', 'currentKshPath', 'baseKshPath',
  'showError', 'showConfirm', 'currentSaveFile', 'pendingOperation',
  'setupEditorChangeListener', 'refreshModified', 'handleSave', 'handleSaveAs',
  'handleSaveKsh', 'doOpenPs', 'doOpenVs', 'doOpenKsh',
  'runAfterSavePrompts', 'handleConfirmAction',
];

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

async function settle() {
  await new Promise(resolve => setImmediate(resolve));
}

function createApp(overrides = {}) {
  const calls = {};
  const context = {
    ref: value => ({ value }),
    shallowRef: value => ({ value }),
    onMounted: () => {},
    onUnmounted: () => {},
    nextTick: callback => Promise.resolve().then(callback),
    setTimeout: callback => queueMicrotask(callback),
    self: {},
    getCurrentWindow: () => ({}),
  };
  for (const name of ['openFileDialog', 'saveFileDialog', 'readFile', 'writeFile', 'analyzeKsh', 'buildKsh']) {
    calls[name] = [];
    context[name] = (...args) => {
      calls[name].push(args);
      if (!overrides[name]) throw new Error(`Unexpected ${name} call`);
      return overrides[name](...args);
    };
  }
  const app = runInNewContext(`${executable}\n({ ${exposedNames.join(', ')} });`, context, {
    filename: appPath.pathname,
  });
  const stages = {};
  for (const stage of ['ps', 'vs']) {
    const prefix = stage === 'ps' ? 'Ps' : 'Vs';
    let content = `original ${stage}`;
    let changed = () => {};
    const editor = {
      getValue: () => content,
      setValue: value => { content = value; changed(); },
      onDidChangeModelContent: callback => { changed = callback; },
    };
    app[`${stage}Editor`].value = editor;
    app[`${stage}SavePoint`].value = content;
    app.setupEditorChangeListener(editor, stage === 'ps');
    stages[stage] = {
      editor,
      name: app[`${stage}Name`],
      modified: app[`${stage}Modified`],
      savePoint: app[`${stage}SavePoint`],
      nameSavePoint: app[`${stage}NameSavePoint`],
      path: app[`current${prefix}Path`],
      open: app[`doOpen${prefix}`],
      edit: value => editor.setValue(value),
    };
  }
  return { app, stages, calls };
}

const importedKsh = {
  ps: { name: 'loaded.ps', content: 'loaded ps' },
  vs: { name: 'loaded.vs', content: 'loaded vs' },
};

for (const stage of ['ps', 'vs']) {
  for (const success of [false, true]) {
    test(`${stage}: ${success ? 'successful' : 'failed'} open during Save As preserves the correct document`, async () => {
      const write = deferred();
      const { app, stages, calls } = createApp({
        saveFileDialog: () => `C:/saved/saved.${stage}`,
        writeFile: () => write.promise,
        openFileDialog: () => `C:/loaded/loaded.${stage}`,
        readFile: () => success ? `loaded ${stage}` : Promise.reject(new Error('read failed')),
      });
      const current = stages[stage];
      current.edit(`submitted ${stage}`);
      const save = app.handleSaveAs(stage);
      await settle();
      assert.equal(calls.writeFile.length, 1);
      await current.open();
      assert.equal(app.showError.value, !success);
      write.resolve();
      assert.equal(await save, true);
      assert.equal(current.path.value, `C:/${success ? 'loaded/loaded' : 'saved/saved'}.${stage}`);
      assert.equal(current.editor.getValue(), `${success ? 'loaded' : 'submitted'} ${stage}`);
      assert.equal(current.savePoint.value, current.editor.getValue());
      assert.equal(current.name.value, success ? 'loaded' : 'saved');
      assert.equal(current.nameSavePoint.value, current.name.value);
      assert.equal(current.modified.value, false);
    });
  }

  test(`${stage}: edits made while saving remain dirty`, async () => {
    const write = deferred();
    const { app, stages } = createApp({
      saveFileDialog: () => `C:/saved/saved.${stage}`,
      writeFile: () => write.promise,
    });
    const current = stages[stage];
    current.edit('submitted');
    const save = app.handleSaveAs(stage);
    await settle();
    current.edit('newer edit');
    write.resolve();
    await save;
    assert.equal(current.editor.getValue(), 'newer edit');
    assert.equal(current.savePoint.value, 'submitted');
    assert.equal(current.modified.value, true);
  });

  test(`${stage}: successive writes are serialized and the latest save point wins`, async () => {
    const firstWrite = deferred();
    const secondWrite = deferred();
    let writes = 0;
    const { app, stages, calls } = createApp({
      writeFile: () => (++writes === 1 ? firstWrite.promise : secondWrite.promise),
    });
    const current = stages[stage];
    current.path.value = `C:/saved/shader.${stage}`;
    current.edit('first');
    const firstSave = app.handleSave(stage);
    await settle();
    current.edit('second');
    const secondSave = app.handleSave(stage);
    await settle();
    assert.equal(calls.writeFile.length, 1);
    firstWrite.resolve();
    await firstSave;
    await settle();
    assert.equal(calls.writeFile.length, 2);
    assert.equal(current.modified.value, true);
    assert.equal(current.savePoint.value, `original ${stage}`);
    secondWrite.resolve();
    await secondSave;
    assert.equal(calls.writeFile[1][1], 'second');
    assert.equal(current.savePoint.value, 'second');
    assert.equal(current.modified.value, false);
  });
}

for (const kind of ['ps', 'vs', 'ksh']) {
  for (const phase of ['dialog', 'read']) {
    for (const change of ['content', 'name']) {
      test(`${kind}: ${change} changes during ${phase} prevent a stale load`, async () => {
        const wait = deferred();
        const target = kind === 'vs' ? 'vs' : 'ps';
        const payload = kind === 'ksh' ? importedKsh : `loaded ${kind}`;
        const { app, stages, calls } = createApp({
          openFileDialog: () => phase === 'dialog' ? wait.promise : `C:/loaded/loaded.${kind}`,
          readFile: () => phase === 'read' ? wait.promise : payload,
          analyzeKsh: () => phase === 'read' ? wait.promise : payload,
        });
        const opening = kind === 'ksh' ? app.doOpenKsh() : stages[kind].open();
        await settle();
        if (change === 'content') {
          stages[target].edit('newer edit');
        } else {
          stages[target].name.value = 'renamed';
          app.refreshModified(target);
        }
        wait.resolve(phase === 'dialog' ? `C:/loaded/loaded.${kind}` : payload);
        await opening;
        assert.equal(stages[target].editor.getValue(), change === 'content' ? 'newer edit' : `original ${target}`);
        assert.equal(stages[target].name.value, change === 'name' ? 'renamed' : 'shader');
        assert.equal(stages[target].path.value, '');
        assert.equal(stages[target].savePoint.value, `original ${target}`);
        assert.equal(stages[target].modified.value, true);
        assert.equal(app.currentKshPath.value, '');
        assert.equal(app.showError.value, true);
        if (phase === 'dialog') assert.equal(calls.readFile.length + calls.analyzeKsh.length, 0);
      });
    }
  }

  for (const success of [false, true]) {
    test(`${kind}: ${success ? 'successful' : 'failed'} open during KSH export preserves the correct save points`, async () => {
      const build = deferred();
      const { app, stages, calls } = createApp({
        saveFileDialog: () => 'C:/saved/saved.ksh',
        buildKsh: () => build.promise,
        openFileDialog: () => `C:/loaded/loaded.${kind}`,
        readFile: () => success ? `loaded ${kind}` : Promise.reject(new Error('read failed')),
        analyzeKsh: () => success ? importedKsh : Promise.reject(new Error('analysis failed')),
      });
      stages.ps.edit('submitted ps');
      stages.vs.edit('submitted vs');
      const save = app.handleSaveKsh();
      await settle();
      assert.equal(calls.buildKsh.length, 1);
      await (kind === 'ksh' ? app.doOpenKsh() : stages[kind].open());
      assert.equal(app.showError.value, !success);
      build.resolve();
      await save;
      const kshPath = success ? (kind === 'ksh' ? 'C:/loaded/loaded.ksh' : '') : 'C:/saved/saved.ksh';
      assert.equal(app.currentKshPath.value, kshPath);
      assert.equal(app.baseKshPath.value, kshPath);
      for (const stage of !success || kind === 'ksh' ? ['ps', 'vs'] : [kind]) {
        assert.equal(stages[stage].savePoint.value, `${success ? 'loaded' : 'submitted'} ${stage}`);
        assert.equal(stages[stage].modified.value, false);
      }
    });
  }
}

for (const success of [false, true]) {
  test(`KSH import ${success ? 'success invalidates' : 'failure preserves'} both pending stage saves`, async () => {
    const write = deferred();
    const { app, stages, calls } = createApp({
      saveFileDialog: options => options.filters[0].extensions[0] === 'ps' ? 'C:/saved/saved.ps' : 'C:/saved/saved.vs',
      writeFile: () => write.promise,
      openFileDialog: () => 'C:/loaded/loaded.ksh',
      analyzeKsh: () => success ? importedKsh : Promise.reject(new Error('analysis failed')),
    });
    stages.ps.edit('submitted ps');
    stages.vs.edit('submitted vs');
    const saves = [app.handleSaveAs('ps'), app.handleSaveAs('vs')];
    await settle();
    assert.equal(calls.writeFile.length, 2);
    await app.doOpenKsh();
    write.resolve();
    await Promise.all(saves);
    for (const stage of ['ps', 'vs']) {
      assert.equal(stages[stage].path.value, success ? '' : `C:/saved/saved.${stage}`);
      assert.equal(stages[stage].savePoint.value, `${success ? 'loaded' : 'submitted'} ${stage}`);
      assert.equal(stages[stage].name.value, success ? 'loaded' : 'saved');
      assert.equal(stages[stage].modified.value, false);
    }
  });
}

test('the save queue prompts for a stage that became dirty after the operation started', async () => {
  const { app, stages } = createApp();
  let executions = 0;
  stages.ps.edit('first ps edit');
  await app.runAfterSavePrompts(() => { executions += 1; }, ['ps', 'vs']);
  assert.equal(app.currentSaveFile.value, 'shader.ps');
  stages.vs.edit('new vs edit');
  await app.handleConfirmAction('discard');
  assert.equal(executions, 0);
  assert.equal(app.showConfirm.value, true);
  assert.equal(app.currentSaveFile.value, 'shader.vs');
  await app.handleConfirmAction('discard');
  assert.equal(executions, 1);
  assert.equal(app.pendingOperation.value, null);
});

test('discard confirmation does not discard changes made while it was displayed', async () => {
  const { app, stages } = createApp();
  let executions = 0;
  stages.ps.edit('first edit');
  await app.runAfterSavePrompts(() => { executions += 1; }, ['ps']);
  stages.ps.edit('newer edit');
  await app.handleConfirmAction('discard');
  assert.equal(executions, 0);
  assert.equal(app.showConfirm.value, true);
  await app.handleConfirmAction('discard');
  assert.equal(executions, 1);
});

test('previously discarded stages are prompted again if edited while another stage is pending', async () => {
  const { app, stages } = createApp();
  let executions = 0;
  stages.ps.edit('first ps edit');
  stages.vs.edit('first vs edit');
  await app.runAfterSavePrompts(() => { executions += 1; }, ['ps', 'vs']);
  await app.handleConfirmAction('discard');
  assert.equal(app.currentSaveFile.value, 'shader.vs');
  stages.ps.edit('newer ps edit');
  await app.handleConfirmAction('discard');
  assert.equal(executions, 0);
  assert.equal(app.currentSaveFile.value, 'shader.ps');
  await app.handleConfirmAction('cancel');
  assert.equal(executions, 0);
  assert.equal(app.pendingOperation.value, null);
});
