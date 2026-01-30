<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
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
import { MousePointer, Move, Loader2, RefreshCw } from 'lucide-vue-next'
import HelpTooltip from '@/components/HelpTooltip.vue'
import { configApi } from '@/api'
import { useConfigStore } from '@/stores/config'
import { HidBackend } from '@/types/generated'
import type { HidConfigUpdate } from '@/types/generated'

const props = defineProps<{
  open: boolean
  mouseMode?: 'absolute' | 'relative'
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'update:mouseMode', value: 'absolute' | 'relative'): void
}>()

const { t } = useI18n()
const configStore = useConfigStore()

const DEFAULT_MOUSE_MOVE_SEND_INTERVAL_MS = 16

function clampMouseMoveSendIntervalMs(ms: number): number {
  if (!Number.isFinite(ms)) return DEFAULT_MOUSE_MOVE_SEND_INTERVAL_MS
  return Math.max(0, Math.min(1000, Math.floor(ms)))
}

function loadMouseMoveSendIntervalFromStorage(): number {
  const raw = localStorage.getItem('hidMouseThrottle')
  const parsed = raw === null ? NaN : Number(raw)
  return clampMouseMoveSendIntervalMs(
    Number.isFinite(parsed) ? parsed : DEFAULT_MOUSE_MOVE_SEND_INTERVAL_MS
  )
}

// Mouse Settings (real-time)
const mouseThrottle = ref<number>(
  loadMouseMoveSendIntervalFromStorage()
)
const showCursor = ref<boolean>(
  localStorage.getItem('hidShowCursor') !== 'false' // default true
)

// Watch showCursor changes and sync to localStorage + notify ConsoleView
watch(showCursor, (newValue, oldValue) => {
  // Only sync if value actually changed (avoid triggering on initialization)
  if (newValue !== oldValue) {
    localStorage.setItem('hidShowCursor', newValue ? 'true' : 'false')
    window.dispatchEvent(new CustomEvent('hidCursorVisibilityChanged', {
      detail: { visible: newValue }
    }))
  }
})

// HID Device Settings (requires apply)
const hidBackend = ref<HidBackend>(HidBackend.None)
const devicePath = ref<string>('')
const baudrate = ref<number>(9600)

// UI state
const applying = ref(false)
const loadingDevices = ref(false)

// Device lists
const serialDevices = ref<Array<{ path: string; name: string }>>([])
const udcDevices = ref<Array<{ name: string }>>([])

// Button display text - simplified to just show label
const buttonText = computed(() => t('actionbar.hidConfig'))

// Available device paths based on backend type
const availableDevicePaths = computed(() => {
  if (hidBackend.value === HidBackend.Ch9329) {
    return serialDevices.value
  } else if (hidBackend.value === HidBackend.Otg) {
    // For OTG, we show UDC devices
    return udcDevices.value.map(udc => ({
      path: udc.name,
      name: udc.name,
    }))
  }
  return []
})

// Load devices
async function loadDevices() {
  loadingDevices.value = true
  try {
    const result = await configApi.listDevices()
    serialDevices.value = result.serial
    udcDevices.value = result.udc
  } catch (e) {
    console.info('[HidConfig] Failed to load devices')
  } finally {
    loadingDevices.value = false
  }
}

// Initialize from current config
function initializeFromCurrent() {
  // Re-sync real-time settings from localStorage
  mouseThrottle.value = loadMouseMoveSendIntervalFromStorage()

  const storedCursor = localStorage.getItem('hidShowCursor') !== 'false'
  showCursor.value = storedCursor

  // Initialize HID device settings from system state
  const hid = configStore.hid
  if (hid) {
    hidBackend.value = hid.backend || HidBackend.None
    if (hidBackend.value === HidBackend.Ch9329) {
      devicePath.value = hid.ch9329_port || ''
      baudrate.value = hid.ch9329_baudrate || 9600
    } else if (hidBackend.value === HidBackend.Otg) {
      devicePath.value = hid.otg_udc || ''
    } else {
      devicePath.value = ''
    }
  }
}

