<script setup lang="ts">
import { focusConsolePanel } from "@/composables/useConsoleAppearance"
import { ref, computed, watch, nextTick } from 'vue'
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
import { MousePointer, Move } from 'lucide-vue-next'
import HelpTooltip from '@/components/HelpTooltip.vue'
import HidDeviceOverview from '@/components/HidDeviceOverview.vue'
import HidDriverDialog from '@/components/HidDriverDialog.vue'
import { useHidConnection } from '@/composables/useHidConnection'
import { useConfigStore } from '@/stores/config'
import { HidBackend } from '@/types/generated'

const props = defineProps<{
  open: boolean
  mouseMode?: 'absolute' | 'relative'
  side?: 'top' | 'right' | 'bottom' | 'left'
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

const mouseThrottle = ref<number>(
  loadMouseMoveSendIntervalFromStorage()
)
const showCursor = ref<boolean>(
  localStorage.getItem('hidShowCursor') !== 'false' // default true
)

watch(showCursor, (newValue, oldValue) => {
  if (newValue !== oldValue) {
    localStorage.setItem('hidShowCursor', newValue ? 'true' : 'false')
    window.dispatchEvent(new CustomEvent('hidCursorVisibilityChanged', {
      detail: { visible: newValue }
    }))
  }
})

const guideOpen = ref(false)
const buttonText = computed(() => t('actionbar.hidConfig'))
const { status, bluetooth, error } = useHidConnection(computed(() => props.open && !guideOpen.value), computed(() => configStore.hid?.backend))
async function configure() {
  emit('update:open', false)
  await nextTick()
  guideOpen.value = true
}

function toggleMouseMode() {
  if (configStore.hid?.backend === HidBackend.Bluetooth) return
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

function handleThrottleChange(value: number[] | undefined) {
  if (!value || value.length === 0 || value[0] === undefined) return
  const throttleValue = clampMouseMoveSendIntervalMs(value[0])
  mouseThrottle.value = throttleValue
  localStorage.setItem('hidMouseThrottle', String(throttleValue))
  window.dispatchEvent(new CustomEvent('hidMouseSendIntervalChanged', {
    detail: { intervalMs: throttleValue },
  }))
}

watch(() => props.open, (open) => {
  if (!open) return
  mouseThrottle.value = loadMouseMoveSendIntervalFromStorage()
  showCursor.value = localStorage.getItem('hidShowCursor') !== 'false'
  void configStore.refreshHid().catch(() => undefined)
})
</script>

<template>
  <Popover :open="open" @update:open="emit('update:open', $event)">
    <PopoverTrigger as-child>
      <Button
        variant="ghost"
        size="sm"
        class="size-8 sm:w-auto p-0 sm:px-2 sm:gap-1.5 text-xs"
        :aria-label="buttonText"
        :title="buttonText"
      >
        <MousePointer v-if="mouseMode === 'absolute'" class="size-3.5 sm:size-4" />
        <Move v-else class="size-3.5 sm:size-4" />
        <span class="hidden sm:inline">{{ buttonText }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent
      @open-auto-focus="focusConsolePanel"
      class="console-config-panel w-[min(320px,92vw)] p-3"
      align="start"
      :side="props.side ?? 'bottom'"
    >
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
                :disabled="configStore.hid?.backend === HidBackend.Bluetooth"
                size="sm"
                class="flex-1 h-8 text-xs"
                @click="toggleMouseMode"
              >
                <MousePointer class="size-3.5 mr-1" />
                {{ t('actionbar.absolute') }}
              </Button>
              <Button
                :variant="mouseMode === 'relative' ? 'default' : 'outline'"
                size="sm"
                class="flex-1 h-8 text-xs"
                @click="toggleMouseMode"
              >
                <Move class="size-3.5 mr-1" />
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

        <Separator />
        <HidDeviceOverview :hid="configStore.hid" :status="status" :bluetooth="bluetooth" :error="error" />
        <Button variant="outline" class="w-full" @click="configure">{{ t('hidGuide.reconfigure') }}</Button>
      </div>
    </PopoverContent>
  </Popover>
  <HidDriverDialog v-if="guideOpen" @close="guideOpen = false" />
</template>
