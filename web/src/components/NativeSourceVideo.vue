<script setup lang="ts">
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ApiError } from '@/api/request'
import { videoSourcesApi, type VideoSource } from '@/api/videoSources'
import { expireVideoSourcesAuth } from '@/composables/useVideoSources'
import { useAuthStore } from '@/stores/auth'
import { generateUUID } from '@/lib/utils'
import { useElementSize } from '@vueuse/core'

const props = defineProps<{
  source: VideoSource
  primary: boolean
  primaryClientId: string
  primaryMjpeg: boolean
  primaryUrl: string
  rotation: number
  actual: boolean
  showStatus: boolean
  monitor: boolean
  frameSize: { width: number; height: number } | null
}>()
const emit = defineEmits<{ size: [width: number, height: number]; reconnect: [] }>()
const { t } = useI18n()
const picture = ref<HTMLDivElement | null>(null)
const { width: tileWidth, height: tileHeight } = useElementSize(picture)
const auth = useAuthStore()
const image = ref<HTMLImageElement | null>(null)
const url = ref('')
const error = ref('')
const live = ref(false)
const actualFps = ref(0)
const clientId = generateUUID()
const frameWidth = ref(props.source.width || 1920)
const frameHeight = ref(props.source.height || 1080)
let generation = 0
let running = false
let session = 0
let pollTimer: ReturnType<typeof setTimeout> | undefined
let retryTimer: ReturnType<typeof setTimeout> | undefined
let controller: AbortController | undefined
let frames: number | undefined
let lastAdvance = Date.now()
let opened = Date.now()
let retries = 0
let sampledAt = Date.now()

const contentStyle = computed(() => {
  const quarter = props.rotation % 180 !== 0
  const width = frameWidth.value
  const height = frameHeight.value
  const scale = props.actual ? 1 : Math.min(tileWidth.value / (quarter ? height : width), tileHeight.value / (quarter ? width : height))
  return {
    width: `${width * scale}px`,
    height: `${height * scale}px`,
    maxWidth: props.actual ? 'none' : undefined,
    transform: `translate(-50%, -50%) rotate(${props.rotation}deg)`,
  }
})
const pictureBoxStyle = computed(() => props.actual ? {
  width: `${props.rotation % 180 ? frameHeight.value : frameWidth.value}px`,
  height: `${props.rotation % 180 ? frameWidth.value : frameHeight.value}px`,
} : { width: '100%', height: '100%' })

function readSize() {
  if (!image.value?.naturalWidth || !image.value.naturalHeight) return
  frameWidth.value = image.value.naturalWidth
  frameHeight.value = image.value.naturalHeight
  emit('size', frameWidth.value, frameHeight.value)
}

function reopen() {
  if (!running || !auth.isAuthenticated) return
  retryTimer = undefined
  opened = lastAdvance = Date.now()
  frames = undefined
  live.value = false
  if (props.primary) emit('reconnect')
  else url.value = videoSourcesApi.streamUrl(props.source.id, ++generation, clientId)
}

function retry(reason: string) {
  error.value = reason
  live.value = false
  if (!running || retryTimer !== undefined || (props.primary && !props.primaryMjpeg)) return
  retryTimer = setTimeout(reopen, Math.min(1000 * 2 ** Math.min(retries++, 4), 15000))
}

