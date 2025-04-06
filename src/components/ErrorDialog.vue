<template>
  <div v-if="show" class="error-dialog-overlay">
    <div class="error-dialog">
      <div class="error-icon">⚠️ 错误</div>
      <div class="error-content">
        <p>{{ message }}</p>
      </div>
      <button class="error-close" @click="close">确定</button>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';

const props = defineProps({
  show: Boolean,
  message: String
});

const emit = defineEmits(['close']);

const close = () => {
  emit('close');
};
</script>

<style scoped>
.error-dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.error-dialog {
  background: var(--window-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  padding: 20px;
  min-width: 300px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.error-icon {
  font-size: 24px;
  text-align: center;
}

.error-content {
  text-align: center;
}

.error-content h3 {
  margin: 0 0 8px 0;
  color: var(--text-primary);
}

.error-content p {
  margin: 0;
  color: var(--text-secondary);
  word-break: break-word;
}

.error-close {
  background: var(--accent-color);
  color: white;
  border: none;
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.error-close:hover {
  background: var(--accent-hover);
}
</style> 