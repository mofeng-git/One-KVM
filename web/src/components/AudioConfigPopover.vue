<script setup lang="ts">
import { ref, watch } from 'vue'
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
import { Volume2, RefreshCw, Loader2 } from 'lucide-vue-next'
import { audioApi, configApi } from '@/api'
import { useConfigStore } from '@/stores/config'
import { useSystemStore } from '@/stores/system'
import { getUnifiedAudio } from '@/composables/useUnifiedAudio'

interface AudioDevice {
  name: string
  description: string
}

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const { t } = useI18n()
const configStore = useConfigStore()
const systemStore = useSystemStore()
const unifiedAudio = getUnifiedAudio()

const localVolume = ref([unifiedAudio.volume.value * 100])

async function handleVolumeChange(value: number[] | undefined) {
  if (!value || value.length === 0 || value[0] === undefined) return

  const newVolume = value[0] / 100
  unifiedAudio.setVolume(newVolume)
  localVolume.value = value

  if (newVolume > 0 && systemStore.audio?.streaming && !unifiedAudio.connected.value) {
    console.log('[Audio] User adjusted volume, connecting unified audio')
    try {
      await unifiedAudio.connect()
    } catch (e) {
      console.info('[Audio] Connect failed:', e)
    }
  }
}

const devices = ref<AudioDevice[]>([])
const loadingDevices = ref(false)
const applying = ref(false)

const audioEnabled = ref(false)
const selectedDevice = ref('')
const selectedQuality = ref<'voice' | 'balanced' | 'high'>('balanced')

async function loadDevices() {
  loadingDevices.value = true
  try {
    const result = await configApi.listDevices()
    devices.value = result.audio
  } catch (e) {
    console.info('[AudioConfig] Failed to load devices')
  } finally {
    loadingDevices.value = false
  }
}

function initializeFromCurrent() {
  const audio = configStore.audio
  if (audio) {
    audioEnabled.value = audio.enabled
    selectedDevice.value = audio.device || ''
    selectedQuality.value = (audio.quality as 'voice' | 'balanced' | 'high') || 'balanced'
  }

  localVolume.value = [unifiedAudio.volume.value * 100]
}

async function applyConfig() {
  applying.value = true

  try {
    await configStore.updateAudio({
      enabled: audioEnabled.value,
      device: selectedDevice.value,
      quality: selectedQuality.value,
    })

    if (audioEnabled.value && selectedDevice.value) {
      try {
        if (localVolume.value[0] === 0) {
          localVolume.value = [100]
          unifiedAudio.setVolume(1)
        }

        await audioApi.start()
      } catch (startError) {
        console.info('[AudioConfig] Audio start failed:', startError)
      }
    } else if (!audioEnabled.value) {
      localVolume.value = [0]
      unifiedAudio.setVolume(0)
      try {
        await audioApi.stop()
      } catch {
      }
      unifiedAudio.disconnect()
    }

    toast.success(t('config.applied'))
  } catch (e) {
    console.info('[AudioConfig] Failed to apply config:', e)
  } finally {
    applying.value = false
  }
}

