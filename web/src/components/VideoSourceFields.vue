<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { VideoDevice } from '@/api'
import type { VideoSourceConfig } from '@/types/generated'
import { useVideoDeviceConfiguration } from '@/composables/useVideoDeviceConfiguration'
import { matchesVideoDevice, preferredVideoPath } from '@/lib/video-device-identity'
import { formatVideoDeviceLabel } from '@/lib/video-device-label'
import { getVideoFormatState, type VideoFormatSupportContext } from '@/lib/video-format-support'
import VideoInputFields from '@/components/VideoInputFields.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const props = withDefaults(defineProps<{
  modelValue: VideoSourceConfig
  devices: VideoDevice[]
  active?: boolean
  compact?: boolean
  primary?: boolean
  videoMode?: VideoFormatSupportContext
}>(), { active: true, compact: false, primary: true, videoMode: 'config' })
const emit = defineEmits<{ 'update:modelValue': [value: VideoSourceConfig] }>()
const { t } = useI18n()
function update<K extends keyof VideoSourceConfig>(key: K, value: VideoSourceConfig[K]) {
  emit('update:modelValue', { ...props.modelValue, [key]: value })
}
// Expose the configured alias to the existing capability/polling controls.
const resolvedDevices = computed(() => props.devices.map(device =>
  matchesVideoDevice(device, props.modelValue.device)
    ? { ...device, path: props.modelValue.device } : device,
))
const device = computed({ get: () => props.modelValue.device, set: value => update('device', value) })
const format = computed({ get: () => props.modelValue.format, set: value => update('format', value) })
const resolution = computed({
  get: () => `${props.modelValue.width}x${props.modelValue.height}`,
  set: value => {
    const [width, height] = value.split('x').map(Number)
    if (width && height) emit('update:modelValue', { ...props.modelValue, width, height })
  },
})
const fps = computed<number | null>({
  get: () => props.modelValue.fps,
  set: value => { if (value !== null) update('fps', value) },
})
const { selectedDevice, availableFormats, availableResolutions, availableFps,
  refreshingInputStatus, refreshInputStatus } = useVideoDeviceConfiguration({
  devices: resolvedDevices,
  selection: { device, format, resolution, fps },
  active: computed(() => props.active && props.modelValue.enabled),
  preferredFormat: device => device.formats.find(item => item.format.toUpperCase() === 'MJPEG')?.format,
})
const selectableFormats = computed(() => availableFormats.value.map(item => ({ ...item,
  disabled: getVideoFormatState(item.format, props.primary ? props.videoMode : 'mjpeg') === 'unsupported',
})))
</script>

<template>
  <div class="space-y-3">
    <div class="grid gap-3 sm:grid-cols-2">
      <div class="space-y-1.5">
        <Label :for="`source-label-${modelValue.id}`">{{ t('multisource.sourceName') }}</Label>
        <Input :id="`source-label-${modelValue.id}`" :model-value="modelValue.label"
          @update:model-value="update('label', String($event))" />
      </div>
      <div class="space-y-1.5">
        <Label :for="`source-device-${modelValue.id}`">{{ t('settings.videoDevice') }}</Label>
        <Select :model-value="selectedDevice?.path ?? ''"
          @update:model-value="value => update('device', String(value))">
          <SelectTrigger :id="`source-device-${modelValue.id}`" class="w-full">
            <SelectValue :placeholder="modelValue.device || t('settings.selectDevice')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="item in devices" :key="item.path"
              :value="matchesVideoDevice(item, modelValue.device) ? modelValue.device : preferredVideoPath(item)">
              {{ formatVideoDeviceLabel(item) }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
    <div class="space-y-1.5">
      <Label :for="`source-path-${modelValue.id}`">{{ t('multisource.deviceBinding') }}</Label>
      <Input :id="`source-path-${modelValue.id}`" :model-value="modelValue.device" class="font-mono text-xs"
        @update:model-value="update('device', String($event))" />
    </div>
    <VideoInputFields v-if="selectedDevice" :compact="compact" :device="selectedDevice"
      :formats="selectableFormats" :resolutions="availableResolutions" :fps-options="availableFps"
      :format="format" :resolution="resolution" :fps="fps" :refreshing="refreshingInputStatus"
      @update:format="format = $event" @update:resolution="resolution = $event"
      @update:fps="fps = $event" @refresh="refreshInputStatus" />
    <template v-else>
      <p class="text-xs text-muted-foreground" role="status">{{ t('multisource.deviceMissing') }}</p>
      <div class="grid grid-cols-3 gap-2">
        <div><Label :for="`source-width-${modelValue.id}`">{{ t('multisource.width') }}</Label>
          <Input :id="`source-width-${modelValue.id}`" type="number" min="320" max="7680" :model-value="modelValue.width"
            @update:model-value="update('width', Number($event))" /></div>
        <div><Label :for="`source-height-${modelValue.id}`">{{ t('multisource.height') }}</Label>
          <Input :id="`source-height-${modelValue.id}`" type="number" min="240" max="4320" :model-value="modelValue.height"
            @update:model-value="update('height', Number($event))" /></div>
        <div><Label :for="`source-fps-${modelValue.id}`">FPS</Label>
          <Input :id="`source-fps-${modelValue.id}`" type="number" min="1" max="120" :model-value="modelValue.fps"
            @update:model-value="update('fps', Number($event))" /></div>
      </div>
    </template>
    <div class="space-y-1.5">
      <Label :for="`source-quality-${modelValue.id}`">{{ t('multisource.quality') }}</Label>
      <Input :id="`source-quality-${modelValue.id}`" type="number" min="1" max="100" :model-value="modelValue.quality"
        @update:model-value="update('quality', Number($event))" />
    </div>
  </div>
</template>
