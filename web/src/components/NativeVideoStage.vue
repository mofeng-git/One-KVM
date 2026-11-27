<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { useElementSize } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import type { VideoSource } from '@/api/videoSources'
import { videoStageGeometry, type LayoutPreset } from '@/lib/videoStageGeometry'
import NativeSourceVideo from '@/components/NativeSourceVideo.vue'

const props = defineProps<{
  sources: VideoSource[]
  primarySourceId: string
  activeSourceId: string
  primarySize: { width: number; height: number } | null
  primaryClientId: string
  primaryMjpeg: boolean
  primaryUrl: string
  primaryRotation: number
  primaryActual: boolean
  single: boolean
  captured: boolean
}>()
const emit = defineEmits<{
  select: [id: string]
  inputTarget: [element: HTMLDivElement | null]
  mousemove: [event: MouseEvent]
  mousedown: [event: MouseEvent]
  mouseup: [event: MouseEvent]
  mouseleave: []
  wheel: [event: WheelEvent]
  contextmenu: [event: MouseEvent]
  pointerdown: [event: PointerEvent]
  pointermove: [event: PointerEvent]
  pointerup: [event: PointerEvent]
  pointercancel: [event: PointerEvent]
  reconnect: []
  localTools: []
  capture: []
}>()
const { t } = useI18n()
const storageKey = `nativeVideoStage:${location.host}`
function readPreferences() {
  try { return JSON.parse(localStorage.getItem(storageKey) || '{}') } catch { return {} }
}
const saved = readPreferences()
const preset = ref<LayoutPreset>(['auto', 'horizontal', 'vertical', 'grid'].includes(saved.preset) ? saved.preset : 'auto')
const focusedSourceId = ref(typeof saved.focusedSourceId === 'string' ? saved.focusedSourceId : '')
const rotations = reactive<Record<string, number>>(saved.rotations || {})
const actual = reactive<Record<string, boolean>>(saved.actual || {})
const sizes = reactive<Record<string, { width: number; height: number }>>({})
const root = ref<HTMLDivElement | null>(null)
const { width, height } = useElementSize(root)
const targets = new Map<string, HTMLDivElement>()
const multisource = computed(() => props.sources.length > 1)
// Captured/local-console affordances follow the rendered tile, not the tile count:
// the sole enabled secondary (primary disabled) is captured directly as well.
const pointTarget = computed(() => multisource.value || props.activeSourceId !== props.primarySourceId)
const singleSourceId = computed(() => props.sources.some(source => source.id === props.primarySourceId)
  ? props.primarySourceId : props.sources.find(source => source.id === props.activeSourceId)?.id || props.sources[0]?.id || '')
const visibleId = computed(() => props.single ? singleSourceId.value : focusedSourceId.value)
const sourceSizes = computed(() => props.sources.map(source => {
  const size = source.id === props.primarySourceId ? props.primarySize || source : sizes[source.id] || source
  const rotation = source.id === props.primarySourceId ? props.primaryRotation : rotations[source.id] || 0
  return { id: source.id, width: rotation % 180 ? size.height : size.width,
    height: rotation % 180 ? size.width : size.height }
}))
const geometry = computed(() => videoStageGeometry(sourceSizes.value, preset.value, width.value / (height.value || 1)))
const scale = computed(() => Math.min(width.value / geometry.value.width, height.value / geometry.value.height))
const focusedSize = computed(() => sourceSizes.value.find(source => source.id === visibleId.value))
const focusedScale = computed(() => focusedSize.value
  ? Math.min(width.value / focusedSize.value.width, height.value / focusedSize.value.height) : 1)
const primaryContentStyle = computed(() => {
  const source = props.primarySize || props.sources.find(source => source.id === props.primarySourceId)
  if (!source) return { width: '100%', height: '100%' }
  const rect = tileStyle(props.primarySourceId)
  const tileWidth = parseFloat(rect.width || '0')
  const tileHeight = parseFloat(rect.height || '0')
  const quarter = props.primaryRotation % 180 !== 0
  const fit = props.primaryActual ? 1 : Math.min(tileWidth / (quarter ? source.height : source.width),
    tileHeight / (quarter ? source.width : source.height))
  return { width: `${source.width * fit}px`, height: `${source.height * fit}px`, transform: `rotate(${props.primaryRotation}deg)` }
})

