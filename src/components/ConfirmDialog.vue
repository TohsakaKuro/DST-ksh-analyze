<template>
  <Transition name="dialog">
    <div v-if="show" class="confirm-dialog-overlay" @click="handleCancel">
      <div class="confirm-dialog" @click.stop>
        <div class="confirm-content">
          <div class="dialog-header">
            <div class="warning-icon">⚠</div>
            <h3>{{ title }}</h3>
          </div>
          <div class="message-content">
            <p class="sub-message">{{ message }}</p>
          </div>
        </div>
        <div class="confirm-actions">
          <button class="confirm-button" @click="handleSave">{{ saveLabel }}</button>
          <button class="confirm-button" @click="handleDiscard">{{ discardLabel }}</button>
          <button class="confirm-button" @click="handleCancel">{{ cancelLabel }}</button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { defineProps, defineEmits, onMounted, onUnmounted } from 'vue';

const props = defineProps({
  show: Boolean,
  title: String,
  message: String,
  file: String,
  saveLabel: {
    type: String,
    default: '保存(S)'
  },
  discardLabel: {
    type: String,
    default: '不保存(N)'
  },
  cancelLabel: {
    type: String,
    default: '取消(C)'
  }
});

const emit = defineEmits(['save', 'discard', 'cancel']);

const handleSave = () => {
  emit('save');
};

const handleDiscard = () => {
  emit('discard');
};

const handleCancel = () => {
  emit('cancel');
};

// 添加键盘事件处理
const handleKeyDown = (event) => {
  if (!props.show) return;
  
  switch (event.key.toLowerCase()) {
    case 's':
      event.preventDefault();
      handleSave();
      break;
    case 'n':
      event.preventDefault();
      handleDiscard();
      break;
    case 'c':
    case 'escape':
      event.preventDefault();
      handleCancel();
      break;
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<style scoped>
.confirm-dialog-overlay {
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

.confirm-dialog {
  background: var(--window-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  width: 400px;
  max-width: 90%;
  margin-bottom: 20vh;
}

.confirm-content {
  padding: 16px;
}

.dialog-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.warning-icon {
  color: #fc0;
  font-size: 24px;
}

.confirm-content h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.message-content {
  color: var(--text-secondary);
}

.message-content p {
  margin: 8px 0;
  line-height: 1.4;
}

.sub-message {
  color: var(--text-secondary);
  opacity: 0.8;
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-color);
}

.confirm-button {
  padding: 6px 12px;
  border-radius: 4px;
  border: 1px solid var(--border-color);
  background: var(--button-bg, #2d2d2d);
  color: var(--text-primary);
  cursor: pointer;
  font-size: 13px;
  transition: background-color 0.2s;
}

.confirm-button:hover {
  background: var(--button-hover);
}

/* 打开动画 */
.dialog-enter-active,
.dialog-leave-active {
  transition: opacity 0.15s ease;
}

.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}

/* 移除其他动画相关的样式 */
</style>