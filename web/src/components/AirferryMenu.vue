<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, Download, File, LoaderCircle, Play, QrCode, RefreshCw, Square, Trash2 } from 'lucide-vue-next'
import { pluginsApi, type Plugin } from '@/api/plugins'
import { usePluginStore } from '@/stores/plugins'
import { airferryEn, airferryZh } from '@/i18n/airferry'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'

interface ProgressState {
  file: string
  received_symbols: number
  total_symbols: number
  bytes_per_second: number
}

interface SavedFile {
  filename: string
  receive_seconds: number
  bytes_per_second: number
}

interface ReceivedFile {
  name: string
  display_name: string
  size: number
}

const props = defineProps<{ plugin: Plugin }>()
const { t } = useI18n({
  useScope: 'local',
  messages: {
    'zh-CN': { airferry: airferryZh },
    'en-US': { airferry: airferryEn },
  },
})
const store = usePluginStore()
const id = computed(() => props.plugin.manifest.id)
const removedFiles = ref(new Set<string>())
const status = computed(() => props.plugin.runtime)
const fields = computed(() => status.value.fields)
const running = computed(() => fields.value.receiving === true)
const capture = computed(() => fields.value.capture as string | undefined)
const eligible = computed(() => capture.value === 'ready')
const event = computed(() => fields.value.event as { state: string; reason?: string; progress?: ProgressState } | null)
const receivedFiles = computed(() => (fields.value.files ?? []) as ReceivedFile[])
const files = computed(() => receivedFiles.value.filter(file => !removedFiles.value.has(file.name)))
watch(receivedFiles, current => {
  for (const name of removedFiles.value) {
    if (!current.some(file => file.name === name)) removedFiles.value.delete(name)
  }
})
const busy = ref(false)
const loading = ref(false)
const error = ref('')
const deleting = ref<string | null>(null)
const mutating = computed(() => busy.value || deleting.value !== null)
const progress = computed(() => running.value ? event.value?.progress : null)
const percent = computed(() => {
  const value = progress.value
  return value?.total_symbols ? Math.min(99, Math.round(value.received_symbols / value.total_symbols * 100)) : 0
})
const phase = computed(() => {
  if (status.value.error || event.value?.state === 'error') return 'error'
  if (status.value.phase === 'starting' || (props.plugin.enabled && !capture.value)) return 'loading'
  if (running.value) return capture.value && !eligible.value ? capture.value : progress.value ? 'receiving' : 'waiting'
  if (completed.value) return 'completed'
  return capture.value && !eligible.value ? capture.value : 'idle'
})
const speed = computed(() => {
  const bytes = progress.value?.bytes_per_second || 0
  return `${(bytes / 1000).toFixed(1)} KB/s`
})
const completed = computed(() => {
  const saved = fields.value.last_saved
  return !running.value && saved && typeof saved === 'object' ? saved as SavedFile : null
})

async function removeFile(name: string) {
  if (mutating.value) return
  deleting.value = name
  error.value = ''
  try {
    await pluginsApi.deleteFile(id.value, name)
    removedFiles.value.add(name)
    await store.refresh()
  } catch (e) { error.value = e instanceof Error ? e.message : t('airferry.deleteFailed') }
  finally { deleting.value = null }
}

function size(bytes: number) {
  return bytes < 1024 ? `${bytes} B` : bytes < 1048576 ? `${(bytes / 1024).toFixed(1)} KiB` : `${(bytes / 1048576).toFixed(1)} MiB`
}
async function refresh() {
  if (loading.value || mutating.value) return
  loading.value = true
  error.value = ''
  try {
    await store.refresh()
  } catch (e) {
    error.value = e instanceof Error ? e.message : t('airferry.loadFailed')
  } finally { loading.value = false }
}

