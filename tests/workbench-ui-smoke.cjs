const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const output = process.env.SHADER_UI_OUTPUT || fs.mkdtempSync(path.join(os.tmpdir(), 'ksh-workspace-'));
const url = process.env.SHADER_UI_URL || 'http://127.0.0.1:1420';
fs.mkdirSync(output, { recursive: true });

// Real Vue/Fluent/Monaco, with native file and window operations isolated from disk.
function mockDesktop() {
  const callbacks = new Map();
  const listeners = new Map();
  let callbackId = 0;
  const vs = 'uniform mat4 ModelViewProjectionMatrix;\nattribute vec3 POSITION;\nattribute vec2 TEXCOORD0;\nvarying vec2 vUV;\n\nvoid main()\n{\n    vUV = TEXCOORD0;\n    gl_Position = ModelViewProjectionMatrix * vec4(POSITION, 1.0);\n}\n';
  const ps = 'uniform sampler2D TEX0;\nvarying vec2 vUV;\n\nvoid main()\n{\n    gl_FragColor = texture2D(TEX0, vUV);\n}\n';
  const result = { vs: { name: 'effect.vs', content: vs }, ps: { name: 'effect.ps', content: ps },
    metadata: { effect_name: 'effect', uniforms: [{ name: 'OPAQUE', default_data: [0x80000000, 0x7fc01234] }],
      vertex: { source_name: 'effect.vs', source: vs, uniform_indices: [0] },
      pixel: { source_name: 'effect.ps', source: ps, uniform_indices: [] } } };
  window.isTauri = true;
  window.__workbenchTest = { calls: [], openPaths: [], savePaths: [], closeFails: false, maximized: false,
    requestClose() { return callbacks.get(listeners.get('tauri://close-requested'))({ event: 'tauri://close-requested', id: 1, payload: null }); } };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' } },
    transformCallback(callback) { callbacks.set(++callbackId, callback); return callbackId; },
    unregisterCallback(id) { callbacks.delete(id); },
    async invoke(command, args) {
      const test = window.__workbenchTest;
      test.calls.push({ command, args });
      if (command === 'plugin:event|listen') { listeners.set(args.event, args.handler); return args.handler; }
      if (command === 'plugin:event|unlisten') return;
      if (command === 'plugin:dialog|open') return test.openPaths.length ? test.openPaths.splice(0) : null;
      if (command === 'plugin:dialog|save') return test.savePaths.shift() || null;
      if (command === 'analyze_ksh') return structuredClone(result);
      if (command === 'plugin:fs|exists') return false;
      if (command === 'plugin:fs|read_text_file') return Array.from(new TextEncoder().encode(args.path.endsWith('.vs') ? vs : ps));
      if (command === 'check_ksh') {
        if (test.checkError) throw new Error(test.checkError);
        return;
      }
      if (command === 'save_editor_source' || command === 'build_ksh') return;
      if (command === 'plugin:window|is_maximized') return test.maximized;
      if (command === 'plugin:window|minimize' || command === 'plugin:window|start_dragging') return;
      if (command === 'plugin:window|toggle_maximize') { test.maximized = !test.maximized; return; }
      if (command === 'plugin:window|close') {
        if (test.closeFails) throw new Error('Simulated close failure');
        queueMicrotask(() => { void test.requestClose(); });
        return;
      }
      if (command === 'plugin:window|destroy') return;
      throw new Error('Unexpected native command: ' + command);
    },
  };
}

