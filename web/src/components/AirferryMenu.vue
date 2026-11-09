<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, Download, File, LoaderCircle, Play, QrCode, RefreshCw, Square, Trash2 } from 'lucide-vue-next'
import { airferryApi, type AirferryStatus } from '@/api'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Progress } from '@/components/ui/progress'

const props = defineProps<{ side?: 'right' | 'bottom'; compact?: boolean }>()
const { t } = useI18n()
const open = ref(false)
const status = ref<AirferryStatus | null>(null)
const busy = ref(false)
const loading = ref(false)
const error = ref('')
const deleting = ref<string | null>(null)
const mutating = computed(() => busy.value || deleting.value !== null)
let requestVersion = 0
let polling: ReturnType<typeof setInterval> | undefined
const progress = computed(() => status.value?.event?.progress)
const percent = computed(() => {
  const value = progress.value
  return value?.total_symbols ? Math.min(99, Math.round(value.received_symbols / value.total_symbols * 100)) : 0
})
const phase = computed(() => {
  if (!status.value) return 'loading'
  if (!status.value.eligible) return status.value.reason
  if (status.value.event?.state === 'error') return 'error'
  if (!status.value.running) return status.value.last_saved ? 'completed' : 'idle'
  return progress.value ? 'receiving' : 'waiting'
})
const speed = computed(() => {
  const bytes = progress.value?.bytes_per_second || 0
  return `${(bytes / 1000).toFixed(1)} KB/s`
})
const completed = computed(() => !status.value?.running ? status.value?.last_saved : null)

async function removeFile(name: string) {
  if (mutating.value) return
  requestVersion++
  deleting.value = name
  error.value = ''
  try {
    await airferryApi.deleteFile(name)
    if (status.value) status.value.files = status.value.files.filter(file => file.name !== name)
  } catch (e) { error.value = e instanceof Error ? e.message : t('airferry.deleteFailed') }
  finally { deleting.value = null }
}