async function toggle() {
  if (mutating.value) return
  busy.value = true
  error.value = ''
  try {
    const shouldReceive = !running.value
    if (!props.plugin.enabled) {
      store.upsert(await pluginsApi.configure(id.value, { enabled: true }))
      for (let attempt = 0; attempt < 20; attempt++) {
        await new Promise(resolve => setTimeout(resolve, 500))
        await store.refresh()
        if (store.installed.find(plugin => plugin.manifest.id === id.value)?.runtime.phase === 'running') break
      }
    }
    await pluginsApi.action(id.value, shouldReceive ? 'start_receive' : 'stop_receive')
    // The host publishes the new receiver status on its next health poll.
    for (let attempt = 0; attempt < 6; attempt++) {
      await new Promise(resolve => setTimeout(resolve, 400))
      await store.refresh()
      const fields = store.installed.find(plugin => plugin.manifest.id === id.value)?.runtime.fields
      if (fields?.receiving === shouldReceive) break
    }
  } catch (e) { error.value = e instanceof Error ? e.message : t('airferry.actionFailed') }
  finally { busy.value = false }
}
onMounted(refresh)
</script>

<template>
  <div class="flex max-h-[min(680px,80dvh)] flex-col overflow-hidden" @keydown.stop @keyup.stop @pointerdown.stop @wheel.stop>
    <div class="flex shrink-0 items-center justify-between gap-3 border-b px-4 py-3">
      <div class="flex items-center gap-2"><QrCode class="size-4 text-muted-foreground" /><h2 class="text-sm font-semibold">{{ t('airferry.title') }}</h2></div>
      <Button variant="ghost" size="icon-sm" :disabled="loading || mutating" :aria-label="t('airferry.refresh')" :title="t('airferry.refresh')" @click="refresh"><RefreshCw class="size-3.5" :class="{ 'animate-spin': loading }" /></Button>
    </div>
    <div class="min-h-0 space-y-4 overflow-y-auto p-4">
      <div class="space-y-3 rounded-lg border p-3">
        <div class="flex items-center gap-2 text-sm font-medium" aria-live="polite">
          <Check v-if="completed" class="size-4 shrink-0 text-emerald-600" />
          <span v-else class="size-2 shrink-0 rounded-full" :class="running && eligible ? 'bg-emerald-600' : 'bg-muted-foreground/50'" />
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
        <Button class="w-full gap-2" :variant="running ? 'outline' : 'default'" :disabled="mutating || (plugin.enabled && !running && !eligible)" @click="toggle">
          <LoaderCircle v-if="busy" class="size-4 animate-spin" /><Square v-else-if="running" class="size-3.5" /><Play v-else class="size-4" />
          {{ t(running ? 'airferry.stop' : 'airferry.start') }}
        </Button>
      </div>
      <p v-if="error || status.error || (event?.state === 'error' && event.reason)" role="alert" class="text-sm leading-relaxed text-destructive">{{ error || status.error || event?.reason }}</p>
      <div class="space-y-2">
        <div class="flex items-center justify-between"><h3 class="text-sm font-medium">{{ t('airferry.files') }}</h3><span class="text-xs tabular-nums text-muted-foreground">{{ files.length }}</span></div>
        <div v-if="files.length" class="divide-y">
          <div v-for="file in files" :key="file.name" class="flex min-w-0 items-center gap-3 py-3">
            <File class="size-4 shrink-0 text-muted-foreground" />
            <div class="min-w-0 flex-1"><p class="truncate text-sm" :title="file.display_name">{{ file.display_name }}</p><p class="mt-0.5 text-xs tabular-nums text-muted-foreground">{{ size(file.size) }}</p></div>
            <Button as-child variant="ghost" size="icon-sm" class="shrink-0 text-muted-foreground"><a :href="pluginsApi.downloadUrl(id, file.name)" download :aria-label="t('airferry.downloadNamed', { name: file.display_name })" :title="t('airferry.download')"><Download class="size-3.5" /></a></Button>
            <Button variant="ghost" size="icon-sm" class="shrink-0 text-muted-foreground hover:text-destructive" :aria-label="t('airferry.deleteNamed', { name: file.display_name })" :title="t('airferry.delete')" :disabled="mutating" @click="removeFile(file.name)"><LoaderCircle v-if="deleting === file.name" class="size-3.5 animate-spin" /><Trash2 v-else class="size-3.5" /></Button>
          </div>
        </div>
        <div v-else class="flex items-center gap-3 rounded-lg bg-muted/40 px-3 py-4"><Download class="size-5 shrink-0 text-muted-foreground" /><p class="text-sm leading-relaxed text-muted-foreground">{{ t('airferry.empty') }}</p></div>
      </div>
    </div>
  </div>
</template>
