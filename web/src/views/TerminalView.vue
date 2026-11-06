<script setup lang="ts">
import { computed, defineAsyncComponent } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSystemStore } from '@/stores/system'

const EmbeddedTerminal = defineAsyncComponent(
  () => import('@/components/EmbeddedTerminal.vue'),
)

const { t } = useI18n()
const systemStore = useSystemStore()
const useBuiltinTerminal = computed(() => systemStore.platform?.mode !== 'windows')
</script>

<template>
  <div class="h-dvh w-full overflow-hidden bg-[#0b0e14]">
    <EmbeddedTerminal v-if="useBuiltinTerminal" />
    <iframe
      v-else
      src="/api/terminal/"
      class="block h-full w-full border-0"
      allow="clipboard-read; clipboard-write"
      :title="t('extensions.ttyd.title')"
    />
  </div>
</template>