function tileStyle(id: string) {
  if (!multisource.value) return { width: '100%', height: '100%' }
  if (visibleId.value) {
    const source = focusedSize.value
    if (id !== visibleId.value || !source) return { visibility: 'hidden' as const, position: 'absolute' as const,
      width: '1px', height: '1px', pointerEvents: 'none' as const }
    const s = actual[id] ? 1 : focusedScale.value
    return { width: `${source.width * s}px`, height: `${source.height * s}px`,
      left: `${Math.max(0, (width.value - source.width * s) / 2)}px`,
      top: `${Math.max(0, (height.value - source.height * s) / 2)}px` }
  }
  const rect = geometry.value.frames.find(frame => frame.id === id)
  if (!rect) return {}
  return { width: `${rect.width * scale.value}px`, height: `${rect.height * scale.value}px`,
    left: `${(width.value - geometry.value.width * scale.value) / 2 + rect.x * scale.value}px`,
    top: `${(height.value - geometry.value.height * scale.value) / 2 + rect.y * scale.value}px` }
}
/** Programmatic focus of the tile that owns pointer input. */
function select(id: string) {
  if (props.activeSourceId !== id) emit('select', id)
  emit('inputTarget', targets.get(id) || null)
}
function bindTarget(id: string, el: unknown) {
  if (el instanceof HTMLDivElement) targets.set(id, el)
  else targets.delete(id)
  if (id === props.activeSourceId || (!props.activeSourceId && id === props.primarySourceId)) {
    emit('inputTarget', targets.get(id) || null)
  }
}
function down(source: VideoSource, event: MouseEvent) {
  select(source.id)
  if (multisource.value && !document.pointerLockElement) {
    targets.get(source.id)?.focus()
    return
  }
  emit('mousedown', event)
}
function capture(id: string) {
  select(id)
  targets.get(id)?.focus()
  emit('capture')
}
function wheel(source: VideoSource, event: WheelEvent) {
  select(source.id)
  if (multisource.value && !props.captured) return
  event.preventDefault()
  emit('wheel', event)
}
function focusSource(id: string) {
  emit('localTools')
  select(id)
  focusedSourceId.value = focusedSourceId.value === id ? '' : id
}
function setRotation(id: string, rotation: number) { rotations[id] = rotation }
function setActual(id: string, value: boolean) { actual[id] = value }
defineExpose({ select, setRotation, setActual, rotations, actual })
watch([preset, rotations, actual, focusedSourceId], () => {
  try { localStorage.setItem(storageKey, JSON.stringify({ preset: preset.value, rotations, actual, focusedSourceId: focusedSourceId.value })) } catch { /* Storage may be disabled. */ }
}, { deep: true })
watch(() => props.activeSourceId, async id => { await nextTick(); emit('inputTarget', targets.get(id) || null) })
watch(() => props.sources.map(source => source.id), ids => {
  if (!ids.includes(focusedSourceId.value)) focusedSourceId.value = ''
})
watch(() => [props.single, singleSourceId.value], () => {
  if (props.single) select(singleSourceId.value)
}, { immediate: true })
</script>

