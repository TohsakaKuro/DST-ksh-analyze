<script setup>
import { computed, reactive } from 'vue';
import AppDialog from './AppDialog.vue';

const props = defineProps({ model: { type: Object, required: true } });
const emit = defineEmits(['resolve']);
function initial(stage) {
  const candidates = props.model.candidates?.[stage] || [];
  const previous = props.model.previous?.[`${stage}Id`];
  if (candidates.some(document => document.id === previous)) return previous;
  const specific = candidates.filter(document => document.hint === stage);
  return specific.length === 1 ? specific[0].id : candidates.length === 1 ? candidates[0].id : null;
}
const selected = reactive({ vs: initial('vs'), ps: initial('ps') });
if (selected.ps === selected.vs) selected.ps = null;
const valid = computed(() => selected.vs != null && selected.ps != null && selected.vs !== selected.ps);
const title = computed(() => props.model.title || ({ export: '导出 KSH', unsaved: '保存更改？', overwrite: '替换现有文件？', metadata: '重新组合源码？' })[props.model.kind]);
function submit() { if (valid.value) emit('resolve', { vsId: selected.vs, psId: selected.ps }); }
</script>

<template>
  <AppDialog :title="title" @cancel="emit('resolve', null)">
    <form v-if="model.kind === 'export'" id="export-selection" class="export-selection" @submit.prevent="submit">
      <fieldset v-for="stage in ['vs', 'ps']" :key="stage" class="source-selection">
        <legend>{{ stage.toUpperCase() }}</legend>
        <div class="source-options">
          <label v-for="document in model.candidates[stage]" :key="document.id" class="source-option" :title="document.label"
            :class="{ 'option-selected': selected[stage] === document.id, 'option-disabled': selected[stage === 'vs' ? 'ps' : 'vs'] === document.id }">
            <input v-model="selected[stage]" type="radio" :name="stage" :value="document.id"
              :disabled="selected[stage === 'vs' ? 'ps' : 'vs'] === document.id" :aria-label="document.name" />
            <span class="source-option-text"><span>{{ document.name }}</span><small v-if="document.label !== document.name">{{ document.label }}</small></span>
          </label>
          <p v-if="!model.candidates[stage].length" class="no-candidates">无可选文件</p>
        </div>
      </fieldset>
    </form>
    <ul v-else-if="model.kind === 'unsaved'" class="file-list"><li v-for="name in model.names" :key="name">{{ name }}</li></ul>
    <ul v-else-if="model.kind === 'overwrite'" class="file-list"><li v-for="path in model.paths" :key="path">{{ path }}</li></ul>
    <p v-else-if="model.kind === 'error'" class="export-error">{{ model.message }}</p>
    <p v-else class="metadata-warning">所选源码不是同一 KSH 的原始 VS/PS 组合。导出将根据当前源码重建参数表，不沿用原 KSH 的默认值和额外元数据。</p>
    <template #actions>
      <template v-if="model.kind === 'unsaved'">
        <fluent-button appearance="primary" autofocus @click="emit('resolve', 'save')">保存</fluent-button>
        <fluent-button @click="emit('resolve', 'discard')">不保存</fluent-button>
      </template>
      <fluent-button v-else-if="model.kind === 'overwrite'" appearance="primary" @click="emit('resolve', true)">替换</fluent-button>
      <fluent-button v-else-if="model.kind === 'metadata'" appearance="primary" @click="emit('resolve', true)">继续</fluent-button>
      <fluent-button v-else-if="model.kind === 'error'" appearance="primary" autofocus @click="emit('resolve', true)">确定</fluent-button>
      <fluent-button v-else appearance="primary" :disabled.prop="!valid" @click="submit">导出…</fluent-button>
      <fluent-button v-if="model.kind !== 'error'" @click="emit('resolve', null)">取消</fluent-button>
    </template>
  </AppDialog>
</template>