(async () => {
  const browser = await chromium.launch({ channel: process.env.SHADER_UI_BROWSER || 'msedge', headless: true });
  const page = await browser.newPage({ viewport: { width: 1200, height: 800 }, reducedMotion: 'reduce' });
  page.setDefaultTimeout(10000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.stack || error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  await page.addInitScript(mockDesktop);
  const tool = label => page.locator(`fluent-button[aria-label="${label}"]`);
  const tab = name => page.getByRole('tab').filter({ hasText: name });
  const editor = name => page.locator('.editor-pane').filter({ has: page.getByRole('textbox', { name, exact: true }) });
  const code = name => editor(name).locator('.view-lines');
  const dialog = name => page.getByRole('dialog', { name, exact: true });
  const action = text => page.locator('fluent-dialog .dialog-actions fluent-button').filter({ hasText: text });
  const exportButton = page.locator('.document-actions fluent-button[appearance="primary"]');
  const nativeCalls = command => page.evaluate(command => window.__workbenchTest.calls.filter(call => call.command === command), command);
  const chooseSave = value => page.evaluate(value => { window.__workbenchTest.savePaths.push(value); }, value);
  async function openFiles(paths) {
    await page.evaluate(paths => { window.__workbenchTest.openPaths = paths; }, paths);
    await tool('打开文件').click();
    await page.waitForFunction(() => !document.querySelector('fluent-button[aria-label="打开文件"]').disabledFocusable);
  }
  async function append(name, text) {
    await page.getByRole('textbox', { name, exact: true }).focus();
    await page.keyboard.press('Control+End');
    await page.keyboard.insertText(text);
  }
  async function fileMenu(text) {
    await page.locator('fluent-menu-button').filter({ hasText: '文件' }).click();
    await page.locator('fluent-menu-item').filter({ hasText: text }).click();
  }
  async function capture(name) {
    await page.mouse.move(500, 500);
    await page.screenshot({ path: path.join(output, name + '.png'), fullPage: true });
  }
  async function dismissNotice() { if (await tool('关闭消息').count()) await tool('关闭消息').click(); }
  async function waitText(name, text) { await code(name).getByText(text, { exact: false }).first().waitFor(); }
  async function choosePair(vsName, psName) {
    await page.locator('fluent-dialog').getByRole('group', { name: 'VS', exact: true }).getByRole('radio', { name: vsName, exact: true }).check();
    await page.locator('fluent-dialog').getByRole('group', { name: 'PS', exact: true }).getByRole('radio', { name: psName, exact: true }).check();
  }
  try {
    await page.goto(url);
    await page.locator('.empty-workspace').waitFor();
    assert.equal(await page.getByRole('tab').count(), 0);
    assert.equal(await page.locator('.monaco-editor').count(), 0);
    assert.equal(await exportButton.evaluate(element => element.disabled), true);
    await capture('workspace-empty');
    assert.equal(await page.locator('img').count(), 1, 'one application icon in the main window');
    await page.getByRole('button', { name: '最小化', exact: true }).click();
    assert.equal((await nativeCalls('plugin:window|minimize')).length, 1);
    await page.getByRole('button', { name: '最大化', exact: true }).click();
    await page.getByRole('button', { name: '还原', exact: true }).waitFor();
    await page.getByRole('button', { name: '还原', exact: true }).click();
    await page.getByRole('button', { name: '最大化', exact: true }).waitFor();
    await page.locator('.header-spacer').dispatchEvent('mousedown', { button: 0, detail: 1 });
    assert.equal((await nativeCalls('plugin:window|start_dragging')).length, 1);
    await page.locator('.header-spacer').dispatchEvent('mousedown', { button: 0, detail: 2 });
    await page.getByRole('button', { name: '还原', exact: true }).waitFor();
    await page.getByRole('button', { name: '还原', exact: true }).click();

    await page.locator('fluent-menu-button').filter({ hasText: '文件' }).click();
    assert.equal((await nativeCalls('plugin:window|start_dragging')).length, 1, 'menu clicks must not drag the window');
    const newMenuItem = page.locator('fluent-menu-item').filter({ hasText: '新建文件' });
    const disabledSave = page.locator('fluent-menu-item').filter({ hasText: /^保存/ });
    const menuStyle = element => ({ background: getComputedStyle(element).backgroundColor, color: getComputedStyle(element).color, cursor: getComputedStyle(element).cursor });
    const enabledStyle = await newMenuItem.evaluate(menuStyle);
    const disabledStyle = await disabledSave.evaluate(menuStyle);
    assert.equal(disabledStyle.background, enabledStyle.background, 'disabled menu items have no filled background');
    assert.notEqual(disabledStyle.color, enabledStyle.color, 'disabled labels remain visually muted');
    assert.equal(disabledStyle.cursor, 'default');
    for (const slot of ['start', 'end']) {
      assert.equal(await disabledSave.locator(`[slot="${slot}"]`).evaluate(element => getComputedStyle(element).color), disabledStyle.color);
    }
    await disabledSave.hover();
    assert.deepEqual(await disabledSave.evaluate(menuStyle), disabledStyle, 'disabled hover must not highlight');
    await capture('workspace-disabled-menu');
    const saveBounds = await disabledSave.boundingBox();
    await page.mouse.click(saveBounds.x + saveBounds.width / 2, saveBounds.y + saveBounds.height / 2);
    assert.equal((await nativeCalls('plugin:dialog|save')).length, 0, 'disabled save cannot open a dialog');
    await newMenuItem.hover();
    assert.notEqual((await newMenuItem.evaluate(menuStyle)).background, enabledStyle.background, 'enabled menu items retain hover feedback');
    await page.keyboard.press('Escape');

    await tool('新建文件').click();
    await tab('未命名-1').waitFor();
    await append('未命名-1', '// scratch');
    await chooseSave('C:/saved/scratch.glsl');
    await page.keyboard.press('Control+s');
    await tab('scratch.glsl').waitFor();
    assert.equal((await nativeCalls('save_editor_source'))[0].args.params.content, '// scratch');
    await page.keyboard.press('Control+w');
    await page.locator('.empty-workspace').waitFor();
    await openFiles(['C:/src/base.vs', 'C:/src/glow.ps', 'C:/src/glow_alt.ps']);
    assert.equal(await page.getByRole('tab').count(), 3);
    await tab('base.vs').focus();
    await page.keyboard.press('End');
    await page.waitForFunction(() => document.activeElement === document.querySelector('[role="tab"][aria-selected="true"]') && document.activeElement.textContent.includes('glow_alt.ps'));
    await page.keyboard.press('Home');
    await page.waitForFunction(() => document.activeElement === document.querySelector('[role="tab"][aria-selected="true"]') && document.activeElement.textContent.includes('base.vs'));
    await tab('glow.ps').click();
    await waitText('glow.ps', 'gl_FragColor');
    await dismissNotice();
    assert.equal((await editor('glow.ps').boundingBox()).y, 78, '40px titlebar plus 38px tabs');
    assert.equal(await page.locator('.document-heading, .stage-heading').count(), 0);
    await capture('workspace-desktop');

    await append('glow.ps', '// pixel_edit');
    await tab('base.vs').click();
    await append('base.vs', '// vertex_edit');
    await tab('glow.ps').click();
    await waitText('glow.ps', 'pixel_edit');
    await page.keyboard.press('Control+z');
    await page.waitForFunction(() => !document.querySelector('[role="tabpanel"]:not([style*="display: none"]) .view-lines').textContent.includes('pixel_edit'));
    await page.keyboard.press('Control+y');
    await waitText('glow.ps', 'pixel_edit');
    const savedBefore = (await nativeCalls('save_editor_source')).length;
    await page.keyboard.press('Control+s');
    await page.waitForFunction(() => document.querySelector('.status-summary').textContent === '1 个未保存');
    assert.equal((await nativeCalls('save_editor_source')).length, savedBefore + 1);
    assert.equal((await nativeCalls('save_editor_source')).at(-1).args.params.path, 'C:/src/glow.ps');
    assert.equal(await tab('base.vs').locator('.modified-dot').count(), 1);
    await dismissNotice();

    const pickersBeforeCheck = (await nativeCalls('plugin:dialog|save')).length;
    await page.evaluate(() => { window.__workbenchTest.checkError = 'VS / PS uniform SHARED 类型不一致\nVS: float\nPS: vec2'; });
    await exportButton.click();
    await choosePair('base.vs', 'glow.ps');
    await action('导出').click();
    await dialog('导出 KSH 失败').waitFor();
    assert.ok((await page.locator('fluent-dialog .export-error').textContent()).includes('SHARED'));
    assert.equal((await nativeCalls('plugin:dialog|save')).length, pickersBeforeCheck);
    assert.equal((await nativeCalls('build_ksh')).length, 0);
    assert.equal(await page.locator('.operation-notice').count(), 0, 'export errors belong in a modal, not the editor notice');
    await capture('workspace-export-error');
    await action('确定').click();
    await page.evaluate(() => { window.__workbenchTest.checkError = null; });

    await exportButton.click();
    await dialog('导出 KSH').waitFor();
    assert.equal(await action('导出').evaluate(element => element.disabled), true, 'multiple pixel files require explicit selection');
    assert.equal(await dialog('导出 KSH').getByRole('textbox').count(), 0, 'no names to enter');
    await choosePair('base.vs', 'glow.ps');
    await capture('workspace-export');
    await chooseSave('C:/exports/glow.ksh');
    await action('导出').click();
    await page.locator('.operation-notice').getByText('已导出 glow.ksh').waitFor();
    const exported = (await nativeCalls('build_ksh')).at(-1).args.params;
    const exportCommands = await page.evaluate(() => window.__workbenchTest.calls.map(call => call.command));
    assert.ok(exportCommands.lastIndexOf('check_ksh') < exportCommands.lastIndexOf('plugin:dialog|save'), 'preflight precedes the output picker');
    assert.equal(exported.name_from_output, true);
    assert.equal(exported.vs_name, 'glow.vs');
    assert.equal(exported.ps_name, 'glow.ps');
    assert.ok(exported.vs_content.includes('vertex_edit'));
    assert.equal(exported.base_ksh, null);
    assert.equal(await tab('base.vs').locator('.modified-dot').count(), 1, 'export must not mark source saved');
    await tool('全部保存').click();
    await page.waitForFunction(() => document.querySelector('.status-summary').textContent === '就绪');
    await dismissNotice();

    await tool('并排编辑').click();
    await waitText('base.vs', 'vertex_edit');
    await waitText('glow.ps', 'pixel_edit');
    assert.equal(await page.locator('.stage-heading').count(), 0);
    await capture('workspace-split');
    await tab('glow_alt.ps').click();
    await waitText('glow_alt.ps', 'gl_FragColor');
    assert.equal(await editor('base.vs').isVisible(), true, 'switch only the active pane');
    await tool('并排编辑').click();
    await tab('glow_alt.ps').click({ button: 'middle' });
    assert.equal(await page.getByRole('tab').count(), 2);
    await tab('glow.ps').click();
    await page.keyboard.press('Control+Tab');
    await page.waitForFunction(() => document.querySelector('[role="tab"][aria-selected="true"]').textContent.includes('base.vs'));

    await openFiles(['C:/fixtures/effect.ksh']);
    assert.equal(await page.getByRole('tab').count(), 4);
    await append('effect.ps', '// imported_edit');
    await openFiles(['C:/fixtures/effect.ksh']);
    assert.equal(await page.getByRole('tab').count(), 4);
    await waitText('effect.ps', 'imported_edit');
    await exportButton.click();
    await choosePair('effect.vs', 'effect.ps');
    await chooseSave('C:/exports/renamed.ksh');
    await action('导出').click();
    await page.locator('.operation-notice').getByText('已导出 renamed.ksh').waitFor();
    const imported = (await nativeCalls('build_ksh')).at(-1).args.params;
    assert.equal(imported.base_ksh.effect_name, 'effect');
    assert.deepEqual(imported.base_ksh.uniforms[0].default_data, [0x80000000, 0x7fc01234]);
    assert.equal(imported.vs_name, 'renamed.vs');
    assert.equal(await tab('effect.ps').locator('.modified-dot').count(), 1);

    await tool('关闭 effect.ps').click();
    await dialog('保存更改？').waitFor();
    assert.equal(await page.locator('.file-list li').count(), 1);
    await page.keyboard.press('Escape');
    await tab('effect.ps').waitFor();
    await tool('关闭 effect.ps').click();
    await chooseSave('C:/extracted/saved.ps');
    await action(/^\s*保存\s*$/).click();
    await tab('effect.ps').waitFor({ state: 'detached' });
    assert.equal((await nativeCalls('save_editor_source')).at(-1).args.params.path, 'C:/extracted/saved.ps');
    await fileMenu('关闭全部');
    await dialog('保存更改？').waitFor();
    await action('取消').click();
    assert.equal(await page.getByRole('tab').count(), 3);
    await fileMenu('关闭全部');
    await action('不保存').click();
    await page.locator('.empty-workspace').waitFor();

    const longName = 'a_very_long_shader_filename_used_to_verify_tab_overflow_and_export_layout';
    await openFiles([`C:/long/${longName}.vs`, `C:/long/${longName}.ps`, 'C:/another/alternative.ps']);
    for (const width of [1000, 736, 390, 360]) {
      await page.setViewportSize({ width, height: 760 });
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, 'horizontal page overflow at ' + width);
      const bounds = await page.locator('.app-header > *').evaluateAll(elements => elements.map(element => { const r = element.getBoundingClientRect(); return [r.left, r.right]; }));
      assert.ok(bounds.every(([left, right]) => left >= 0 && right <= width), 'toolbar overflow at ' + width);
      await capture('workspace-' + width);
      await tool('并排编辑').click();
      await capture('workspace-' + width + '-split');
      const panes = await page.locator('.editor-pane:visible').evaluateAll(elements => elements.map(element => { const r = element.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; }));
      assert.equal(panes.length, 2);
      assert.ok(panes.every(pane => pane.width > 100 && pane.height > 100));
      await tool('并排编辑').click();
      await exportButton.click();
      await dialog('导出 KSH').waitFor();
      const box = await dialog('导出 KSH').boundingBox();
      assert.ok(box.x >= 0 && box.x + box.width <= width);
      await capture('workspace-' + width + '-export');
      await page.keyboard.press('Escape');
    }
    await page.setViewportSize({ width: 1200, height: 800 });
    await fileMenu('关闭全部');
    await page.locator('.empty-workspace').waitFor();
    await tool('新建文件').click();
    await append('未命名-2', '// protect');
    await page.getByRole('button', { name: '关闭窗口', exact: true }).click();
    await dialog('保存更改？').waitFor();
    await action('取消').click();
    assert.equal(await tab('未命名-2').count(), 1, 'titlebar close preserves unsaved content when canceled');
    await page.evaluate(() => { window.__workbenchTest.closeFails = true; void window.__workbenchTest.requestClose(); });
    await dialog('保存更改？').waitFor();
    await action('不保存').click();
    await page.locator('.operation-notice').getByText('Simulated close failure').waitFor();
    await page.evaluate(() => { void window.__workbenchTest.requestClose(); });
    await dialog('保存更改？').waitFor();
    await action('取消').click();
    assert.equal((await nativeCalls('plugin:window|destroy')).length, 0);
    assert.deepEqual(errors, []);
    console.log('UI smoke passed: independent tabs, current/all save, chosen-pair export, dirty close protection, split and responsive layouts.');
    console.log('Screenshots:', output);
  } catch (error) {
    console.log(await page.locator('body').ariaSnapshot());
    await capture('workspace-failure');
    throw error;
  } finally {
    console.log('Browser errors:', errors);
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