function size(bytes: number) {
  return bytes < 1024 ? `${bytes} B` : bytes < 1048576 ? `${(bytes / 1024).toFixed(1)} KiB` : `${(bytes / 1048576).toFixed(1)} MiB`
}
async function refresh() {
  if (loading.value || mutating.value) return
  const version = requestVersion
  loading.value = true
  try {
    const result = await airferryApi.status()
    if (version === requestVersion) { status.value = result; error.value = '' }
  } catch (e) {
    if (version === requestVersion) error.value = e instanceof Error ? e.message : t('airferry.loadFailed')
  }
  finally { loading.value = false }
}
async function toggle() {
  if (mutating.value) return
  requestVersion++
  busy.value = true
  error.value = ''
  try { status.value = await (status.value?.running ? airferryApi.stop() : airferryApi.start()) }
  catch (e) { error.value = e instanceof Error ? e.message : t('airferry.actionFailed') }
  finally { busy.value = false }
}
watch(open, (value) => {
  clearInterval(polling)
  if (value) { void refresh(); polling = setInterval(() => void refresh(), 1000) }
})
onUnmounted(() => { requestVersion++; clearInterval(polling) })
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" :aria-label="t('airferry.title')" :title="t('airferry.title')" data-testid="airferry-menu">
        <span class="relative"><QrCode class="size-4" /><span v-if="status?.running" class="absolute -right-1 -top-0.5 size-1.5 rounded-full bg-emerald-600" /></span>
        <span v-if="!props.compact" class="hidden sm:inline">{{ t('airferry.title') }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent :side="props.side || 'bottom'" align="start" class="flex max-h-[min(680px,80dvh)] w-[min(400px,calc(100vw-2rem))] flex-col overflow-hidden p-0" @keydown.stop @keyup.stop @pointerdown.stop @wheel.stop>
      <div class="flex shrink-0 items-center justify-between gap-3 border-b px-4 py-3">
        <div class="flex items-center gap-2"><QrCode class="size-4 text-muted-foreground" /><h2 class="text-sm font-semibold">{{ t('airferry.title') }}</h2></div>
        <Button variant="ghost" size="icon-sm" :disabled="loading || mutating" :aria-label="t('airferry.refresh')" :title="t('airferry.refresh')" @click="refresh"><RefreshCw class="size-3.5" :class="{ 'animate-spin': loading }" /></Button>
      </div>
      <div class="min-h-0 space-y-4 overflow-y-auto p-4">
        <p class="text-sm leading-relaxed text-muted-foreground">{{ t('airferry.instructions') }}</p>
        <div class="space-y-3 rounded-lg border p-3">
          <div class="flex items-center gap-2 text-sm font-medium" aria-live="polite">
            <Check v-if="completed" class="size-4 shrink-0 text-emerald-600" />
            <span v-else class="size-2 shrink-0 rounded-full" :class="status?.running && status?.eligible ? 'bg-emerald-600' : 'bg-muted-foreground/50'" />
            {{ t(`airferry.phases.${phase}`) }}
          </div>
          <template v-if="progress">
            <p class="truncate text-sm" :title="progress.file">{{ progress.file || t('airferry.identifying') }}</p>
            <Progress :model-value="percent" class="h-1.5" :aria-label="t('airferry.progress')" />
            <div class="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-xs tabular-nums text-muted-foreground">
              <span>{{ progress.received_symbols }} / {{ progress.total_symbols }} {{ t('airferry.symbols') }} · {{ percent }}%</span>
              <span :title="t('airferry.speedHint')" data-testid="airferry-speed">≈ {{ speed }}</span>
            </div>
          </template>
          <div v-else-if="completed" class="space-y-1 text-sm" data-testid="airferry-completed">
            <p class="truncate" :title="completed.filename">{{ completed.filename }}</p>
            <p class="text-xs tabular-nums text-muted-foreground">{{ t('airferry.completedSummary', { seconds: completed.receive_seconds.toFixed(1), speed: (completed.bytes_per_second / 1000).toFixed(1) }) }}</p>
          </div>
          <Button class="w-full gap-2" :variant="status?.running ? 'outline' : 'default'" :disabled="mutating || !status || (!status.running && !status.eligible)" @click="toggle">
            <LoaderCircle v-if="busy" class="size-4 animate-spin" /><Square v-else-if="status?.running" class="size-3.5" /><Play v-else class="size-4" />
            {{ t(status?.running ? 'airferry.stop' : 'airferry.start') }}
          </Button>
        </div>
        <p v-if="error" role="alert" class="text-sm leading-relaxed text-destructive">{{ error }}</p>
        <div class="space-y-2">
          <div class="flex items-center justify-between"><h3 class="text-sm font-medium">{{ t('airferry.files') }}</h3><span class="text-xs tabular-nums text-muted-foreground">{{ status?.files.length || 0 }}</span></div>
          <div v-if="status?.files.length" class="divide-y">
            <div v-for="file in status.files" :key="file.name" class="flex min-w-0 items-center gap-3 py-3">
              <File class="size-4 shrink-0 text-muted-foreground" />
              <div class="min-w-0 flex-1"><p class="truncate text-sm" :title="file.display_name">{{ file.display_name }}</p><p class="mt-0.5 text-xs tabular-nums text-muted-foreground">{{ size(file.size) }}</p></div>
              <Button as-child variant="ghost" size="icon-sm" class="shrink-0 text-muted-foreground"><a :href="airferryApi.downloadUrl(file.name)" download :aria-label="t('airferry.downloadNamed', { name: file.display_name })" :title="t('airferry.download')"><Download class="size-3.5" /></a></Button>
              <Button variant="ghost" size="icon-sm" class="shrink-0 text-muted-foreground hover:text-destructive" :aria-label="t('airferry.deleteNamed', { name: file.display_name })" :title="t('airferry.delete')" :disabled="mutating" @click="removeFile(file.name)"><LoaderCircle v-if="deleting === file.name" class="size-3.5 animate-spin" /><Trash2 v-else class="size-3.5" /></Button>
            </div>
          </div>
          <div v-else class="flex items-center gap-3 rounded-lg bg-muted/40 px-3 py-4"><Download class="size-5 shrink-0 text-muted-foreground" /><p class="text-sm leading-relaxed text-muted-foreground">{{ t('airferry.empty') }}</p></div>
        </div>
      </div>
    </PopoverContent>
  </Popover>
</template>
