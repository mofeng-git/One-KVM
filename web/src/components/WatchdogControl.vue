<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Loader2 } from 'lucide-vue-next'
import { watchdogConfigApi } from '@/api'
import type { WatchdogConfigResponse } from '@/types/generated'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
const props = defineProps<{ active: boolean }>()
const { t } = useI18n()
const watchdogStatus = ref<WatchdogConfigResponse | null>(null)
const watchdogLoading = ref(false)
const watchdogError = ref('')

const watchdogStatusKey = computed(() => {
  const status = watchdogStatus.value
  if (!status) return 'closed'
  if (!status.supported) return status.enabled ? 'error' : 'unsupported'
  if (status.running) return 'running'
  if (status.enabled) return 'error'
  return 'closed'
})

const watchdogDisplayReason = computed(() => {
  const reason = watchdogStatus.value?.reason
  if (!reason) return ''
  if (reason.includes('No hardware watchdog device found'))
    return t('settings.watchdog.unsupportedReason')
  if (reason.includes('discover')) return t('settings.watchdog.discoveryFailed')
  if (reason.includes('open a hardware watchdog')) return t('settings.watchdog.openFailed')
  if (reason.includes('keepalive')) return t('settings.watchdog.feedFailed')
  if (reason.includes('safely disabled') || reason.includes('nowayout'))
    return t('settings.watchdog.disableFailed')
  return t('settings.watchdog.abnormalReason')
})

function watchdogRequestError(error: unknown, action: 'enable' | 'disable'): string {
  const message = error instanceof Error ? error.message : ''
  if (message.includes('No hardware watchdog device found'))
    return t('settings.watchdog.unsupportedReason')
  if (message.includes('cannot be safely disabled') || message.includes('nowayout')) {
    return t('settings.watchdog.disableFailed')
  }
  return t(
    action === 'enable' ? 'settings.watchdog.enableFailed' : 'settings.watchdog.toggleFailed',
  )
}

async function loadWatchdogConfig() {
  watchdogLoading.value = true
  watchdogError.value = ''
  try {
    watchdogStatus.value = await watchdogConfigApi.get()
  } catch (error) {
    watchdogError.value = error instanceof Error ? error.message : t('settings.watchdog.loadFailed')
  } finally {
    watchdogLoading.value = false
  }
}

async function updateWatchdog(enabled: boolean) {
  if (!watchdogStatus.value || watchdogLoading.value) return
  watchdogLoading.value = true
  watchdogError.value = ''
  try {
    watchdogStatus.value = await watchdogConfigApi.update({ enabled })
  } catch (error) {
    watchdogError.value = watchdogRequestError(error, enabled ? 'enable' : 'disable')
    try {
      watchdogStatus.value = await watchdogConfigApi.get()
    } catch {
      // Preserve the last confirmed state when status refresh also fails.
    }
  } finally {
    watchdogLoading.value = false
  }
}

watch(() => props.active, (active) => { if (active) void loadWatchdogConfig() }, { immediate: true })
</script>

<template>
<div class="settings-quick-control" data-quick-control="watchdog">
  <Label for="watchdog-enabled">{{ t('settings.watchdog.title') }}</Label>
  <div class="flex min-w-0 items-center gap-2">
    <span v-if="watchdogStatus || watchdogError" class="max-w-40 truncate text-xs"
      :class="watchdogError || watchdogStatusKey === 'error' ? 'text-destructive' : 'text-muted-foreground'"
      :title="watchdogError || watchdogDisplayReason" role="status">
      {{ watchdogError || t(`settings.watchdog.status.${watchdogStatusKey}`) }}
    </span>
    <Loader2 v-if="watchdogLoading" class="size-3.5 animate-spin text-muted-foreground" />
    <Switch id="watchdog-enabled" :model-value="watchdogStatus?.enabled ?? false"
      :disabled="watchdogLoading || !watchdogStatus || (!watchdogStatus.supported && !watchdogStatus.enabled)"
      @update:model-value="updateWatchdog" />
  </div>
</div>

</template>
