<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Eye, EyeOff, Loader2, RefreshCw, Save } from 'lucide-vue-next'
import { configApi, streamApi, type DeviceList, type EncoderBackendInfo } from '@/api'
import { EncoderType } from '@/types/generated'
import { useConfigStore } from '@/stores/config'
import { toConfigFps } from '@/lib/fps'
import { formatVideoDeviceLabel } from '@/lib/video-device-label'
import { getVideoFormatState } from '@/lib/video-format-support'
import { useVideoDeviceConfiguration } from '@/composables/useVideoDeviceConfiguration'
import CaptureActivityControl from '@/components/CaptureActivityControl.vue'
import VideoInputFields from '@/components/VideoInputFields.vue'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Card, CardHeader, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from '@/components/ui/select'
const props = defineProps<{ active: boolean }>()
const { t } = useI18n()
const configStore = useConfigStore()
const EMPTY_SELECT_VALUE = '__one-kvm-empty-select-value__'
const showPasswords = ref(false)
const devices = ref<Pick<DeviceList, 'video'>>({ video: [] })
const config = ref({
  video_device: '',
  video_format: '',
  video_width: 1920,
  video_height: 1080,
  video_fps: 30,
  encoder_backend: EncoderType.Auto,
  stun_server: '',
  turn_server: '',
  turn_username: '',
  turn_password: '',
})
const availableBackends = ref<EncoderBackendInfo[]>([])

const selectedBackendFormats = computed(() => {
  if (config.value.encoder_backend === 'auto') return []
  const backend = availableBackends.value.find((b) => b.id === config.value.encoder_backend)
  return backend?.supported_formats || []
})

const videoDeviceSelection = computed({
  get: () => config.value.video_device,
  set: (value) => {
    config.value.video_device = value
  },
})
const videoFormatSelection = computed({
  get: () => config.value.video_format,
  set: (value) => {
    config.value.video_format = value
  },
})
const videoResolutionSelection = computed({
  get: () => `${config.value.video_width}x${config.value.video_height}`,
  set: (value) => {
    const [width, height] = value.split('x').map(Number)
    if (width && height) {
      config.value.video_width = width
      config.value.video_height = height
    }
  },
})
const videoFpsSelection = computed<number | null>({
  get: () => config.value.video_fps,
  set: (value) => {
    if (value !== null) config.value.video_fps = value
  },
})

const videoConfiguration = useVideoDeviceConfiguration({
  devices: computed(() => devices.value.video),
  selection: {
    device: videoDeviceSelection,
    format: videoFormatSelection,
    resolution: videoResolutionSelection,
    fps: videoFpsSelection,
  },
  active: computed(() => props.active),
  listenForStreamEvents: true,
  preferredFormat: (device) =>
    device.formats.find(
      (format) =>
        getVideoFormatState(format.format, 'config', config.value.encoder_backend) !==
        'unsupported',
    )?.format,
})
const {
  selectedDevice,
  isSourceFollowing,
  availableFormats,
  availableResolutions,
  availableFps,
  refreshInputStatus,
  refreshingInputStatus,
} = videoConfiguration

const availableFormatOptions = computed(() => {
  return availableFormats.value.map((format) => {
    const state = getVideoFormatState(format.format, 'config', config.value.encoder_backend)
    return {
      ...format,
      state,
      disabled: state === 'unsupported',
    }
  })
})

const selectableFormats = computed(() => {
  return availableFormatOptions.value.filter((format) => !format.disabled)
})

watch(
  selectableFormats,
  () => {
    if (selectableFormats.value.length === 0) {
      config.value.video_format = ''
      return
    }

    const isValid = selectableFormats.value.some((f) => f.format === config.value.video_format)
    if (!isValid) {
      config.value.video_format = selectableFormats.value[0]?.format || ''
    }
  },
  { deep: true },
)

