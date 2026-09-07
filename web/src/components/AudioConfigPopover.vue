<script setup lang="ts">
import { focusConsolePanel } from "@/composables/useConsoleAppearance"
import { computed, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Loader2, RefreshCw, Volume2, VolumeX } from 'lucide-vue-next'

import { audioApi, configApi } from '@/api'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Separator } from '@/components/ui/separator'
import { Slider } from '@/components/ui/slider'
import MicrophoneTransferControls from '@/components/MicrophoneTransferControls.vue'
import { getMicrophone } from '@/composables/useMicrophone'
import { getUnifiedAudio } from '@/composables/useUnifiedAudio'
import { useConfigStore } from '@/stores/config'
import { useSystemStore } from '@/stores/system'

interface AudioDevice {
  name: string
  description: string
}

const props = defineProps<{
  open: boolean
  microphoneEnabled?: boolean
  side?: 'top' | 'right' | 'bottom' | 'left'
}>()

const emit = defineEmits<{
  (event: 'update:open', value: boolean): void
}>()

const { t } = useI18n()
const configStore = useConfigStore()
const systemStore = useSystemStore()
const unifiedAudio = getUnifiedAudio()
const microphone = getMicrophone()
const playbackMuted = computed(() => unifiedAudio.muted.value || unifiedAudio.volume.value === 0)

const localVolume = ref([unifiedAudio.volume.value * 100])
const devices = ref<AudioDevice[]>([])
const loadingDevices = ref(false)
const applying = ref(false)
const audioEnabled = ref(false)
const selectedDevice = ref('')
const selectedQuality = ref<'voice' | 'balanced' | 'high'>('balanced')
const EMPTY_SELECT_VALUE = '__one-kvm-empty-select-value__'

async function handleVolumeChange(value: number[] | undefined) {
  if (!value || value.length === 0 || value[0] === undefined) return

  const newVolume = value[0] / 100
  unifiedAudio.setVolume(newVolume)
  localVolume.value = value

  if (newVolume > 0 && systemStore.audio?.streaming && !unifiedAudio.connected.value) {
    try {
      await unifiedAudio.connect()
    } catch (error) {
      console.info('[Audio] Connect failed:', error)
    }
  }
}

async function loadDevices() {
  loadingDevices.value = true
  try {
    const result = await configApi.listDevices()
    devices.value = result.audio
  } catch {
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
      if (localVolume.value[0] === 0) {
        localVolume.value = [100]
        unifiedAudio.setVolume(1)
      }

      await audioApi.start()
    } else if (!audioEnabled.value) {
      localVolume.value = [0]
      unifiedAudio.setVolume(0)
      await audioApi.stop()
      unifiedAudio.disconnect()
    }

    toast.success(t('common.success'))
  } catch (error) {
    toast.error(t('common.error'), {
      description: error instanceof Error ? error.message : String(error),
    })
  } finally {
    applying.value = false
  }
}

watch(() => props.open, isOpen => {
  if (!isOpen) return

  if (devices.value.length === 0) {
    void loadDevices()
  }
  if (props.microphoneEnabled) {
    void microphone.refreshInputDevices()
  }

  configStore.refreshAudio()
    .then(initializeFromCurrent)
    .catch(initializeFromCurrent)
})

onUnmounted(() => {
  void microphone.stop()
})
</script>

