<script setup>
defineOptions({ inheritAttrs: false });
defineProps({ label: String, icon: [Object, Function], disabled: Boolean, selected: { type: Boolean, default: undefined } });
</script>

<template>
  <fluent-button v-bind="$attrs" :title="label" :aria-label="label" :disabled-focusable.camel.prop="disabled"
    :aria-pressed="selected === undefined ? undefined : selected" icon-only
    :appearance="selected ? 'secondary' : 'subtle'" size="small">
    <component :is="icon" aria-hidden="true" :size="16" />
  </fluent-button>
</template>