<template>
  <div ref="root" class="native-video-stage" :class="{ multisource, 'point-target': pointTarget, captured }">
    <div v-if="multisource" class="stage-layout-tools" data-local-console-tools @pointerdown.stop="emit('localTools')">
      <button v-for="layout in (['horizontal', 'vertical', 'grid', 'auto'] as const)" :key="layout"
        :aria-pressed="preset === layout" @click="preset = layout">{{ t(`multisource.${layout}`) }}</button>
      <button v-if="focusedSourceId" @click="focusedSourceId = ''; emit('localTools')">{{ t('multisource.restore') }}</button>
    </div>
    <div v-for="source in sources" :key="source.id" :ref="el => bindTarget(source.id, el)"
      class="native-source-tile" :class="{ selected: source.id === activeSourceId }"
      :style="tileStyle(source.id)" :tabindex="visibleId && visibleId !== source.id ? -1 : 0"
      :aria-label="source.label || source.id"
      @focus="select(source.id)" @dblclick.stop="multisource && focusSource(source.id)"
      @mousedown="down(source, $event)" @mouseup="emit('mouseup', $event)"
      @mousemove="source.id === activeSourceId && emit('mousemove', $event)"
      @mouseleave="emit('mouseleave')" @wheel="wheel(source, $event)"
      @contextmenu="emit('contextmenu', $event)"
      @pointerdown="select(source.id); emit('pointerdown', $event)"
      @pointermove="source.id === activeSourceId && emit('pointermove', $event)"
      @pointerup="emit('pointerup', $event)" @pointercancel="emit('pointercancel', $event)">
      <NativeSourceVideo :source="source" :primary="source.id === primarySourceId"
        :primary-client-id="primaryClientId" :primary-mjpeg="primaryMjpeg" :primary-url="primaryUrl"
        :rotation="rotations[source.id] || 0" :actual="!!actual[source.id]"
        :show-status="pointTarget && (multisource || source.id === activeSourceId)"
        :monitor="source.id !== '__native_primary__' && (multisource || source.id === activeSourceId)"
        :frame-size="source.id === primarySourceId ? primarySize : null"
        @size="(w, h) => sizes[source.id] = { width: w, height: h }" @reconnect="emit('reconnect')">
        <slot :multisource="multisource" :primary-content-style="primaryContentStyle" />
      </NativeSourceVideo>
      <slot v-if="pointTarget && source.id === activeSourceId" name="cursor" />
      <span v-if="pointTarget && captured && source.id === activeSourceId" class="capture-state" role="status">{{ t('multisource.captured') }}</span>
      <div v-if="pointTarget && !captured" class="tile-controls" data-local-console-tools
        @pointerdown.stop="emit('localTools')" @mousedown.stop @mouseup.stop @dblclick.stop @wheel.stop @mousemove.stop>
        <button v-if="multisource" @click="focusSource(source.id)" :aria-label="t(focusedSourceId === source.id ? 'multisource.restore' : 'multisource.enlarge')">⛶</button>
        <button @click="capture(source.id)" :title="t('multisource.relativeOnly')">{{ t('multisource.capture') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.native-video-stage { position: relative; width: 100%; height: 100%; min-height: 120px; overflow: auto; }
.native-source-tile { position: relative; background: black; overflow: visible; outline: none; touch-action: none; cursor: none; }
.point-target:not(.captured) .native-source-tile { cursor: default; }
.capture-state { position: absolute; top: 3px; right: 4px; z-index: 20; color: white; background: #075985d9; padding: 2px 5px; border-radius: 3px; font-size: 10px; pointer-events: none; }
.multisource .native-source-tile { position: absolute; display: flex; align-items: center; justify-content: center; overflow: hidden; }
.multisource .native-source-tile.selected { outline: 1px solid #38bdf8; outline-offset: -1px; }
.stage-layout-tools { position: sticky; width: max-content; max-width: 100%; top: 24px; left: 50%; transform: translateX(-50%); z-index: 25; display: flex; gap: 2px; opacity: 0; transition: opacity .15s; }
.native-video-stage:hover .stage-layout-tools, .stage-layout-tools:focus-within { opacity: 1; }
.stage-layout-tools button, .tile-controls button { color: white; background: #161616d9; border: 1px solid #ffffff40; padding: 3px 6px; border-radius: 4px; font-size: 11px; cursor: pointer; }
.stage-layout-tools button[aria-pressed=true] { background: #075985; }
.tile-controls { position: absolute; top: 3px; right: 4px; z-index: 20; opacity: 0; }
.native-source-tile:hover .tile-controls, .native-source-tile:focus-within .tile-controls { opacity: 1; }
@media (hover: none) { .stage-layout-tools { opacity: 1; } }
</style>
