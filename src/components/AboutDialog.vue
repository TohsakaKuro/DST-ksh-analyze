<script setup>
import { openUrl } from '@tauri-apps/plugin-opener';
import { isTauri } from '@tauri-apps/api/core';
import { ref } from 'vue';
import AppDialog from './AppDialog.vue';
import { version } from '../../package.json';
import appIcon from '../../src-tauri/icons/64x64.png';
defineEmits(['close']);
const error = ref('');
async function open(path = '') {
  const url = `https://github.com/TohsakaKuro/DST-ksh-analyze${path}`;
  try {
    if (isTauri()) await openUrl(url);
    else window.open(url, '_blank', 'noopener,noreferrer');
  } catch (failure) { error.value = String(failure.message || failure); }
}
</script>

<template>
  <AppDialog title="关于" @cancel="$emit('close')">
    <div class="about-product"><img :src="appIcon" alt="" /><div><strong>DST KSH Analyze</strong><p>{{ version }}</p></div></div>
    <div class="about-links"><fluent-button appearance="subtle" @click="open()">GitHub</fluent-button><fluent-button appearance="subtle" @click="open('/issues')">问题反馈</fluent-button></div>
    <p v-if="error" role="alert" class="form-error">{{ error }}</p>
    <template #actions><fluent-button appearance="primary" autofocus @click="$emit('close')">关闭</fluent-button></template>
  </AppDialog>
</template>