async function poll(epoch = session) {
  if (!running || epoch !== session || !auth.isAuthenticated) return
  const current = new AbortController()
  controller = current
  const timeout = setTimeout(() => current.abort(), 5000)
  try {
    const status = await videoSourcesApi.status(props.source.id, current.signal,
      props.primary ? props.primaryClientId : clientId)
    if (!running || epoch !== session || current.signal.aborted) return
    readSize()
    const now = Date.now()
    const advanced = frames !== undefined && status.frames > frames
    if (!Number.isFinite(status.frames) || status.frames < 0) throw new Error(t('multisource.statusError'))
    if (advanced) {
      actualFps.value = (status.frames - frames!) * 1000 / Math.max(1, now - sampledAt)
      lastAdvance = now
    } else actualFps.value = 0
    sampledAt = now
    const mjpeg = !props.primary || props.primaryMjpeg
    const streamOpen = !props.primary || !!props.primaryUrl
    if (status.reason || !['ok', 'ready', 'streaming', 'running'].includes(status.state)) {
      live.value = false
      error.value = status.reason || status.state
      // Reconnect only this client, with backoff, while capture recovers.
      if (mjpeg && streamOpen && now - opened > 8000) retry(error.value)
    } else if (mjpeg && streamOpen && ((frames !== undefined && status.frames < frames)
      || (now - opened > 8000 && status.connected === false)
      || now - lastAdvance > 8000)) {
      retry(t('multisource.stale'))
    } else if (!mjpeg || (advanced && status.connected === true)) {
      // WebRTC reports encoded input frames; output demand can intentionally pause
      // encoding while the pipeline remains healthy. MJPEG reports capture frames.
      error.value = ''
      live.value = true
      retries = 0
      clearTimeout(retryTimer)
      retryTimer = undefined
    }
    frames = status.frames
  } catch (reason) {
    if (!running || epoch !== session) return
    if (reason instanceof ApiError && reason.status === 401) { expireVideoSourcesAuth(); stop(); return }
    retry(reason instanceof Error ? reason.message : t('multisource.statusError'))
  } finally {
    clearTimeout(timeout)
    if (running && epoch === session && auth.isAuthenticated) pollTimer = setTimeout(() => poll(epoch), 2000)
  }
}

function start() {
  if (running || !auth.isAuthenticated) return
  running = true
  session++
  opened = lastAdvance = sampledAt = Date.now()
  frames = undefined
  if (!props.primary) reopen()
  if (props.monitor) void poll()
}
function stop() {
  running = false
  session++
  clearTimeout(pollTimer)
  clearTimeout(retryTimer)
  retryTimer = undefined
  controller?.abort()
  url.value = ''
}
watch(() => [props.source.device, props.source.format, props.source.width, props.source.height,
  props.source.fps, props.source.quality, props.primary, props.primaryMjpeg, props.monitor], () => { stop(); start() })
watch(() => props.frameSize, size => {
  if (!size) return
  frameWidth.value = size.width
  frameHeight.value = size.height
}, { immediate: true })
watch(() => props.primaryUrl, () => {
  if (props.primary) { opened = lastAdvance = Date.now(); frames = undefined; live.value = false }
})
watch(() => auth.isAuthenticated, value => { if (value) start(); else stop() })
onMounted(start)
onActivated(start)
onDeactivated(stop)
onUnmounted(stop)
</script>

<template>
  <slot v-if="primary" />
  <div v-else ref="picture" class="source-picture">
    <div class="source-picture-box" :style="pictureBoxStyle">
      <img ref="image" :src="url || undefined" :alt="source.label" :style="contentStyle"
        draggable="false" @load="readSize" @error="retry(t('multisource.streamError'))" />
    </div>
  </div>
  <span v-if="showStatus" class="source-label" :title="`${source.label} · ${frameWidth}×${frameHeight}`">
    <span :class="['source-dot', { live }]" />{{ source.label || source.id }}
  </span>
  <span v-if="showStatus" class="source-details">{{ t('multisource.sourceDetails', {
    width: frameWidth, height: frameHeight, fps: actualFps.toFixed(1),
  }) }}</span>
  <div v-if="error" class="source-error" role="status">{{ error }}</div>
</template>

<style scoped>
.source-picture { width: 100%; height: 100%; display: flex; align-items: safe center; justify-content: safe center; overflow: auto; }
.source-picture-box { position: relative; flex-shrink: 0; }
.source-picture img { position: absolute; top: 50%; left: 50%; object-fit: contain; pointer-events: none; user-select: none; }
.source-label { position: absolute; top: 3px; left: 4px; max-width: calc(100% - 8px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 1px 4px; border-radius: 3px; background: #0009; color: #fff; font-size: 10px; pointer-events: none; }
.source-dot { display: inline-block; width: 5px; height: 5px; border-radius: 50%; margin-right: 4px; background: #fbbf24; }
.source-dot.live { background: #4ade80; }
.source-error { position: absolute; bottom: 4px; left: 4px; right: 4px; padding: 4px; background: #400e; color: #fecaca; font-size: 12px; overflow-wrap: anywhere; pointer-events: none; }
.source-details { position: absolute; top: 22px; left: 4px; padding: 1px 4px; color: white; background: #000b; font-size: 10px; pointer-events: none; opacity: 0; }
:global(.native-source-tile:hover) .source-details, :global(.native-source-tile:focus-within) .source-details { opacity: 1; }
</style>