const videoSaving = ref<'capture' | 'encoder' | 'connection' | null>(null)
async function saveVideoCard(card: 'capture' | 'encoder' | 'connection') {
  if (videoSaving.value) return
  videoSaving.value = card
  try {
    if (card === 'capture') {
      await configStore.updateVideo(
        isSourceFollowing.value
          ? { device: config.value.video_device || undefined }
          : {
              device: config.value.video_device || undefined,
              format: config.value.video_format || undefined,
              width: config.value.video_width,
              height: config.value.video_height,
              fps: toConfigFps(config.value.video_fps),
            },
      )
    } else if (card === 'encoder') {
      await configStore.updateStream({ encoder: config.value.encoder_backend })
    } else {
      await configStore.updateStream({
        stun_server: config.value.stun_server.trim(),
        turn_server: config.value.turn_server.trim(),
        turn_username: config.value.turn_username.trim(),
        turn_password: config.value.turn_password.trim(),
      })
    }
    toast.success(t('settingsUi.saveDone'))
  } catch (error) {
    toast.error(t('settingsUi.saveFailed'), {
      description: error instanceof Error ? error.message : undefined,
    })
  } finally {
    videoSaving.value = null
  }
}

async function loadVideoConfig() {
  try {
    const [video, stream] = await Promise.all([
      configStore.refreshVideo(), configStore.refreshStream(),
    ])
    config.value = {
      video_device: video.device || '',
      video_format: video.format || '',
      video_width: video.width || 1920,
      video_height: video.height || 1080,
      video_fps: video.fps || 30,
      encoder_backend: stream.encoder || EncoderType.Auto,
      stun_server: stream.stun_server || '',
      turn_server: stream.turn_server || '',
      turn_username: stream.turn_username || '',
      turn_password: stream.turn_password || '',
    }
  } catch { /* The request layer displays loading failures. */ }
}
async function loadDeviceConfig() {
  try { devices.value = await configApi.listDevices() } catch {}
}
async function loadBackends() {
  try {
    const result = await streamApi.getCodecs()
    availableBackends.value = result.backends || []
  } catch {}
}

watch(() => props.active, async (active) => {
  if (active) await Promise.all([loadVideoConfig(), loadDeviceConfig(), loadBackends()])
}, { immediate: true })
</script>

