<script setup lang="ts">
import HidDriverForm from '@/components/HidDriverForm.vue'
import { selectionFrom, readPendingHid, writePendingHid } from '@/lib/hidGuide'

import { ref, computed, onMounted, watch, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { configApi, streamApi, type DeviceList, type EncoderBackendInfo, type PlatformCapabilities } from '@/api'
import { toConfigFps } from '@/lib/fps'
import { formatVideoDeviceLabel } from '@/lib/video-device-label'
import { useVideoDeviceConfiguration } from '@/composables/useVideoDeviceConfiguration'
import VideoInputFields from '@/components/VideoInputFields.vue'
import LanguageToggleButton from '@/components/LanguageToggleButton.vue'
import BrandMark from '@/components/BrandMark.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from '@/components/ui/hover-card'
import { Switch } from '@/components/ui/switch'
import { Stepper, StepperItem, StepperSeparator, StepperTitle, StepperTrigger } from '@/components/ui/stepper'
import { Alert, AlertDescription } from '@/components/ui/alert'
import {
  Eye,
  EyeOff,
  ChevronRight,
  ChevronLeft,
  User,
  Video,
  Keyboard,
  Check,
  HelpCircle,
  Puzzle,
  AlertTriangle,
} from 'lucide-vue-next'

const { t } = useI18n()
const router = useRouter()
const authStore = useAuthStore()

const step = ref(1)
const loading = ref(false)
const error = ref('')
const slideDirection = ref<'forward' | 'backward'>('forward')

const username = ref('')
const password = ref('')
const confirmPassword = ref('')
const showPassword = ref(false)

const usernameError = ref('')
const passwordError = ref('')
const confirmPasswordError = ref('')
const usernameTouched = ref(false)
const passwordTouched = ref(false)
const confirmPasswordTouched = ref(false)

const videoDevice = ref('')
const videoFormat = ref('')
const videoResolution = ref('')
const videoFps = ref<number | null>(null)

const audioDevice = ref('')
const audioEnabled = ref(true)
const platform = ref<PlatformCapabilities | null>(null)
const isWindows = computed(() => platform.value?.mode === 'windows')
const audioSupported = computed(() => platform.value?.audio.available ?? true)
const totalSteps = 4
const EMPTY_SELECT_VALUE = '__one-kvm-empty-select-value__'

const hidSelection = ref(readPendingHid()?.selection ?? selectionFrom())
const hidSelectionValid = ref(false)

const ttydEnabled = ref(false)
const ttydAvailable = ref(false)

// Encoder backend settings
const encoderBackend = ref('auto')
const availableBackends = ref<EncoderBackendInfo[]>([])
const showAdvancedEncoder = ref(false)

const devices = ref<DeviceList>({
  video: [],
  serial: [],
  audio: [],
  udc: [],
  extensions: {
    ttyd_available: false,
    rustdesk_available: false,
  },
})

const passwordStrength = computed(() => {
  const pwd = password.value
  if (!pwd) return 0
  let score = 0
  if (pwd.length >= 4) score++
  if (pwd.length >= 8) score++
  if (/[A-Z]/.test(pwd) && /[a-z]/.test(pwd)) score++
  if (/[0-9]/.test(pwd)) score++
  if (/[^A-Za-z0-9]/.test(pwd)) score++
  return Math.min(score, 4)
})

const passwordStrengthText = computed(() => {
  const levels = [
    '',
    t('setup.passwordWeak'),
    t('setup.passwordMedium'),
    t('setup.passwordStrong'),
    t('setup.passwordVeryStrong'),
  ]
  return levels[passwordStrength.value] || ''
})

const passwordStrengthColor = computed(() => {
  const colors = ['bg-muted', 'bg-destructive', 'bg-warning', 'bg-warning', 'bg-success']
  return colors[passwordStrength.value] || colors[0]
})

const videoConfiguration = useVideoDeviceConfiguration({
  devices: computed(() => devices.value.video),
  selection: {
    device: videoDevice,
    format: videoFormat,
    resolution: videoResolution,
    fps: videoFps,
  },
  active: computed(() => step.value === 2),
  preferredFormat: device =>
    device.formats.find(format => format.format.toUpperCase().includes('MJPEG'))?.format,
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

const stepLabels = computed(() => [
  t('setup.stepAccount'),
  t('setup.stepAudioVideo'),
  t('setup.stepHid'),
  t('setup.stepExtensions'),
])

function validateUsername() {
  usernameTouched.value = true
  if (username.value.length === 0) {
    usernameError.value = ''
  } else if (username.value.length < 2) {
    usernameError.value = t('setup.usernameHint')
  } else {
    usernameError.value = ''
  }
}

function validatePassword() {
  passwordTouched.value = true
  if (password.value.length === 0) {
    passwordError.value = ''
  } else if (password.value.length < 4) {
    passwordError.value = t('setup.passwordHint')
  } else {
    passwordError.value = ''
  }
  if (confirmPasswordTouched.value) {
    validateConfirmPassword()
  }
}

function validateConfirmPassword() {
  confirmPasswordTouched.value = true
  if (confirmPassword.value.length === 0) {
    confirmPasswordError.value = ''
  } else if (confirmPassword.value !== password.value) {
    confirmPasswordError.value = t('setup.passwordMismatch')
  } else {
    confirmPasswordError.value = ''
  }
}

// Match audio to the selected capture device's USB bus.
watch(videoDevice, (newDevice) => {
  // Auto-select matching audio device based on USB bus
  if (newDevice && audioEnabled.value && audioSupported.value) {
    const video = devices.value.video.find((d) => d.path === newDevice)
    if (video?.usb_bus) {
      // Find audio device on the same USB bus
      const matchedAudio = devices.value.audio.find(
        (a) => a.usb_bus && a.usb_bus === video.usb_bus
      )
      if (matchedAudio) {
        audioDevice.value = matchedAudio.name
        return
      }
    }
    // Fallback: select first HDMI audio device
    const hdmiAudio = devices.value.audio.find((a) => a.is_hdmi)
    if (hdmiAudio) {
      audioDevice.value = hdmiAudio.name
    } else if (devices.value.audio.length > 0 && devices.value.audio[0]) {
      audioDevice.value = devices.value.audio[0].name
    }
  }
})

onMounted(async () => {
  try {
    const status = await authStore.checkSetupStatus()
    platform.value = status.platform
    if (!audioSupported.value) {
      audioEnabled.value = false
      audioDevice.value = '__none__'
    }
  } catch {
  }

  try {
    const result = await configApi.listDevices()
    devices.value = result

    // Auto-select first video device
    if (result.video.length > 0 && result.video[0]) {
      videoDevice.value = result.video[0].path
    }

    // Auto-select audio device if available (and no video device to trigger watch)
    if (audioSupported.value && result.audio.length > 0 && !audioDevice.value) {
      // Prefer HDMI audio device
      const hdmiAudio = result.audio.find((a) => a.is_hdmi)
      audioDevice.value = hdmiAudio?.name || result.audio[0]?.name || ''
    }

    // Set extension availability from devices API
    if (result.extensions) {
      ttydAvailable.value = result.extensions.ttyd_available
    }
  } catch {
  }

  // Load encoder backends
  try {
    const codecsResult = await streamApi.getCodecs()
    availableBackends.value = codecsResult.backends || []
  } catch {
  }

  document.addEventListener('keydown', handleKeyDown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeyDown)
})

function handleKeyDown(e: KeyboardEvent) {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
    return
  }
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    if (step.value < totalSteps) {
      nextStep()
    } else {
      handleSetup()
    }
  }
  if (e.key === 'Escape' && step.value > 1) {
    e.preventDefault()
    prevStep()
  }
}

function validateStep1(): boolean {
  validateUsername()
  validatePassword()
  validateConfirmPassword()

  if (username.value.length < 2) {
    error.value = t('setup.usernameHint')
    return false
  }
  if (password.value.length < 4) {
    error.value = t('setup.passwordHint')
    return false
  }
  if (password.value !== confirmPassword.value) {
    error.value = t('setup.passwordMismatch')
    return false
  }
  return true
}

function validateStep2(): boolean {
  // Video settings are optional, but if device is selected, format should be too
  if (videoDevice.value && !isSourceFollowing.value && !videoFormat.value) {
    error.value = t('setup.selectFormat')
    return false
  }
  return true
}

function validateStep3(): boolean {
  if (!hidSelectionValid.value) {
    error.value = t('hidGuide.selectDevice')
    return false
  }
  writePendingHid({ selection: hidSelection.value, phase: 'selected' })
  return true
}

function nextStep() {
  error.value = ''

  if (step.value === 1 && !validateStep1()) return
  if (step.value === 2 && !validateStep2()) return
  if (step.value === 3 && !validateStep3()) return

  if (step.value < totalSteps) {
    slideDirection.value = 'forward'
    step.value++
  }
}

function prevStep() {
  error.value = ''
  if (step.value > 1) {
    slideDirection.value = 'backward'
    step.value--
  }
}

async function handleSetup() {
  error.value = ''

  if (!readPendingHid() || loading.value) return

  loading.value = true
  // Reconcile a previous timed-out account request before submitting again.
  try {
    const status = await authStore.checkSetupStatus()
    if (status.initialized) {
      loading.value = false
      await router.push('/login')
      return
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    loading.value = false
    return
  }

  const [width, height] = (videoResolution.value || '').split('x').map(Number)

  const setupData: Parameters<typeof authStore.setup>[0] = {
    username: username.value,
    password: password.value,
  }

  if (videoDevice.value) {
    setupData.video_device = videoDevice.value
  }
  if (!isSourceFollowing.value && videoFormat.value) {
    setupData.video_format = videoFormat.value
  }
  if (!isSourceFollowing.value && width && height) {
    setupData.video_width = width
    setupData.video_height = height
  }
  if (!isSourceFollowing.value && videoFps.value) {
    setupData.video_fps = toConfigFps(videoFps.value)
  }

  setupData.hid_backend = 'none'
  setupData.msd_enabled = false

  // Encoder backend setting
  if (encoderBackend.value !== 'auto') {
    setupData.encoder_backend = encoderBackend.value
  }

  if (audioSupported.value && audioDevice.value && audioDevice.value !== '__none__') {
    setupData.audio_device = audioDevice.value
  }

  setupData.ttyd_enabled = ttydEnabled.value

  const success = await authStore.setup(setupData)

  if (success) {
    const loggedIn = await authStore.login(username.value, password.value)
    router.push(loggedIn ? '/' : '/login')
  } else {
    error.value = authStore.error || t('setup.setupFailed')
  }

  loading.value = false
}

const stepIcons = [User, Video, Keyboard, Puzzle]
</script>

<template>
  <div class="min-h-screen min-h-dvh flex items-start sm:items-center justify-center dot-grid-bg px-4 py-6 sm:py-10">
    <Card class="w-full max-w-lg relative">
      <!-- Language Switcher -->
      <div class="absolute top-4 right-4">
        <LanguageToggleButton />
      </div>

      <CardHeader class="text-center space-y-2 pt-8">
        <div class="mx-auto flex justify-center">
          <BrandMark size="lg" />
        </div>
        <CardTitle class="text-xl">{{ t('setup.welcome') }}</CardTitle>
        <CardDescription>{{ t('setup.description') }}</CardDescription>
      </CardHeader>

      <CardContent class="space-y-5 sm:space-y-6">
        <!-- Progress Text -->
        <p class="text-sm text-muted-foreground text-center">
          {{ t('setup.progress', { current: step, total: totalSteps }) }}
        </p>

        <Stepper :model-value="step" class="mb-5 flex w-full items-start gap-1 sm:mb-6 sm:gap-2">
          <StepperItem
            v-for="i in totalSteps"
            :key="i"
            v-slot="{ state }"
            :step="i"
            class="relative flex w-full flex-col items-center justify-center"
          >
            <StepperSeparator
              v-if="i < totalSteps"
              class="absolute left-[calc(50%+18px)] right-[calc(-50%+8px)] top-5 h-0.5 rounded-full group-data-[state=completed]:bg-primary"
            />
            <StepperTrigger as-child>
              <Button
                type="button"
                size="icon"
                :variant="state === 'completed' || state === 'active' ? 'default' : 'outline'"
                class="z-10 size-9 shrink-0 rounded-full disabled:opacity-100 sm:size-10"
                :class="state === 'active' && 'ring-2 ring-ring ring-offset-2 ring-offset-background'"
                disabled
              >
                <Check v-if="state === 'completed'" class="size-4 sm:size-5" />
                <component :is="stepIcons[i - 1]" v-else class="size-4 sm:size-5" />
              </Button>
            </StepperTrigger>
            <StepperTitle class="mt-1 max-w-14 whitespace-normal text-center text-[10px] leading-tight sm:max-w-16 sm:text-xs" :class="state === 'active' ? 'text-foreground' : 'text-muted-foreground'">
              {{ stepLabels[i - 1] }}
            </StepperTitle>
          </StepperItem>
        </Stepper>

        <!-- Step Content with Animation -->
        <Transition :name="slideDirection === 'forward' ? 'slide-forward' : 'slide-backward'" mode="out-in">
          <!-- Step 1: Account Setup -->
          <div v-if="step === 1" key="step1" class="space-y-4">
            <h3 class="text-lg font-medium text-center">{{ t('setup.stepAccount') }}</h3>

            <div class="space-y-2">
              <Label for="username" :class="{ 'text-destructive': usernameError }">
                {{ t('setup.setUsername') }}
              </Label>
              <div class="relative">
                <User
                  class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground"
                />
                <Input
                  id="username"
                  v-model="username"
                  type="text"
                  autocomplete="username"
                  :placeholder="t('setup.usernameHint')"
                  class="pl-10"
                  :class="{ 'border-destructive focus-visible:ring-destructive': usernameError }"
                  @blur="validateUsername"
                  @input="usernameTouched && validateUsername()"
                />
              </div>
              <p v-if="usernameError" class="text-xs text-destructive">{{ usernameError }}</p>
            </div>

            <div class="space-y-2">
              <Label for="password" :class="{ 'text-destructive': passwordError }">
                {{ t('setup.setPassword') }}
              </Label>
              <div class="relative">
                <Input
                  id="password"
                  v-model="password"
                  :type="showPassword ? 'text' : 'password'"
                  autocomplete="new-password"
                  :placeholder="t('setup.passwordHint')"
                  class="pr-10"
                  :class="{ 'border-destructive focus-visible:ring-destructive': passwordError }"
                  @blur="validatePassword"
                  @input="passwordTouched && validatePassword()"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground"
                  :aria-label="showPassword ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
                  @click="showPassword = !showPassword"
                >
                  <Eye v-if="!showPassword" class="w-4 h-4" />
                  <EyeOff v-else class="w-4 h-4" />
                </Button>
              </div>
              <p v-if="passwordError" class="text-xs text-destructive">{{ passwordError }}</p>

              <!-- Password Strength Indicator -->
              <div v-if="password.length > 0" class="space-y-1">
                <div class="flex gap-1 h-1">
                  <div
                    v-for="i in 4"
                    :key="i"
                    class="flex-1 rounded-full transition-colors duration-300"
                    :class="i <= passwordStrength ? passwordStrengthColor : 'bg-muted'"
                  />
                </div>
                <p class="text-xs text-muted-foreground">
                  {{ t('setup.passwordStrength') }}: {{ passwordStrengthText }}
                </p>
              </div>
            </div>

            <div class="space-y-2">
              <Label for="confirmPassword" :class="{ 'text-destructive': confirmPasswordError }">
                {{ t('setup.confirmPassword') }}
              </Label>
              <Input
                id="confirmPassword"
                v-model="confirmPassword"
                :type="showPassword ? 'text' : 'password'"
                autocomplete="new-password"
                :placeholder="t('setup.confirmPassword')"
                :class="{ 'border-destructive focus-visible:ring-destructive': confirmPasswordError }"
                @blur="validateConfirmPassword"
                @input="confirmPasswordTouched && validateConfirmPassword()"
              />
              <p v-if="confirmPasswordError" class="text-xs text-destructive">{{ confirmPasswordError }}</p>
            </div>
          </div>

          <!-- Step 2: Audio/Video Settings -->
          <div v-else-if="step === 2" key="step2" class="space-y-4">
            <h3 class="text-lg font-medium text-center">{{ t('setup.stepAudioVideo') }}</h3>

            <div class="space-y-2">
              <div class="flex items-center gap-2">
                <Label for="videoDevice">{{ t('setup.videoDevice') }}</Label>
                <HoverCard>
                  <HoverCardTrigger as-child>
                    <Button type="button" variant="ghost" size="icon-xs" class="text-muted-foreground" :aria-label="t('common.info')">
                      <HelpCircle class="w-4 h-4" />
                    </Button>
                  </HoverCardTrigger>
                  <HoverCardContent class="w-64 text-sm">
                    {{ t('setup.videoDeviceHelp') }}
                  </HoverCardContent>
                </HoverCard>
              </div>
              <Select
                :model-value="videoDevice"
                @update:model-value="value => videoDevice = value === EMPTY_SELECT_VALUE ? '' : String(value)"
              >
                <SelectTrigger id="videoDevice" class="w-full">
                  <SelectValue :placeholder="t('setup.selectVideoDevice')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem :value="EMPTY_SELECT_VALUE">{{ t('setup.selectVideoDevice') }}</SelectItem>
                  <SelectItem v-for="dev in devices.video" :key="dev.path" :value="dev.path">
                    {{ formatVideoDeviceLabel(dev) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <VideoInputFields
              v-if="selectedDevice"
              :device="selectedDevice"
              :formats="availableFormats"
              :resolutions="availableResolutions"
              :fps-options="availableFps"
              :format="videoFormat"
              :resolution="videoResolution"
              :fps="videoFps"
              :refreshing="refreshingInputStatus"
              @update:format="videoFormat = $event"
              @update:resolution="videoResolution = $event"
              @update:fps="videoFps = $event"
              @refresh="refreshInputStatus"
            />

            <p v-if="!devices.video.length" class="text-sm text-muted-foreground text-center py-4">
              {{ t('setup.noVideoDevices') }}
            </p>

            <!-- Audio Device Selection -->
            <div v-if="!isWindows" class="space-y-2 pt-2 border-t">
              <div class="flex items-center gap-2">
                <Label for="audioDevice">{{ t('setup.audioDevice') }}</Label>
                <HoverCard>
                  <HoverCardTrigger as-child>
                    <Button type="button" variant="ghost" size="icon-xs" class="text-muted-foreground" :aria-label="t('common.info')">
                      <HelpCircle class="w-4 h-4" />
                    </Button>
                  </HoverCardTrigger>
                  <HoverCardContent class="w-64 text-sm">
                    {{ t('setup.audioDeviceHelp') }}
                  </HoverCardContent>
                </HoverCard>
              </div>
              <Select
                :model-value="audioDevice"
                :disabled="!audioEnabled"
                @update:model-value="value => audioDevice = value === EMPTY_SELECT_VALUE ? '' : String(value)"
              >
                <SelectTrigger id="audioDevice" class="w-full">
                  <SelectValue :placeholder="t('setup.selectAudioDevice')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem :value="EMPTY_SELECT_VALUE">{{ t('setup.selectAudioDevice') }}</SelectItem>
                  <SelectItem value="__none__">{{ t('setup.noAudio') }}</SelectItem>
                  <SelectItem v-for="dev in devices.audio" :key="dev.name" :value="dev.name">{{ dev.description }}{{ dev.is_hdmi ? ' (HDMI)' : '' }}</SelectItem>
                </SelectContent>
              </Select>
              <p v-if="!devices.audio.length" class="text-xs text-muted-foreground">
                {{ t('setup.noAudioDevices') }}
              </p>
            </div>

            <!-- Advanced: Encoder Backend (Collapsible) -->
            <div class="mt-4 border rounded-lg">
              <Button
                type="button"
                variant="ghost"
                class="h-auto w-full justify-between rounded-lg p-3 text-left"
                :aria-label="t('setup.advancedEncoder')"
                @click="showAdvancedEncoder = !showAdvancedEncoder"
              >
                <span class="text-sm font-medium">
                  {{ t('setup.advancedEncoder') }} ({{ t('common.optional') }})
                </span>
                <ChevronRight
                  class="size-4 transition-transform duration-200"
                  :class="{ 'rotate-90': showAdvancedEncoder }"
                />
              </Button>
              <div v-if="showAdvancedEncoder" class="px-3 pb-3 space-y-3">
                <p class="text-xs text-muted-foreground">
                  {{ t('setup.encoderHint') }}
                </p>
                <Select v-model="encoderBackend">
                  <SelectTrigger class="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="auto">{{ t('setup.autoRecommended') }}</SelectItem>
                    <SelectItem v-for="backend in availableBackends" :key="backend.id" :value="backend.id">
                      {{ backend.name }}
                      ({{ backend.is_hardware ? t('setup.hardware') : t('setup.software') }})
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </div>

          <!-- Step 3: HID Settings -->
          <div v-else-if="step === 3" key="step3" class="space-y-4">
            <h3 class="text-lg font-medium text-center">{{ t('setup.stepHid') }}</h3>

            <HidDriverForm v-model="hidSelection" @valid="hidSelectionValid = $event" />
          </div>

          <!-- Step 4: Extensions Settings -->
          <div v-else-if="step === 4" key="step4" class="space-y-4">
            <h3 class="text-lg font-medium text-center">{{ t('setup.stepExtensions') }}</h3>
            <p class="text-sm text-muted-foreground text-center">
              {{ t('setup.extensionsDescription') }}
            </p>

            <!-- ttyd -->
            <div class="flex items-center justify-between p-4 rounded-lg border" :class="{ 'opacity-50': !ttydAvailable }">
              <div class="space-y-1">
                <div class="flex items-center gap-2">
                  <Label class="text-base font-medium">{{ t('setup.ttydTitle') }}</Label>
                  <span v-if="!ttydAvailable" class="text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded">
                    {{ t('setup.notInstalled') }}
                  </span>
                </div>
                <p class="text-sm text-muted-foreground">
                  {{ t('setup.ttydDescription') }}
                </p>
              </div>
              <Switch v-model="ttydEnabled" :disabled="!ttydAvailable" />
            </div>

            <p class="text-xs text-muted-foreground text-center pt-2">
              {{ t('setup.extensionsHint') }}
            </p>
          </div>
        </Transition>

        <!-- Error Message -->
        <Transition name="fade">
          <Alert v-if="error" variant="destructive">
            <AlertTriangle />
            <AlertDescription>{{ error }}</AlertDescription>
          </Alert>
        </Transition>

        <!-- Navigation Buttons -->
        <div class="flex gap-3">
          <Button v-if="step > 1" variant="outline" class="flex-1" @click="prevStep">
            <ChevronLeft class="w-4 h-4 mr-2" />
            {{ t('common.back') }}
          </Button>

          <Button v-if="step < totalSteps" class="flex-1" :disabled="step === 3 && !hidSelectionValid" @click="nextStep">
            {{ t(step === 3 ? 'hidGuide.useConfiguration' : 'common.next') }}
            <ChevronRight class="w-4 h-4 ml-2" />
          </Button>

          <Button v-if="step === totalSteps" class="flex-1" :disabled="loading" @click="handleSetup">
            <span v-if="loading">{{ t('common.loading') }}</span>
            <span v-else>{{ t('setup.complete') }}</span>
          </Button>
        </div>

        <!-- Keyboard shortcuts hint -->
        <p class="text-xs text-muted-foreground text-center">
          <kbd class="px-1.5 py-0.5 bg-muted rounded text-xs">Enter</kbd>
          {{ t(step === 3 ? 'hidGuide.useConfiguration' : 'common.next') }}
          <span v-if="step > 1" class="ml-2">
            <kbd class="px-1.5 py-0.5 bg-muted rounded text-xs">Esc</kbd>
            {{ t('common.back') }}
          </span>
        </p>
      </CardContent>
    </Card>
  </div>
</template>

<style scoped>
/* Forward slide animation (going to next step) */
.slide-forward-enter-active,
.slide-forward-leave-active {
  transition: all 0.25s ease-out;
}

.slide-forward-enter-from {
  opacity: 0;
  transform: translateX(30px);
}

.slide-forward-leave-to {
  opacity: 0;
  transform: translateX(-30px);
}

/* Backward slide animation (going to previous step) */
.slide-backward-enter-active,
.slide-backward-leave-active {
  transition: all 0.25s ease-out;
}

.slide-backward-enter-from {
  opacity: 0;
  transform: translateX(-30px);
}

.slide-backward-leave-to {
  opacity: 0;
  transform: translateX(30px);
}

/* Fade animation for error messages */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
