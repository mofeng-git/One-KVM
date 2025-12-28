<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Slider } from '@/components/ui/slider'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Monitor, RefreshCw, Loader2, Settings } from 'lucide-vue-next'
import HelpTooltip from '@/components/HelpTooltip.vue'
import { configApi, streamApi, type VideoCodecInfo, type EncoderBackendInfo } from '@/api'
import { useSystemStore } from '@/stores/system'
import { useDebounceFn } from '@vueuse/core'
import { useRouter } from 'vue-router'

export type VideoMode = 'mjpeg' | 'h264' | 'h265' | 'vp8' | 'vp9'

interface VideoDevice {
  path: string
  name: string
  driver: string
  formats: {
    format: string
    description: string
    resolutions: {
      width: number
      height: number
      fps: number[]
    }[]
  }[]
}

const props = defineProps<{
  open: boolean
  videoMode: VideoMode
  isAdmin?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'update:videoMode', value: VideoMode): void
}>()

const { t } = useI18n()
const systemStore = useSystemStore()
const router = useRouter()

// Device list
const devices = ref<VideoDevice[]>([])
const loadingDevices = ref(false)

// Codec list
const codecs = ref<VideoCodecInfo[]>([])
const loadingCodecs = ref(false)

// Backend list
const backends = ref<EncoderBackendInfo[]>([])
const currentEncoderBackend = ref<string>('auto')

// Browser supported codecs (WebRTC receive capabilities)
const browserSupportedCodecs = ref<Set<string>>(new Set())

// Check browser WebRTC codec support
function detectBrowserCodecSupport() {
  const supported = new Set<string>()

  // MJPEG is always supported (HTTP streaming, no WebRTC)
  supported.add('mjpeg')

  // Check WebRTC receive capabilities
  if (typeof RTCRtpReceiver !== 'undefined' && RTCRtpReceiver.getCapabilities) {
    const capabilities = RTCRtpReceiver.getCapabilities('video')
    if (capabilities?.codecs) {
      for (const codec of capabilities.codecs) {
        const mimeType = codec.mimeType.toLowerCase()
        // Map MIME types to our codec IDs
        if (mimeType.includes('h264') || mimeType.includes('avc')) {
          supported.add('h264')
        }
        if (mimeType.includes('h265') || mimeType.includes('hevc')) {
          supported.add('h265')
        }
        if (mimeType.includes('vp8')) {
          supported.add('vp8')
        }
        if (mimeType.includes('vp9')) {
          supported.add('vp9')
        }
        if (mimeType.includes('av1')) {
          supported.add('av1')
        }
      }
    }
  } else {
    // Fallback: assume basic codecs are supported
    supported.add('h264')
    supported.add('vp8')
    supported.add('vp9')
  }

  browserSupportedCodecs.value = supported
  console.info('[VideoConfig] Browser supported codecs:', Array.from(supported))
}

// Check if a codec is supported by browser
const isBrowserSupported = (codecId: string): boolean => {
  return browserSupportedCodecs.value.has(codecId)
}

// Translate backend name for display
const translateBackendName = (backend: string | undefined): string => {
  if (!backend) return ''
  // Translate known backend names
  const lowerBackend = backend.toLowerCase()
  if (lowerBackend === 'software') {
    return t('actionbar.backendSoftware')
  }
  if (lowerBackend === 'auto') {
    return t('actionbar.backendAuto')
  }
  // Hardware backends (VAAPI, V4L2 M2M, etc.) keep original names
  return backend
}

// Check if a format has fps >= 30 in any resolution
const hasHighFps = (format: { resolutions: { fps: number[] }[] }): boolean => {
  return format.resolutions.some(res => res.fps.some(fps => fps >= 30))
}

