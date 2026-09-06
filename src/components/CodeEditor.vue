<script setup>
import { getCurrentInstance, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { initializeGlslEditor } from '../editor/glsl';

const props = defineProps({
  modelValue: { type: String, default: '' },
  filename: { type: String, default: 'shader' },
  documentId: { type: [String, Number], required: true },
  active: { type: Boolean, default: true },
  theme: { type: String, default: 'dark', validator: value => value === 'dark' || value === 'light' },
});
const emit = defineEmits(['update:modelValue', 'focus', 'cursor']);

const container = ref(null);
const instanceId = getCurrentInstance().uid;
const monaco = initializeGlslEditor();
const listeners = [];
let editor = null;
let model = null;
let modelSequence = 0;
let applyingExternalValue = false;
let savedViewState = null;

const actionIds = {
  undo: 'undo',
  redo: 'redo',
  find: 'actions.find',
  replace: 'editor.action.startFindReplaceAction',
  comment: 'editor.action.commentLine',
};

function emitCursor() {
  const position = editor?.getPosition();
  if (position) emit('cursor', { line: position.lineNumber, column: position.column });
}

function createModel(value) {
  const name = props.filename.split(/[/\\]/).pop() || 'untitled.glsl';
  const uri = monaco.Uri.from({
    scheme: 'inmemory',
    path: `/dst-shaders/${instanceId}/${++modelSequence}/${name}`,
  });
  const nextModel = monaco.editor.createModel(value, 'glsl', uri);
  nextModel.updateOptions({ tabSize: 2, insertSpaces: true });
  return nextModel;
}

function replaceDocument(value) {
  const previousModel = model;
  applyingExternalValue = true;
  try {
    model = createModel(value);
    editor.setModel(model);
    savedViewState = null;
    previousModel?.dispose();
  } finally {
    applyingExternalValue = false;
  }
  emitCursor();
}

function focus() {
  editor?.focus();
  emitCursor();
}

function layout() {
  editor?.layout();
}

function runAction(action) {
  if (!editor || !Object.hasOwn(actionIds, action)) return false;
  const actionId = actionIds[action];
  editor.focus();
  editor.trigger('workbench', actionId, null);
  return true;
}

watch(
  () => [props.documentId, props.modelValue],
  ([documentId, value], [previousDocumentId]) => {
    if (!editor) return;
    if (documentId !== previousDocumentId) {
      replaceDocument(value);
    } else if (model.getValue() !== value) {
      applyingExternalValue = true;
      try {
        model.setValue(value);
        savedViewState = null;
      } finally {
        applyingExternalValue = false;
      }
    }
  },
);

watch(() => props.active, async active => {
  if (!editor) return;
  if (!active) {
    savedViewState = editor.saveViewState();
    return;
  }
  await nextTick();
  if (!editor || !props.active) return;
  editor.layout();
  if (savedViewState) editor.restoreViewState(savedViewState);
  emitCursor();
});

watch(() => props.theme, theme => {
  editor?.updateOptions({ theme: theme === 'light' ? 'vs' : 'vs-dark' });
});

watch(() => props.filename, () => {
  editor?.updateOptions({ ariaLabel: props.filename });
});

onMounted(() => {
  model = createModel(props.modelValue);
  editor = monaco.editor.create(container.value, {
    model,
    theme: props.theme === 'light' ? 'vs' : 'vs-dark',
    ariaLabel: props.filename,
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
    guides: { indentation: true },
    wordWrap: 'off',
    cursorStyle: 'line',
    cursorWidth: 2,
    cursorBlinking: 'blink',
    mouseWheelZoom: false,
    letterSpacing: 0,
    lineHeight: 20,
    renderValidationDecorations: 'off',
    // No GLSL semantic provider is registered; avoid pending symbol work on tab disposal.
    occurrencesHighlight: 'off',
    'semanticHighlighting.enabled': false,
  });
  listeners.push(
    editor.onDidChangeModelContent(() => {
      if (!applyingExternalValue) emit('update:modelValue', model.getValue());
    }),
    editor.onDidFocusEditorWidget(() => { emit('focus'); emitCursor(); }),
    editor.onDidChangeCursorPosition(emitCursor),
  );
  emitCursor();
});

onBeforeUnmount(() => {
  for (const listener of listeners) listener.dispose();
  editor?.dispose();
  model?.dispose();
  editor = null;
  model = null;
  savedViewState = null;
});

defineExpose({ focus, layout, runAction });
</script>

<template>
  <div ref="container" class="code-editor"></div>
</template>

<style scoped>
.code-editor {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
</style>
