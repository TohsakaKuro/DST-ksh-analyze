<template>
  <div v-if="show" class="dialog-overlay" @click="$emit('close')">
    <div class="dialog-content" @click.stop>
      <div class="dialog-header">
        <h2>关于 DST KSH 分析器</h2>
        <button class="icon-button" @click="$emit('close')">
          <span class="icon">✕</span>
        </button>
      </div>
      <div class="dialog-body">
        <p>版本：{{ version }}</p>
        <p>一个用于分析和编辑 DST KSH 着色器文件的工具。</p>
        <div class="feature-list">
          <h3>主要功能：</h3>
          <ul>
            <li>KSH 文件导入导出</li>
          </ul>
        </div>
        <div class="links">
          <h3>相关链接：</h3>
          <div class="link-item">
            <span class="link-title">GitHub 仓库：</span>
            <a class="link-url" @click="openExternal(repository)">
              {{ repository }}
            </a>
          </div>
          <div class="link-item">
            <span class="link-title">问题反馈：</span>
            <a class="link-url" @click="openExternal(issues)">
              {{ issues }}
            </a>
          </div>
          <p>欢迎贡献</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { openUrl } from '@tauri-apps/plugin-opener';

const props = defineProps({
  show: {
    type: Boolean,
    required: true
  }
});

defineEmits(['close']);

const version = ref('');
const repository = ref('');
const issues = ref('');

const openExternal = async (url) => {
  //return
  try {
    await openUrl(url);
  } catch (error) {
    console.error('打开链接失败:', error);
  }
};

onMounted(async () => {
  try {
    const response = await fetch('/package.json');
    const data = await response.json();
    version.value = data.version;
    repository.value = data.repository?.url || 'https://github.com/TohsakaKuro/DST-ksh-analyze';
    issues.value = data.bugs?.url || 'https://github.com/TohsakaKuro/DST-ksh-analyze/issues';
  } catch (error) {
    console.error('无法读取版本信息:', error);
    version.value = '未知';
    repository.value = 'https://github.com/TohsakaKuro/DST-ksh-analyze';
    issues.value = 'https://github.com/TohsakaKuro/DST-ksh-analyze/issues';
  }
});
</script>

<style>
.dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog-content {
  background: var(--window-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 24px;
  min-width: 400px;
  max-width: 600px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.dialog-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.dialog-header h2 {
  font-size: 24px;
  font-weight: 500;
  color: var(--text-primary);
}

.dialog-body {
  color: var(--text-secondary);
  font-size: 14px;
  line-height: 1.6;
}

.dialog-body p {
  margin-bottom: 16px;
}

.feature-list {
  margin: 16px 0;
}

.feature-list h3 {
  color: var(--text-primary);
  margin-bottom: 8px;
}

.feature-list ul {
  list-style-type: none;
  padding-left: 0;
}

.feature-list li {
  margin-bottom: 4px;
  padding-left: 20px;
  position: relative;
}

.feature-list li::before {
  content: "•";
  position: absolute;
  left: 0;
  color: var(--accent-color);
}

.links {
  margin-top: 24px;
}

.links h3 {
  color: var(--text-primary);
  margin-bottom: 8px;
}

.link-item {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
  font-size: 13px;
}

.link-title {
  color: var(--text-secondary);
  margin-right: 8px;
  white-space: nowrap;
}

.link-url {
  color: var(--accent-color);
  text-decoration: none;
  word-break: break-all;
  cursor: pointer;
}

.link-url:hover {
  text-decoration: underline;
}

.icon-button {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 3px;
  transition: all 0.2s ease;
}

.icon-button:hover {
  background: var(--button-hover);
}

.icon-button .icon {
  font-size: 14px;
  width: 14px;
  height: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style> 