// Check if a format is recommended based on video mode
const isFormatRecommended = (formatName: string): boolean => {
  const formats = availableFormats.value
  const upperFormat = formatName.toUpperCase()

  // MJPEG/HTTP mode: recommend MJPEG
  if (props.videoMode === 'mjpeg') {
    return upperFormat === 'MJPEG'
  }

  // WebRTC mode: check NV12 first, then YUYV
  const currentFormat = formats.find(f => f.format.toUpperCase() === upperFormat)
  if (!currentFormat) return false

  // Check if NV12 exists with fps >= 30
  const nv12Format = formats.find(f => f.format.toUpperCase() === 'NV12')
  const nv12HasHighFps = nv12Format && hasHighFps(nv12Format)

  // Check if YUYV exists with fps >= 30
  const yuyvFormat = formats.find(f => f.format.toUpperCase() === 'YUYV')
  const yuyvHasHighFps = yuyvFormat && hasHighFps(yuyvFormat)

  // Priority 1: NV12 with high fps
  if (nv12HasHighFps) {
    return upperFormat === 'NV12'
  }

  // Priority 2: YUYV with high fps (only if NV12 doesn't qualify)
  if (yuyvHasHighFps) {
    return upperFormat === 'YUYV'
  }

  return false
}

// Selected values (mode comes from props)
const selectedDevice = ref<string>('')
const selectedFormat = ref<string>('')
const selectedResolution = ref<string>('')
const selectedFps = ref<number>(30)
const selectedBitrate = ref<number[]>([8000])

// UI state
const applying = ref(false)
const applyingBitrate = ref(false)

// Current config from store
const currentConfig = computed(() => ({
  device: systemStore.stream?.device || '',
  format: systemStore.stream?.format || '',
  width: systemStore.stream?.resolution?.[0] || 1920,
  height: systemStore.stream?.resolution?.[1] || 1080,
  fps: systemStore.stream?.targetFps || 30,
}))

// Button display text - simplified to just show label
const buttonText = computed(() => t('actionbar.videoConfig'))

// Available codecs for selection (filtered by backend support and enriched with backend info)
const availableCodecs = computed(() => {
  const allAvailable = codecs.value.filter(c => c.available)

  // Auto mode: show all available with their best (hardware-preferred) backend
  if (currentEncoderBackend.value === 'auto') {
    return allAvailable
  }

  // Specific backend: filter by supported formats and override backend info
  const backend = backends.value.find(b => b.id === currentEncoderBackend.value)
  if (!backend) return allAvailable

  return allAvailable
    .filter(codec => {
      // MJPEG is always available (doesn't require encoder)
      if (codec.id === 'mjpeg') return true
      // Check if codec format is supported by the configured backend
      return backend.supported_formats.includes(codec.id)
    })
    .map(codec => {
      // For MJPEG, keep original info
      if (codec.id === 'mjpeg') return codec

      // Override backend info for WebRTC codecs based on selected backend
      return {
        ...codec,
        hardware: backend.is_hardware,
        backend: backend.name,
      }
    })
})

// Cascading filters
const availableFormats = computed(() => {
  const device = devices.value.find(d => d.path === selectedDevice.value)
  return device?.formats || []
})

const availableResolutions = computed(() => {
  const format = availableFormats.value.find(f => f.format === selectedFormat.value)
  return format?.resolutions || []
})

const availableFps = computed(() => {
  const resolution = availableResolutions.value.find(
    r => `${r.width}x${r.height}` === selectedResolution.value
  )
  return resolution?.fps || []
})

// Get selected format description for display in trigger
const selectedFormatInfo = computed(() => {
  const format = availableFormats.value.find(f => f.format === selectedFormat.value)
  return format ? { description: format.description, format: format.format } : null
})

// Get selected codec info for display in trigger
const selectedCodecInfo = computed(() => {
  const codec = availableCodecs.value.find(c => c.id === props.videoMode)
  return codec || null
})

// Load devices
async function loadDevices() {
  loadingDevices.value = true
  try {
    const result = await configApi.listDevices()
    devices.value = result.video
  } catch (e) {
    console.info('[VideoConfig] Failed to load devices')
    toast.error(t('config.loadDevicesFailed'))
  } finally {
    loadingDevices.value = false
  }
}

// Load available codecs and backends
async function loadCodecs() {
  loadingCodecs.value = true
  try {
    const result = await streamApi.getCodecs()
    codecs.value = result.codecs
    backends.value = result.backends || []
  } catch (e) {
    console.info('[VideoConfig] Failed to load codecs')
    // Fallback to default codecs
    codecs.value = [
      { id: 'mjpeg', name: 'MJPEG / HTTP', protocol: 'http', hardware: false, backend: 'software', available: true },
      { id: 'h264', name: 'H.264 / WebRTC', protocol: 'webrtc', hardware: false, backend: 'software', available: true },
    ]
  } finally {
    loadingCodecs.value = false
  }
}

