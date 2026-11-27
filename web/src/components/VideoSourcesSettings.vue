<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Plus, RefreshCw, Save, Trash2, ChevronUp, ChevronDown, Loader2 } from 'lucide-vue-next'
import { configApi, type VideoDevice } from '@/api'
import { useConfigStore } from '@/stores/config'
import type { VideoConfig, VideoSourceConfig } from '@/types/generated'
import { generateUUID } from '@/lib/utils'
import { matchesVideoDevice, preferredVideoPath } from '@/lib/video-device-identity'
import VideoSourceFields from '@/components/VideoSourceFields.vue'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Card, CardContent, CardHeader } from '@/components/ui/card'

const props = withDefaults(defineProps<{ active?: boolean }>(), { active: true })
const emit = defineEmits<{ saved: [] }>()
const { t } = useI18n()
const configStore = useConfigStore()
const draft = ref<VideoSourceConfig[]>([])
const primary = ref('')
const devices = ref<VideoDevice[]>([])
const busy = ref(false)
const error = ref('')
const ready = ref(false)
const valid = computed(() => draft.value.some(source => source.enabled && source.id === primary.value)
  && draft.value.every(source => source.device.trim() && source.device.toLowerCase() !== 'auto'))

function syncDraft(video: VideoConfig) {
  draft.value = video.sources?.length ? video.sources.map(source => ({ ...source })) : [{
    id: 'primary', label: t('multisource.primarySource'), enabled: true,
    device: video.device ?? '', format: video.format ?? 'MJPEG',
    width: video.width, height: video.height, fps: video.fps, quality: video.quality,
  }]
  primary.value = video.primary_source_id || draft.value.find(source => source.enabled)?.id || ''
}

async function load() {
  busy.value = true
  error.value = ''
  try {
    const [video, inventory] = await Promise.all([configStore.refreshVideo(), configApi.listDevices()])
    devices.value = inventory.video
    syncDraft(video)
    ready.value = true
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { busy.value = false }
}

function add() {
  const device = devices.value.find(item => !draft.value.some(source => matchesVideoDevice(item, source.device)))
  draft.value.push({ id: generateUUID(), label: `${t('multisource.source')} ${draft.value.length + 1}`,
    enabled: true, device: device ? preferredVideoPath(device) : '', format: 'MJPEG',
    width: 1920, height: 1080, fps: 30, quality: 80 })
}
function remove(id: string) {
  draft.value = draft.value.filter(source => source.id !== id)
  if (primary.value === id) primary.value = draft.value.find(source => source.enabled)?.id || ''
}
function enable(source: VideoSourceConfig, enabled: boolean) {
  source.enabled = enabled
  if (!enabled && primary.value === source.id) primary.value = draft.value.find(item => item.enabled)?.id || ''
  if (enabled && !primary.value) primary.value = source.id
}
function move(index: number, delta: number) {
  const next = index + delta
  if (next < 0 || next >= draft.value.length) return
  const source = draft.value.splice(index, 1)[0]!
  draft.value.splice(next, 0, source)
}
async function save() {
  if (!valid.value || busy.value) return
  const previousConfig = configStore.video
  busy.value = true
  error.value = ''
  try {
    await configStore.updateVideo({ sources: draft.value.map(source => ({ ...source })), primary_source_id: primary.value })
    toast.success(t('multisource.sourcesSaved'))
    emit('saved')
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
  finally {
    if (configStore.video && configStore.video !== previousConfig) syncDraft(configStore.video)
    busy.value = false
  }
}
onMounted(load)
</script>

<template>
  <Card>
    <CardHeader class="settings-card-heading">
      <CardTitle :help="t('multisource.sourcesHelp')">{{ t('multisource.videoSources') }}</CardTitle>
      <div class="settings-card-actions">
        <Button variant="ghost" size="icon-sm" :disabled="busy" :aria-label="t('common.refresh')" @click="load"><RefreshCw class="size-4" /></Button>
        <Button variant="outline" :disabled="busy || !ready" @click="add"><Plus class="size-4" />{{ t('multisource.addSource') }}</Button>
        <Button :disabled="busy || !ready || !valid" @click="save"><Loader2 v-if="busy" class="size-4 animate-spin" /><Save v-else class="size-4" />{{ t('common.save') }}</Button>
      </div>
    </CardHeader>
    <CardContent class="space-y-4">
      <p class="text-sm text-muted-foreground">{{ t('multisource.primaryHelp') }}</p>
      <p v-if="error" class="whitespace-pre-line break-words text-sm text-destructive" role="alert">{{ error }}</p>
      <fieldset v-for="(source, index) in draft" :key="source.id" :disabled="busy" class="space-y-3 rounded-lg border p-3">
        <legend class="px-1 text-sm font-medium">{{ source.label || `${t('multisource.source')} ${index + 1}` }}</legend>
        <div class="flex flex-wrap items-center gap-2">
          <Switch :id="`source-enabled-${source.id}`" :model-value="source.enabled" :aria-label="t('multisource.enabled')"
            @update:model-value="enable(source, $event)" />
          <label :for="`source-enabled-${source.id}`" class="text-xs">{{ t('multisource.enabled') }}</label>
          <label class="ml-2 flex items-center gap-1.5 text-xs">
            <input type="radio" name="primary-video-source" :value="source.id" v-model="primary" :disabled="!source.enabled" />
            {{ t('multisource.primarySource') }}
          </label>
          <div class="ml-auto flex gap-1">
            <Button variant="ghost" size="icon-sm" :disabled="index === 0" :aria-label="t('multisource.moveUp')" @click="move(index, -1)"><ChevronUp class="size-4" /></Button>
            <Button variant="ghost" size="icon-sm" :disabled="index === draft.length - 1" :aria-label="t('multisource.moveDown')" @click="move(index, 1)"><ChevronDown class="size-4" /></Button>
            <Button variant="ghost" size="icon-sm" :disabled="draft.length === 1" :aria-label="t('multisource.removeSource')" @click="remove(source.id)"><Trash2 class="size-4" /></Button>
          </div>
        </div>
        <VideoSourceFields :model-value="source" :devices="devices" :active="props.active" :primary="source.id === primary"
          @update:model-value="draft[index] = $event" />
      </fieldset>
    </CardContent>
  </Card>
</template>