watch(() => props.open, (isOpen) => {
  if (!isOpen) return

  if (devices.value.length === 0) {
    loadDevices()
  }

  configStore.refreshAudio()
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
      <Button variant="ghost" size="sm" class="h-7 w-7 sm:h-8 sm:w-auto p-0 sm:px-2 sm:gap-1.5 text-xs">
        <Volume2 class="h-3.5 w-3.5 sm:h-4 sm:w-4" />
        <span class="hidden sm:inline">{{ t('actionbar.audioConfig') }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-[min(320px,92vw)] p-3" align="start">
      <div class="space-y-3">
        <h4 class="text-sm font-medium">{{ t('actionbar.audioConfig') }}</h4>

        <Separator />

        <!-- Playback Control (immediate effect) -->
        <div class="space-y-3">
          <h5 class="text-xs font-medium text-muted-foreground">
            {{ t('actionbar.playbackControl') }}
          </h5>

          <!-- Volume -->
          <div class="space-y-2">
            <div class="flex justify-between items-center">
              <Label class="text-xs text-muted-foreground">{{ t('actionbar.volume') }}</Label>
              <span class="text-xs font-mono">{{ Math.round(localVolume[0] ?? 0) }}%</span>
            </div>
            <div class="flex items-center gap-2">
              <Volume2 class="h-3.5 w-3.5 text-muted-foreground opacity-50" />
              <Slider
                :model-value="localVolume"
                @update:model-value="handleVolumeChange"
                :min="0"
                :max="100"
                :step="1"
                :disabled="!systemStore.audio?.streaming"
                class="flex-1"
              />
              <Volume2 class="h-3.5 w-3.5 text-muted-foreground" />
            </div>
          </div>
        </div>

        <!-- Device Settings (requires apply) -->
        <Separator />

        <div class="space-y-3">
            <div class="flex items-center justify-between">
              <h5 class="text-xs font-medium text-muted-foreground">
                {{ t('actionbar.audioDeviceSettings') }}
              </h5>
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

            <!-- Enable Audio -->
            <div class="space-y-2">
              <Label class="text-xs text-muted-foreground">{{ t('actionbar.audioEnabled') }}</Label>
              <div class="flex gap-2">
                <Button
                  :variant="audioEnabled ? 'default' : 'outline'"
                  size="sm"
                  class="flex-1 h-8 text-xs"
                  @click="audioEnabled = true"
                >
                  {{ t('common.enabled') }}
                </Button>
                <Button
                  :variant="!audioEnabled ? 'default' : 'outline'"
                  size="sm"
                  class="flex-1 h-8 text-xs"
                  @click="audioEnabled = false"
                >
                  {{ t('common.disabled') }}
                </Button>
              </div>
            </div>

            <!-- Device Selection -->
            <div class="space-y-2">
              <Label class="text-xs text-muted-foreground">{{ t('actionbar.audioDevice') }}</Label>
              <Select
                :model-value="selectedDevice"
                @update:model-value="(v) => selectedDevice = v as string"
                :disabled="loadingDevices || devices.length === 0"
              >
                <SelectTrigger class="h-8 text-xs">
                  <SelectValue :placeholder="t('actionbar.selectAudioDevice')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="device in devices"
                    :key="device.name"
                    :value="device.name"
                    class="text-xs"
                  >
                    {{ device.description || device.name }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <!-- Audio Quality -->
            <div class="space-y-2">
              <Label class="text-xs text-muted-foreground">{{ t('actionbar.audioQuality') }}</Label>
              <div class="flex gap-1">
                <Button
                  :variant="selectedQuality === 'voice' ? 'default' : 'outline'"
                  size="sm"
                  class="flex-1 h-8 text-xs"
                  @click="selectedQuality = 'voice'"
                >
                  {{ t('actionbar.qualityVoice') }} 32k
                </Button>
                <Button
                  :variant="selectedQuality === 'balanced' ? 'default' : 'outline'"
                  size="sm"
                  class="flex-1 h-8 text-xs"
                  @click="selectedQuality = 'balanced'"
                >
                  {{ t('actionbar.qualityBalanced') }} 64k
                </Button>
                <Button
                  :variant="selectedQuality === 'high' ? 'default' : 'outline'"
                  size="sm"
                  class="flex-1 h-8 text-xs"
                  @click="selectedQuality = 'high'"
                >
                  {{ t('actionbar.qualityHigh') }} 128k
                </Button>
              </div>
            </div>

            <!-- Apply Button -->
            <Button
              class="w-full h-8 text-xs"
              :disabled="applying"
              @click="applyConfig"
            >
              <Loader2 v-if="applying" class="h-3.5 w-3.5 mr-1.5 animate-spin" />
              <span>{{ applying ? t('actionbar.applying') : t('common.apply') }}</span>
            </Button>
          </div>
      </div>
    </PopoverContent>
  </Popover>
</template>