// Load current encoder backend from config
async function loadEncoderBackend() {
  try {
    const config = await configApi.get()
    // Access nested stream.encoder
    const streamConfig = config.stream as { encoder?: string } | undefined
    currentEncoderBackend.value = streamConfig?.encoder || 'auto'
  } catch (e) {
    console.info('[VideoConfig] Failed to load encoder backend config')
    currentEncoderBackend.value = 'auto'
  }
}

// Navigate to settings page (video tab)
function goToSettings() {
  router.push('/settings?tab=video')
}

// Initialize selected values from current config
function initializeFromCurrent() {
  const config = currentConfig.value
  selectedDevice.value = config.device
  selectedFormat.value = config.format
  selectedResolution.value = `${config.width}x${config.height}`
  selectedFps.value = config.fps
}

// Handle video mode change
function handleVideoModeChange(mode: unknown) {
  if (typeof mode !== 'string') return
  emit('update:videoMode', mode as VideoMode)
}

// Handle device change
function handleDeviceChange(devicePath: unknown) {
  if (typeof devicePath !== 'string') return
  selectedDevice.value = devicePath

  // Auto-select first format
  const device = devices.value.find(d => d.path === devicePath)
  if (device?.formats[0]) {
    selectedFormat.value = device.formats[0].format

    // Auto-select first resolution
    const resolution = device.formats[0].resolutions[0]
    if (resolution) {
      selectedResolution.value = `${resolution.width}x${resolution.height}`
      selectedFps.value = resolution.fps[0] || 30
    }
  }
}

// Handle format change
function handleFormatChange(format: unknown) {
  if (typeof format !== 'string') return
  selectedFormat.value = format

  // Auto-select first resolution for this format
  const formatData = availableFormats.value.find(f => f.format === format)
  if (formatData?.resolutions[0]) {
    const resolution = formatData.resolutions[0]
    selectedResolution.value = `${resolution.width}x${resolution.height}`
    selectedFps.value = resolution.fps[0] || 30
  }
}

// Handle resolution change
function handleResolutionChange(resolution: unknown) {
  if (typeof resolution !== 'string') return
  selectedResolution.value = resolution

  // Auto-select first FPS for this resolution
  const resolutionData = availableResolutions.value.find(
    r => `${r.width}x${r.height}` === resolution
  )
  if (resolutionData?.fps[0]) {
    selectedFps.value = resolutionData.fps[0]
  }
}

// Handle FPS change
function handleFpsChange(fps: unknown) {
  if (typeof fps !== 'string' && typeof fps !== 'number') return
  selectedFps.value = typeof fps === 'string' ? Number(fps) : fps
}

// Apply bitrate change (real-time)
async function applyBitrate(bitrate: number) {
  if (applyingBitrate.value) return
  applyingBitrate.value = true
  try {
    await streamApi.setBitrate(bitrate)
  } catch (e) {
    console.info('[VideoConfig] Failed to apply bitrate:', e)
  } finally {
    applyingBitrate.value = false
  }
}

// Debounced bitrate application
const debouncedApplyBitrate = useDebounceFn((bitrate: number) => {
  applyBitrate(bitrate)
}, 300)

// Watch bitrate slider changes (only when in WebRTC mode)
watch(selectedBitrate, (newValue) => {
  if (props.videoMode !== 'mjpeg' && newValue[0] !== undefined) {
    debouncedApplyBitrate(newValue[0])
  }
})

// Apply video configuration
async function applyVideoConfig() {
  const [width, height] = selectedResolution.value.split('x').map(Number)

  applying.value = true
  try {
    await configApi.update({
      video: {
        device: selectedDevice.value,
        format: selectedFormat.value,
        width,
        height,
        fps: selectedFps.value,
      },
    })

    toast.success(t('config.applied'))
    // Stream state will be updated via WebSocket system.device_info event
  } catch (e) {
    console.info('[VideoConfig] Failed to apply config:', e)
    // Error toast already shown by API layer
  } finally {
    applying.value = false
  }
}

