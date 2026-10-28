<script setup lang="ts">
import SettingsHelp from './SettingsHelp.vue'
defineProps<{ help?: string }>()
</script>
<template>
  <h2 data-slot="card-title" class="settings-card-title">
    <slot />
    <SettingsHelp v-if="help" :content="help" />
  </h2>
</template>