<template>
  <Popover :open="open" @update:open="emit('update:open', $event)">
    <PopoverTrigger as-child>
      <Button
        variant="ghost"
        size="sm"
        class="size-8 p-0 text-xs sm:w-auto sm:gap-1.5 sm:px-2"
        :aria-label="playbackMuted ? `${t('actionbar.audioConfig')} · ${t('actionbar.muted')}` : t('actionbar.audioConfig')"
        :title="playbackMuted ? `${t('actionbar.audioConfig')} · ${t('actionbar.muted')}` : t('actionbar.audioConfig')"
      >
        <VolumeX v-if="playbackMuted" class="size-3.5 sm:size-4" />
        <Volume2 v-else class="size-3.5 sm:size-4" />
        <span class="hidden sm:inline">{{ t('actionbar.audioConfig') }}</span>
      </Button>
    </PopoverTrigger>

    <PopoverContent
      @open-auto-focus="focusConsolePanel"
      class="console-config-panel w-[min(320px,92vw)] p-3"
      align="start"
      :side="props.side ?? 'bottom'"
    >
      <div class="space-y-3">
        <h4 class="text-sm font-medium">{{ t('actionbar.audioConfig') }}</h4>

        <Separator />

        <template v-if="props.microphoneEnabled">
          <MicrophoneTransferControls />
          <Separator />
        </template>

        <!-- Playback volume and capture configuration form one section. -->
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <h5 class="text-xs font-medium text-muted-foreground">
              {{ t('actionbar.playbackControl') }}
            </h5>
            <Button
              variant="ghost"
              size="icon-xs"
              :disabled="loadingDevices"
              @click="loadDevices"
            >
              <RefreshCw :class="['size-3.5', loadingDevices && 'animate-spin']" />
            </Button>
          </div>

          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <Label class="text-xs text-muted-foreground">{{ t('actionbar.volume') }}</Label>
              <span class="font-mono text-xs">{{ Math.round(localVolume[0] ?? 0) }}%</span>
            </div>
            <div class="flex items-center gap-2">
              <Volume2 class="size-3.5 text-muted-foreground opacity-50" />
              <Slider
                :model-value="localVolume"
                :min="0"
                :max="100"
                :step="1"
                :disabled="!systemStore.audio?.streaming"
                class="flex-1"
                @update:model-value="handleVolumeChange"
              />
              <Volume2 class="size-3.5 text-muted-foreground" />
            </div>
          </div>

          <div class="space-y-2">
            <Label class="text-xs text-muted-foreground">{{ t('actionbar.audioEnabled') }}</Label>
            <div class="flex gap-2">
              <Button
                :variant="audioEnabled ? 'default' : 'outline'"
                size="sm"
                class="flex-1 text-xs"
                @click="audioEnabled = true"
              >
                {{ t('common.enabled') }}
              </Button>
              <Button
                :variant="!audioEnabled ? 'default' : 'outline'"
                size="sm"
                class="flex-1 text-xs"
                @click="audioEnabled = false"
              >
                {{ t('common.disabled') }}
              </Button>
            </div>
          </div>

          <div class="space-y-2">
            <Label class="text-xs text-muted-foreground">{{ t('actionbar.audioDevice') }}</Label>
            <Select
              :model-value="selectedDevice"
              :disabled="loadingDevices || devices.length === 0"
              @update:model-value="selectedDevice = $event === EMPTY_SELECT_VALUE ? '' : String($event)"
            >
              <SelectTrigger size="sm" class="w-full text-xs">
                <SelectValue :placeholder="t('actionbar.selectAudioDevice')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="EMPTY_SELECT_VALUE" class="text-xs">{{ t('actionbar.selectAudioDevice') }}</SelectItem>
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

          <div class="space-y-2">
            <Label class="text-xs text-muted-foreground">{{ t('actionbar.audioQuality') }}</Label>
            <div class="flex gap-1">
              <Button
                :variant="selectedQuality === 'voice' ? 'default' : 'outline'"
                size="sm"
                class="flex-1 text-xs"
                @click="selectedQuality = 'voice'"
              >
                {{ t('actionbar.qualityVoice') }} 32k
              </Button>
              <Button
                :variant="selectedQuality === 'balanced' ? 'default' : 'outline'"
                size="sm"
                class="flex-1 text-xs"
                @click="selectedQuality = 'balanced'"
              >
                {{ t('actionbar.qualityBalanced') }} 64k
              </Button>
              <Button
                :variant="selectedQuality === 'high' ? 'default' : 'outline'"
                size="sm"
                class="flex-1 text-xs"
                @click="selectedQuality = 'high'"
              >
                {{ t('actionbar.qualityHigh') }} 128k
              </Button>
            </div>
          </div>

          <Button
            size="sm"
            class="w-full text-xs"
            :disabled="applying"
            @click="applyConfig"
          >
            <Loader2 v-if="applying" class="size-3.5 animate-spin" />
            {{ applying ? t('actionbar.applying') : t('common.apply') }}
          </Button>
        </div>
      </div>
    </PopoverContent>
  </Popover>
</template>
