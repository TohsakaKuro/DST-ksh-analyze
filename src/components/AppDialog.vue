<script setup>
import { ref, onMounted, onBeforeUnmount, nextTick } from 'vue';
const props = defineProps({ title: String, dismissible: { type: Boolean, default: true } });
const emit = defineEmits(['cancel']);
const element = ref(null);
let nativeDialog;
let returnFocus;
let unmounting = false;
let cancelEmitted = false;
function preventDismiss(event) {
  if (!props.dismissible) {
    event.preventDefault();
    event.stopImmediatePropagation();
  }
}
function backdropClick(event) {
  if (event.target === nativeDialog) preventDismiss(event);
}
function toggle(event) {
  if (!unmounting && !cancelEmitted && event.detail?.newState === 'closed') {
    cancelEmitted = true;
    emit('cancel');
  }
}
onMounted(async () => {
  await nextTick();
  if (unmounting || !element.value) return;
  returnFocus = document.activeElement;
  nativeDialog = element.value.dialog;
  nativeDialog.addEventListener('cancel', preventDismiss, true);
  nativeDialog.addEventListener('click', backdropClick, true);
  element.value.show();
});
onBeforeUnmount(() => {
  unmounting = true;
  nativeDialog?.removeEventListener('cancel', preventDismiss, true);
  nativeDialog?.removeEventListener('click', backdropClick, true);
  if (nativeDialog?.open) nativeDialog.close();
  if (returnFocus?.isConnected) returnFocus.focus({ preventScroll: true });
});
</script>

<template>
  <fluent-dialog ref="element" type="modal" :aria-label="title" @toggle="toggle">
    <fluent-dialog-body>
      <h2 slot="title">{{ title }}</h2>
      <slot />
      <div slot="action" class="dialog-actions"><slot name="actions" /></div>
    </fluent-dialog-body>
  </fluent-dialog>
</template>