// Toggle mouse mode (real-time)
function toggleMouseMode() {
  const newMode = props.mouseMode === 'absolute' ? 'relative' : 'absolute'
  emit('update:mouseMode', newMode)

  // Update backend config
  configStore.updateHid({
    mouse_absolute: newMode === 'absolute',
  }).catch(_e => {
    console.info('[HidConfig] Failed to update mouse mode')
    toast.error(t('config.updateFailed'))
  })
}

// Update mouse throttle (real-time)
function handleThrottleChange(value: number[] | undefined) {
  if (!value || value.length === 0 || value[0] === undefined) return
  const throttleValue = clampMouseMoveSendIntervalMs(value[0])
  mouseThrottle.value = throttleValue
  // Save to localStorage
  localStorage.setItem('hidMouseThrottle', String(throttleValue))
  // Notify ConsoleView (storage event doesn't fire in same tab)
  window.dispatchEvent(new CustomEvent('hidMouseSendIntervalChanged', {
    detail: { intervalMs: throttleValue },
  }))
}

// Handle backend change
function handleBackendChange(backend: unknown) {
  if (typeof backend !== 'string') return
  if (backend === HidBackend.Otg || backend === HidBackend.Ch9329 || backend === HidBackend.None) {
    hidBackend.value = backend
  } else {
    return
  }

  // Clear device path when changing backend
  devicePath.value = ''

  // Auto-select first device if available
  if (availableDevicePaths.value.length > 0 && availableDevicePaths.value[0]) {
    devicePath.value = availableDevicePaths.value[0].path
  }
}

// Handle device path change
function handleDevicePathChange(path: unknown) {
  if (typeof path !== 'string') return
  devicePath.value = path
}

// Handle baudrate change
function handleBaudrateChange(rate: unknown) {
  if (typeof rate !== 'string') return
  baudrate.value = Number(rate)
}

// Apply HID device configuration
async function applyHidConfig() {
  applying.value = true
  try {
    const config: HidConfigUpdate = {
      backend: hidBackend.value,
    }

    if (hidBackend.value === HidBackend.Ch9329) {
      config.ch9329_port = devicePath.value
      config.ch9329_baudrate = baudrate.value
    } else if (hidBackend.value === HidBackend.Otg) {
      config.otg_udc = devicePath.value
    }

    await configStore.updateHid(config)

    toast.success(t('config.applied'))

    // HID state will be updated via WebSocket device_info event
  } catch (e) {
    console.info('[HidConfig] Failed to apply config:', e)
    // Error toast already shown by API layer
  } finally {
    applying.value = false
  }
}

// Watch open state
watch(() => props.open, (isOpen) => {
  if (!isOpen) return

  // Load devices on first open
  if (serialDevices.value.length === 0) {
    loadDevices()
  }

  configStore.refreshHid()
    .then(() => {
      initializeFromCurrent()
    })
    .catch(() => {
      initializeFromCurrent()
    })
})
</script>

