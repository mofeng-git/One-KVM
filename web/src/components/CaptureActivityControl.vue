<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Loader2 } from 'lucide-vue-next'
import { useConfigStore } from '@/stores/config'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'

const props = defineProps<{ active: boolean }>()
const { t } = useI18n()
const configStore = useConfigStore()
const loaded = ref(false)
const saving = ref(false)
const enabled = computed(() => configStore.stream?.keep_capture_alive ?? false)
watch(() => props.active, async (active) => {
  if (!active || loaded.value) return
  try {
    await configStore.ensureStream()
    loaded.value = true
  } catch { /* The request layer displays loading failures. */ }
}, { immediate: true })
async function update(enabled: boolean) {
  if (saving.value || !loaded.value) return
  saving.value = true
  try {
    await configStore.updateStream({ keep_capture_alive: enabled })
  } catch (error) {
    toast.error(t('settingsUi.saveFailed'), {
      description: error instanceof Error ? error.message : undefined,
    })
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="settings-quick-control" data-quick-control="capture">
    <Label for="keep-capture-alive">{{ t('settings.captureActivity') }}</Label>
    <div class="flex items-center gap-2">
      <Loader2 v-if="saving" class="size-3.5 animate-spin" />
      <Switch id="keep-capture-alive" :model-value="enabled"
        :aria-label="t('settings.keepCaptureAlive')" :disabled="saving || !loaded"
        @update:model-value="update" />
    </div>
  </div>
</template>