<template>
<!-- Video Section -->
<div v-show="active" class="space-y-4">
  <CaptureActivityControl :active="active" />
  <!-- Video Device Settings -->
  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.videoSettingsDesc')">{{
        t('settings.videoSettings')
      }}</CardTitle>
      <div class="settings-card-actions">
        <Button
          variant="ghost"
          size="icon-sm"
          :aria-label="t('common.refresh')"
          @click="loadDeviceConfig"
        >
          <RefreshCw class="size-4" />
        </Button>
        <div class="settings-save-action">
          <Button :disabled="videoSaving !== null" @click="saveVideoCard('capture')"
            ><Loader2 v-if="videoSaving === 'capture'" class="size-4 animate-spin" /><Save
              v-else
              class="size-4"
            />{{ t('settingsUi.saveCapture') }}</Button
          >
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-field space-y-2">
        <Label for="video-device">{{ t('settings.videoDevice') }}</Label>
        <Select
          :model-value="config.video_device"
          @update:model-value="
            (value) =>
              (config.video_device = value === EMPTY_SELECT_VALUE ? '' : String(value))
          "
        >
          <SelectTrigger id="video-device" class="w-full"
            ><SelectValue :placeholder="t('settings.selectDevice')"
          /></SelectTrigger>
          <SelectContent>
            <SelectItem :value="EMPTY_SELECT_VALUE">{{
              t('settings.selectDevice')
            }}</SelectItem>
            <SelectItem v-for="dev in devices.video" :key="dev.path" :value="dev.path">{{
              formatVideoDeviceLabel(dev)
            }}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <VideoInputFields
        v-if="selectedDevice"
        :device="selectedDevice"
        :formats="availableFormatOptions"
        :resolutions="availableResolutions"
        :fps-options="availableFps"
        :format="config.video_format"
        :resolution="videoResolutionSelection"
        :fps="config.video_fps"
        :refreshing="refreshingInputStatus"
        @update:format="config.video_format = $event"
        @update:resolution="videoResolutionSelection = $event"
        @update:fps="config.video_fps = $event"
        @refresh="refreshInputStatus"
      />
    </CardContent>
  </Card>

  <!-- Encoder Settings -->
  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.encoderBackendDesc')">{{
        t('settings.encoderBackend')
      }}</CardTitle>
      <div class="settings-card-actions">
        <div class="settings-save-action">
          <Button :disabled="videoSaving !== null" @click="saveVideoCard('encoder')"
            ><Loader2 v-if="videoSaving === 'encoder'" class="size-4 animate-spin" /><Save
              v-else
              class="size-4"
            />{{ t('settingsUi.saveEncoder') }}</Button
          >
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-field space-y-2">
        <Label for="encoder-backend">{{ t('settings.backend') }}</Label>
        <Select v-model="config.encoder_backend">
          <SelectTrigger id="encoder-backend" class="w-full"
            ><SelectValue
          /></SelectTrigger>
          <SelectContent>
            <SelectItem value="auto">{{ t('settings.autoRecommended') }}</SelectItem>
            <SelectItem
              v-for="backend in availableBackends"
              :key="backend.id"
              :value="backend.id"
              >{{ backend.name }}
              {{
                backend.is_hardware
                  ? `(${t('settings.hardware')})`
                  : `(${t('settings.software')})`
              }}</SelectItem
            >
          </SelectContent>
        </Select>
      </div>
      <div
        v-if="config.encoder_backend !== 'auto' && selectedBackendFormats.length > 0"
        class="settings-field space-y-2"
      >
        <Label>{{ t('settings.supportedFormats') }}</Label>
        <div class="flex flex-wrap gap-2">
          <Badge
            v-for="format in selectedBackendFormats"
            :key="format"
            variant="outline"
            >{{ format.toUpperCase() }}</Badge
          >
        </div>
      </div>
    </CardContent>
  </Card>

  <!-- WebRTC/STUN/TURN Settings -->
  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.webrtcSettingsDesc')">{{
        t('settings.webrtcSettings')
      }}</CardTitle>
      <div class="settings-card-actions">
        <div class="settings-save-action">
          <Button :disabled="videoSaving !== null" @click="saveVideoCard('connection')"
            ><Loader2
              v-if="videoSaving === 'connection'"
              class="size-4 animate-spin"
            /><Save v-else class="size-4" />{{ t('settingsUi.saveConnection') }}</Button
          >
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-field space-y-2">
        <Label for="stun-server">{{ t('settings.stunServer') }}</Label>
        <Input
          id="stun-server"
          v-model="config.stun_server"
          :placeholder="t('settings.stunServerPlaceholder')"
        />
      </div>
      <Separator />
      <div class="settings-field space-y-2">
        <Label for="turn-server">{{ t('settings.turnServer') }}</Label>
        <Input
          id="turn-server"
          v-model="config.turn_server"
          :placeholder="t('settings.turnServerPlaceholder')"
        />
      </div>
      <div class="settings-field-list grid gap-4 sm:grid-cols-2">
        <div class="settings-field space-y-2">
          <Label for="turn-username">{{ t('settings.turnUsername') }}</Label>
          <Input
            id="turn-username"
            v-model="config.turn_username"
            :disabled="!config.stun_server && !config.turn_server"
          />
        </div>
        <div class="settings-field space-y-2">
          <Label for="turn-password">{{ t('settings.turnPassword') }}</Label>
          <div class="relative">
            <Input
              id="turn-password"
              v-model="config.turn_password"
              :type="showPasswords ? 'text' : 'password'"
              autocomplete="off"
              :disabled="!config.stun_server && !config.turn_server"
            />
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground"
              :aria-label="
                showPasswords
                  ? t('extensions.rustdesk.hidePassword')
                  : t('extensions.rustdesk.showPassword')
              "
              @click="showPasswords = !showPasswords"
            >
              <Eye v-if="!showPasswords" class="size-4" />
              <EyeOff v-else class="size-4" />
            </Button>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</div>

</template>