<template>
  <Popover :open="open" @update:open="emit('update:open', $event)">
    <PopoverTrigger as-child>
      <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
        <MousePointer v-if="mouseMode === 'absolute'" class="h-4 w-4" />
        <Move v-else class="h-4 w-4" />
        <span class="hidden sm:inline">{{ buttonText }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-[320px] p-3" align="start">
      <div class="space-y-3">
        <h4 class="text-sm font-medium">{{ t('actionbar.hidConfig') }}</h4>

        <Separator />

        <!-- Mouse Settings (Real-time) -->
        <div class="space-y-3">
          <h5 class="text-xs font-medium text-muted-foreground">{{ t('actionbar.mouseSettings') }}</h5>

          <!-- Positioning Mode -->
          <div class="space-y-2">
            <div class="flex items-center gap-1">
              <Label class="text-xs text-muted-foreground">{{ t('actionbar.positioningMode') }}</Label>
              <HelpTooltip :content="mouseMode === 'absolute' ? t('help.absoluteMode') : t('help.relativeMode')" icon-size="sm" />
            </div>
            <div class="flex gap-2">
              <Button
                :variant="mouseMode === 'absolute' ? 'default' : 'outline'"
                size="sm"
                class="flex-1 h-8 text-xs"
                @click="toggleMouseMode"
              >
                <MousePointer class="h-3.5 w-3.5 mr-1" />
                {{ t('actionbar.absolute') }}
              </Button>
              <Button
                :variant="mouseMode === 'relative' ? 'default' : 'outline'"
                size="sm"
                class="flex-1 h-8 text-xs"
                @click="toggleMouseMode"
              >
                <Move class="h-3.5 w-3.5 mr-1" />
                {{ t('actionbar.relative') }}
              </Button>
            </div>
          </div>

          <!-- Event Throttle -->
          <div class="space-y-2">
            <div class="flex justify-between items-center">
              <div class="flex items-center gap-1">
                <Label class="text-xs text-muted-foreground">{{ t('actionbar.sendInterval') }}</Label>
                <HelpTooltip :content="t('help.mouseThrottle')" icon-size="sm" />
              </div>
              <span class="text-xs font-mono">{{ mouseThrottle }}ms</span>
            </div>
            <Slider
              :model-value="[mouseThrottle]"
              @update:model-value="handleThrottleChange"
              :min="0"
              :max="1000"
              :step="1"
              class="py-2"
            />
            <div class="flex justify-between text-xs text-muted-foreground">
              <span>0ms</span>
              <span>1000ms</span>
            </div>
          </div>

          <!-- Show Cursor -->
          <div class="flex items-center justify-between">
            <Label class="text-xs text-muted-foreground">{{ t('actionbar.showCursor') }}</Label>
            <Switch v-model="showCursor" />
          </div>
        </div>

        <!-- HID Device Settings (Requires Apply) -->
        <Separator />

        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <h5 class="text-xs font-medium text-muted-foreground">{{ t('actionbar.hidDeviceSettings') }}</h5>
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

          <!-- Backend Type -->
          <div class="space-y-2">
            <Label class="text-xs text-muted-foreground">{{ t('actionbar.backend') }}</Label>
            <Select
              :model-value="hidBackend"
              @update:model-value="handleBackendChange"
            >
              <SelectTrigger class="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="HidBackend.Otg" class="text-xs">USB OTG</SelectItem>
                <SelectItem :value="HidBackend.Ch9329" class="text-xs">CH9329 (Serial)</SelectItem>
                <SelectItem :value="HidBackend.None" class="text-xs">{{ t('common.disabled') }}</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- Device Path (OTG or CH9329) -->
          <div v-if="hidBackend !== HidBackend.None" class="space-y-2">
            <Label class="text-xs text-muted-foreground">{{ t('actionbar.devicePath') }}</Label>
            <Select
              :model-value="devicePath"
              @update:model-value="handleDevicePathChange"
              :disabled="availableDevicePaths.length === 0"
            >
              <SelectTrigger class="h-8 text-xs">
                <SelectValue :placeholder="t('actionbar.selectDevice')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="device in availableDevicePaths"
                  :key="device.path"
                  :value="device.path"
                  class="text-xs"
                >
                  {{ device.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- Baudrate (CH9329 only) -->
          <div v-if="hidBackend === HidBackend.Ch9329" class="space-y-2">
            <Label class="text-xs text-muted-foreground">{{ t('actionbar.baudrate') }}</Label>
            <Select
              :model-value="String(baudrate)"
              @update:model-value="handleBaudrateChange"
            >
              <SelectTrigger class="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="9600" class="text-xs">9600</SelectItem>
                <SelectItem value="19200" class="text-xs">19200</SelectItem>
                <SelectItem value="38400" class="text-xs">38400</SelectItem>
                <SelectItem value="57600" class="text-xs">57600</SelectItem>
                <SelectItem value="115200" class="text-xs">115200</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- Apply Button -->
          <Button
            class="w-full h-8 text-xs"
            :disabled="applying"
            @click="applyHidConfig"
          >
            <Loader2 v-if="applying" class="h-3.5 w-3.5 mr-1.5 animate-spin" />
            <span>{{ applying ? t('actionbar.applying') : t('common.apply') }}</span>
          </Button>
          </div>
      </div>
    </PopoverContent>
  </Popover>
</template>