// Watch open state
watch(() => props.open, (isOpen) => {
  if (isOpen) {
    // Detect browser codec support on first open
    if (browserSupportedCodecs.value.size === 0) {
      detectBrowserCodecSupport()
    }
    // Load devices on first open
    if (devices.value.length === 0) {
      loadDevices()
    }
    // Load codecs and backends on first open
    if (codecs.value.length === 0) {
      loadCodecs()
    }
    // Load encoder backend config
    loadEncoderBackend()
    // Initialize from current config
    initializeFromCurrent()
  }
})
</script>

<template>
  <Popover :open="open" @update:open="emit('update:open', $event)">
    <PopoverTrigger as-child>
      <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
        <Monitor class="h-4 w-4" />
        <span class="hidden sm:inline">{{ buttonText }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-[320px] p-3" align="start">
      <div class="space-y-3">
        <h4 class="text-sm font-medium">{{ t('actionbar.videoConfig') }}</h4>

        <Separator />

        <!-- Stream Settings Section -->
        <div class="space-y-3">
          <h5 class="text-xs font-medium text-muted-foreground">{{ t('actionbar.streamSettings') }}</h5>

          <!-- Mode Selection -->
          <div class="space-y-2">
            <Label class="text-xs">{{ t('actionbar.videoMode') }}</Label>
            <Select
              :model-value="props.videoMode"
              @update:model-value="handleVideoModeChange"
              :disabled="loadingCodecs || availableCodecs.length === 0"
            >
              <SelectTrigger class="h-8 text-xs">
                <div v-if="selectedCodecInfo" class="flex items-center gap-1.5 truncate">
                  <span class="truncate">{{ selectedCodecInfo.name }}</span>
                  <span
                    v-if="selectedCodecInfo.backend && selectedCodecInfo.id !== 'mjpeg'"
                    class="text-[10px] px-1 py-0.5 rounded shrink-0"
                    :class="selectedCodecInfo.hardware
                      ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                      : 'bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300'"
                  >
                    {{ translateBackendName(selectedCodecInfo.backend) }}
                  </span>
                </div>
                <span v-else class="text-muted-foreground">{{ loadingCodecs ? t('common.loading') : t('actionbar.selectMode') }}</span>
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="codec in availableCodecs"
                  :key="codec.id"
                  :value="codec.id"
                  :disabled="!isBrowserSupported(codec.id)"
                  :class="['text-xs', { 'opacity-50': !isBrowserSupported(codec.id) }]"
                >
                  <div class="flex items-center gap-2">
                    <span>{{ codec.name }}</span>
                    <!-- Show backend badge for WebRTC codecs -->
                    <span
                      v-if="codec.backend && codec.id !== 'mjpeg'"
                      class="text-[10px] px-1.5 py-0.5 rounded"
                      :class="codec.hardware
                        ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
                        : 'bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300'"
                    >
                      {{ translateBackendName(codec.backend) }}
                    </span>
                    <span
                      v-if="!isBrowserSupported(codec.id)"
                      class="text-[10px] text-muted-foreground"
                    >
                      ({{ t('actionbar.browserUnsupported') }})
                    </span>
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
            <p v-if="props.videoMode !== 'mjpeg'" class="text-xs text-muted-foreground">
              {{ t('actionbar.webrtcHint') }}
            </p>
          </div>

          <!-- Bitrate Slider - Only shown for WebRTC modes -->
          <div v-if="props.videoMode !== 'mjpeg'" class="space-y-2">
            <div class="flex items-center gap-1">
              <Label class="text-xs">{{ t('actionbar.bitrate') }}</Label>
              <HelpTooltip :content="t('help.videoBitrate')" icon-size="sm" />
            </div>
            <div class="flex items-center gap-3">
              <Slider
                v-model="selectedBitrate"
                :min="1000"
                :max="15000"
                :step="500"
                class="flex-1"
              />
              <span class="text-xs text-muted-foreground w-20 text-right">{{ selectedBitrate[0] }} kbps</span>
            </div>
          </div>

          <!-- Settings Link - Admin only -->
          <Button
            v-if="props.isAdmin"
            variant="ghost"
            size="sm"
            class="w-full h-7 text-xs text-muted-foreground hover:text-foreground justify-start px-0"
            @click="goToSettings"
          >
            <Settings class="h-3.5 w-3.5 mr-1.5" />
            {{ t('actionbar.changeEncoderBackend') }}
          </Button>
        </div>

        <!-- Device Settings Section - Admin only -->
        <template v-if="props.isAdmin">
          <Separator />

          <div class="space-y-3">
          <div class="flex items-center justify-between">
            <h5 class="text-xs font-medium text-muted-foreground">{{ t('actionbar.deviceSettings') }}</h5>
            <Button
              variant="ghost"
              size="icon"
              class="h-6 w-6"
              :disabled="loadingDevices"
              @click="loadDevices"
            >
              <RefreshCw :class="['h-3.5 w-3.5', loadingDevices && 'animate-spin']" />
            </Button>
          </div>

          <!-- Device Selection -->
          <div class="space-y-2">
            <Label class="text-xs">{{ t('actionbar.videoDevice') }}</Label>
            <Select
              :model-value="selectedDevice"
              @update:model-value="handleDeviceChange"
              :disabled="loadingDevices || devices.length === 0"
            >
              <SelectTrigger class="h-8 text-xs">
                <SelectValue :placeholder="loadingDevices ? t('common.loading') : t('actionbar.selectDevice')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="device in devices"
                  :key="device.path"
                  :value="device.path"
                  class="text-xs"
                >
                  {{ device.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- Format Selection -->
          <div class="space-y-2">
            <Label class="text-xs">{{ t('actionbar.videoFormat') }}</Label>
            <Select
              :model-value="selectedFormat"
              @update:model-value="handleFormatChange"
              :disabled="!selectedDevice || availableFormats.length === 0"
            >
              <SelectTrigger class="h-8 text-xs">
                <div v-if="selectedFormatInfo" class="flex items-center gap-1.5 truncate">
                  <span class="truncate">{{ selectedFormatInfo.description }}</span>
                  <span
                    v-if="isFormatRecommended(selectedFormatInfo.format)"
                    class="text-[10px] px-1 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300 shrink-0"
                  >
                    {{ t('actionbar.recommended') }}
                  </span>
                </div>
                <span v-else class="text-muted-foreground">{{ t('actionbar.selectFormat') }}</span>
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="format in availableFormats"
                  :key="format.format"
                  :value="format.format"
                  class="text-xs"
                >
                  <div class="flex items-center gap-2">
                    <span>{{ format.description }}</span>
                    <span
                      v-if="isFormatRecommended(format.format)"
                      class="text-[10px] px-1.5 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300"
                    >
                      {{ t('actionbar.recommended') }}
                    </span>
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- Resolution Selection -->
          <div class="space-y-2">
            <Label class="text-xs">{{ t('actionbar.videoResolution') }}</Label>
            <Select
              :model-value="selectedResolution"
              @update:model-value="handleResolutionChange"
              :disabled="!selectedFormat || availableResolutions.length === 0"
            >
              <SelectTrigger class="h-8 text-xs">
                <SelectValue :placeholder="t('actionbar.selectResolution')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="res in availableResolutions"
                  :key="`${res.width}x${res.height}`"
                  :value="`${res.width}x${res.height}`"
                  class="text-xs"
                >
                  {{ res.width }} x {{ res.height }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- FPS Selection -->
          <div class="space-y-2">
            <Label class="text-xs">{{ t('actionbar.videoFps') }}</Label>
            <Select
              :model-value="String(selectedFps)"
              @update:model-value="handleFpsChange"
              :disabled="!selectedResolution || availableFps.length === 0"
            >
              <SelectTrigger class="h-8 text-xs">
                <SelectValue :placeholder="t('actionbar.selectFps')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="fps in availableFps"
                  :key="fps"
                  :value="String(fps)"
                  class="text-xs"
                >
                  {{ fps }} FPS
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- Apply Button -->
          <Button
            class="w-full h-8 text-xs"
            :disabled="applying || !selectedDevice || !selectedFormat"
            @click="applyVideoConfig"
          >
            <Loader2 v-if="applying" class="h-3.5 w-3.5 mr-1.5 animate-spin" />
            <span>{{ applying ? t('actionbar.applying') : t('common.apply') }}</span>
          </Button>
        </div>
        </template>
      </div>
    </PopoverContent>
  </Popover>
</template>
