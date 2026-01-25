<script setup lang="ts">
import { ref, computed, onMounted, watch, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { configApi, streamApi, type EncoderBackendInfo } from '@/api'
import {
  supportedLanguages,
  setLanguage,
  getCurrentLanguage,
  type SupportedLocale,
} from '@/i18n'
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
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from '@/components/ui/hover-card'
import { Switch } from '@/components/ui/switch'
import {
  Monitor,
  Eye,
  EyeOff,
  ChevronRight,
  ChevronLeft,
  User,
  Video,
  Keyboard,
  Check,
  HelpCircle,
  Languages,
  Puzzle,
} from 'lucide-vue-next'

const { t } = useI18n()
const router = useRouter()
const authStore = useAuthStore()

// Language switcher
const currentLanguage = ref<SupportedLocale>(getCurrentLanguage())

function switchLanguage(lang: SupportedLocale) {
  currentLanguage.value = lang
  setLanguage(lang)
}

// Steps: 1 = Account, 2 = Audio/Video, 3 = HID, 4 = Extensions
const step = ref(1)
const totalSteps = 4
const loading = ref(false)
const error = ref('')
const slideDirection = ref<'forward' | 'backward'>('forward')

// Account settings
const username = ref('')
const password = ref('')
const confirmPassword = ref('')
const showPassword = ref(false)

// Form validation states
const usernameError = ref('')
const passwordError = ref('')
const confirmPasswordError = ref('')
const usernameTouched = ref(false)
const passwordTouched = ref(false)
const confirmPasswordTouched = ref(false)

// Video settings
const videoDevice = ref('')
const videoFormat = ref('')
const videoResolution = ref('')
const videoFps = ref<number | null>(null)

// Audio settings
const audioDevice = ref('')
const audioEnabled = ref(true)

// HID settings
const hidBackend = ref('ch9329')
const ch9329Port = ref('')
const ch9329Baudrate = ref(9600)
const otgUdc = ref('')

// Extension settings
const ttydEnabled = ref(false)
const ttydAvailable = ref(false)

// Encoder backend settings
const encoderBackend = ref('auto')
const availableBackends = ref<EncoderBackendInfo[]>([])
const showAdvancedEncoder = ref(false)

// Device info from API
interface VideoDeviceInfo {
  path: string
  name: string
  driver: string
  formats: Array<{
    format: string
    description: string
    resolutions: Array<{
      width: number
      height: number
      fps: number[]
    }>
  }>
  usb_bus: string | null
}

interface AudioDeviceInfo {
  name: string
  description: string
  is_hdmi: boolean
  usb_bus: string | null
}

interface DeviceInfo {
  video: VideoDeviceInfo[]
  serial: Array<{ path: string; name: string }>
  audio: AudioDeviceInfo[]
  udc: Array<{ name: string }>
  extensions: {
    ttyd_available: boolean
  }
}

const devices = ref<DeviceInfo>({
  video: [],
  serial: [],
  audio: [],
  udc: [],
  extensions: {
    ttyd_available: false,
  },
})

// Password strength calculation
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
  const colors = ['bg-muted', 'bg-red-500', 'bg-orange-500', 'bg-yellow-500', 'bg-green-500']
  return colors[passwordStrength.value] || colors[0]
})

// Computed: available formats for selected video device
const availableFormats = computed(() => {
  const device = devices.value.video.find((d) => d.path === videoDevice.value)
  return device?.formats || []
})

// Computed: available resolutions for selected format
const availableResolutions = computed(() => {
  const format = availableFormats.value.find((f) => f.format === videoFormat.value)
  return format?.resolutions || []
})

// Computed: available FPS for selected resolution
const availableFps = computed(() => {
  const [width, height] = (videoResolution.value || '').split('x').map(Number)
  const resolution = availableResolutions.value.find(
    (r) => r.width === width && r.height === height
  )
  return resolution?.fps || []
})

// Common baud rates for CH9329
const baudRates = [9600, 19200, 38400, 57600, 115200]

// Step labels for the indicator
const stepLabels = computed(() => [
  t('setup.stepAccount'),
  t('setup.stepAudioVideo'),
  t('setup.stepHid'),
  t('setup.stepExtensions'),
])

// Real-time validation functions
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
  // Also validate confirm password if it was touched
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

// Watch video device change to auto-select first format and matching audio device
watch(videoDevice, (newDevice) => {
  videoFormat.value = ''
  videoResolution.value = ''
  videoFps.value = null
  if (availableFormats.value.length > 0) {
    // Prefer MJPEG if available
    const mjpeg = availableFormats.value.find((f) => f.format.toUpperCase().includes('MJPEG'))
    videoFormat.value = mjpeg?.format || availableFormats.value[0]?.format || ''
  }

  // Auto-select matching audio device based on USB bus
  if (newDevice && audioEnabled.value) {
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

// Watch format change to auto-select best resolution
watch(videoFormat, () => {
  videoResolution.value = ''
  videoFps.value = null
  if (availableResolutions.value.length > 0) {
    // Prefer 1080p if available, otherwise highest resolution
    const r1080 = availableResolutions.value.find((r) => r.width === 1920 && r.height === 1080)
    const r720 = availableResolutions.value.find((r) => r.width === 1280 && r.height === 720)
    const best = r1080 || r720 || availableResolutions.value[0]
    if (best) {
      videoResolution.value = `${best.width}x${best.height}`
    }
  }
})

// Watch resolution change to auto-select FPS
watch(videoResolution, () => {
  videoFps.value = null
  if (availableFps.value.length > 0) {
    // Prefer 30fps if available
    videoFps.value = availableFps.value.includes(30) ? 30 : availableFps.value[0] || null
  }
})

// Watch HID backend change to set defaults
watch(hidBackend, (newBackend) => {
  if (newBackend === 'ch9329' && !ch9329Port.value && devices.value.serial.length > 0) {
    ch9329Port.value = devices.value.serial[0]?.path || ''
  }
  if (newBackend === 'otg' && !otgUdc.value && devices.value.udc.length > 0) {
    otgUdc.value = devices.value.udc[0]?.name || ''
  }
})

onMounted(async () => {
  try {
    const result = await configApi.listDevices()
    devices.value = result

    // Auto-select first video device
    if (result.video.length > 0 && result.video[0]) {
      videoDevice.value = result.video[0].path
    }

    // Auto-select first serial device for CH9329
    if (result.serial.length > 0 && result.serial[0]) {
      ch9329Port.value = result.serial[0].path
    }

    // Auto-select first UDC for OTG
    if (result.udc.length > 0 && result.udc[0]) {
      otgUdc.value = result.udc[0].name
    }

    // Auto-select audio device if available (and no video device to trigger watch)
    if (result.audio.length > 0 && !audioDevice.value) {
      // Prefer HDMI audio device
      const hdmiAudio = result.audio.find((a) => a.is_hdmi)
      audioDevice.value = hdmiAudio?.name || result.audio[0]?.name || ''
    }

    // Set extension availability from devices API
    if (result.extensions) {
      ttydAvailable.value = result.extensions.ttyd_available
    }
  } catch {
    // Use defaults
  }

  // Load encoder backends
  try {
    const codecsResult = await streamApi.getCodecs()
    availableBackends.value = codecsResult.backends || []
  } catch {
    // Use defaults
  }

  // Add keyboard navigation
  document.addEventListener('keydown', handleKeyDown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeyDown)
})

function handleKeyDown(e: KeyboardEvent) {
  // Don't interfere with input fields
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
  // Trigger validation for all fields
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
  if (videoDevice.value && !videoFormat.value) {
    error.value = t('setup.selectFormat')
    return false
  }
  return true
}

function validateStep3(): boolean {
  if (hidBackend.value === 'ch9329' && !ch9329Port.value) {
    error.value = t('setup.selectSerialPort')
    return false
  }
  if (hidBackend.value === 'otg' && !otgUdc.value) {
    error.value = t('setup.selectUdc')
    return false
  }
  return true
}

function nextStep() {
  error.value = ''

  if (step.value === 1 && !validateStep1()) return
  if (step.value === 2 && !validateStep2()) return

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

  if (!validateStep3()) return

  loading.value = true

  // Parse resolution
  const [width, height] = (videoResolution.value || '').split('x').map(Number)

  const setupData: Parameters<typeof authStore.setup>[0] = {
    username: username.value,
    password: password.value,
  }

  // Video settings
  if (videoDevice.value) {
    setupData.video_device = videoDevice.value
  }
  if (videoFormat.value) {
    setupData.video_format = videoFormat.value
  }
  if (width && height) {
    setupData.video_width = width
    setupData.video_height = height
  }
  if (videoFps.value) {
    setupData.video_fps = videoFps.value
  }

  // HID settings
  setupData.hid_backend = hidBackend.value
  if (hidBackend.value === 'ch9329') {
    setupData.hid_ch9329_port = ch9329Port.value
    setupData.hid_ch9329_baudrate = ch9329Baudrate.value
  }
  if (hidBackend.value === 'otg' && otgUdc.value) {
    setupData.hid_otg_udc = otgUdc.value
  }

  // Encoder backend setting
  if (encoderBackend.value !== 'auto') {
    setupData.encoder_backend = encoderBackend.value
  }

  // Audio settings
  if (audioDevice.value && audioDevice.value !== '__none__') {
    setupData.audio_device = audioDevice.value
  }

  // Extension settings
  setupData.ttyd_enabled = ttydEnabled.value

  const success = await authStore.setup(setupData)

  if (success) {
    // Auto login after setup
    await authStore.login(username.value, password.value)
    router.push('/')
  } else {
    error.value = authStore.error || t('setup.setupFailed')
  }

  loading.value = false
}

// Step icon component helper
const stepIcons = [User, Video, Keyboard, Puzzle]
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-background p-4">
    <Card class="w-full max-w-lg relative">
      <!-- Language Switcher -->
      <div class="absolute top-4 right-4">
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="sm" class="gap-2">
              <Languages class="w-4 h-4" />
              <span class="text-sm">
                {{ supportedLanguages.find((l) => l.code === currentLanguage)?.name }}
              </span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              v-for="lang in supportedLanguages"
              :key="lang.code"
              :class="{ 'bg-accent': lang.code === currentLanguage }"
              @click="switchLanguage(lang.code)"
            >
              <span class="mr-2">{{ lang.flag }}</span>
              {{ lang.name }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <CardHeader class="text-center space-y-2 pt-12">
        <div
          class="inline-flex items-center justify-center w-16 h-16 mx-auto rounded-full bg-primary/10"
        >
          <Monitor class="w-8 h-8 text-primary" />
        </div>
        <CardTitle class="text-2xl">{{ t('setup.welcome') }}</CardTitle>
        <CardDescription>{{ t('setup.description') }}</CardDescription>
      </CardHeader>

      <CardContent class="space-y-6">
        <!-- Progress Text -->
        <p class="text-sm text-muted-foreground text-center">
          {{ t('setup.progress', { current: step, total: totalSteps }) }}
        </p>

        <!-- Step Indicator with Labels -->
        <div class="flex items-center justify-center gap-2 mb-6">
          <template v-for="i in totalSteps" :key="i">
            <div class="flex flex-col items-center gap-1">
              <div
                class="flex items-center justify-center w-10 h-10 rounded-full border-2 transition-all duration-300"
                :class="
                  step > i
                    ? 'bg-primary border-primary text-primary-foreground scale-100'
                    : step === i
                      ? 'border-primary text-primary scale-110'
                      : 'border-muted text-muted-foreground scale-100'
                "
              >
                <Check v-if="step > i" class="w-5 h-5" />
                <component :is="stepIcons[i - 1]" v-else class="w-5 h-5" />
              </div>
              <span
                class="text-xs transition-colors duration-300 max-w-16 text-center leading-tight"
                :class="step >= i ? 'text-foreground font-medium' : 'text-muted-foreground'"
              >
                {{ stepLabels[i - 1] }}
              </span>
            </div>
            <div
              v-if="i < totalSteps"
              class="w-8 h-0.5 transition-colors duration-300 mb-6"
              :class="step > i ? 'bg-primary' : 'bg-muted'"
            />
          </template>
        </div>

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
                  :placeholder="t('setup.passwordHint')"
                  class="pr-10"
                  :class="{ 'border-destructive focus-visible:ring-destructive': passwordError }"
                  @blur="validatePassword"
                  @input="passwordTouched && validatePassword()"
                />
                <button
                  type="button"
                  class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                  @click="showPassword = !showPassword"
                >
                  <Eye v-if="!showPassword" class="w-4 h-4" />
                  <EyeOff v-else class="w-4 h-4" />
                </button>
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
                    <button type="button" class="text-muted-foreground hover:text-foreground transition-colors">
                      <HelpCircle class="w-4 h-4" />
                    </button>
                  </HoverCardTrigger>
                  <HoverCardContent class="w-64 text-sm">
                    {{ t('setup.videoDeviceHelp') }}
                  </HoverCardContent>
                </HoverCard>
              </div>
              <Select v-model="videoDevice">
                <SelectTrigger>
                  <SelectValue :placeholder="t('setup.selectVideoDevice')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="dev in devices.video" :key="dev.path" :value="dev.path">
                    {{ dev.name }} ({{ dev.path }})
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div v-if="videoDevice" class="space-y-2">
              <div class="flex items-center gap-2">
                <Label for="videoFormat">{{ t('setup.videoFormat') }}</Label>
                <HoverCard>
                  <HoverCardTrigger as-child>
                    <button type="button" class="text-muted-foreground hover:text-foreground transition-colors">
                      <HelpCircle class="w-4 h-4" />
                    </button>
                  </HoverCardTrigger>
                  <HoverCardContent class="w-64 text-sm">
                    {{ t('setup.videoFormatHelp') }}
                  </HoverCardContent>
                </HoverCard>
              </div>
              <Select v-model="videoFormat">
                <SelectTrigger>
                  <SelectValue :placeholder="t('setup.selectFormat')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="fmt in availableFormats" :key="fmt.format" :value="fmt.format">
                    {{ fmt.format }} - {{ fmt.description }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div v-if="videoFormat" class="grid grid-cols-2 gap-4">
              <div class="space-y-2">
                <Label for="videoResolution">{{ t('setup.resolution') }}</Label>
                <Select v-model="videoResolution">
                  <SelectTrigger>
                    <SelectValue :placeholder="t('setup.selectResolution')" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="res in availableResolutions"
                      :key="`${res.width}x${res.height}`"
                      :value="`${res.width}x${res.height}`"
                    >
                      {{ res.width }}x{{ res.height }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div class="space-y-2">
                <Label for="videoFps">{{ t('setup.fps') }}</Label>
                <Select v-model="videoFps">
                  <SelectTrigger>
                    <SelectValue :placeholder="t('setup.selectFps')" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="fps in availableFps" :key="fps" :value="fps">
                      {{ fps }} FPS
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <p v-if="!devices.video.length" class="text-sm text-muted-foreground text-center py-4">
              {{ t('setup.noVideoDevices') }}
            </p>

            <!-- Audio Device Selection -->
            <div class="space-y-2 pt-2 border-t">
              <div class="flex items-center gap-2">
                <Label for="audioDevice">{{ t('setup.audioDevice') }}</Label>
                <HoverCard>
                  <HoverCardTrigger as-child>
                    <button type="button" class="text-muted-foreground hover:text-foreground transition-colors">
                      <HelpCircle class="w-4 h-4" />
                    </button>
                  </HoverCardTrigger>
                  <HoverCardContent class="w-64 text-sm">
                    {{ t('setup.audioDeviceHelp') }}
                  </HoverCardContent>
                </HoverCard>
              </div>
              <Select v-model="audioDevice" :disabled="!audioEnabled">
                <SelectTrigger>
                  <SelectValue :placeholder="t('setup.selectAudioDevice')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none__">{{ t('setup.noAudio') }}</SelectItem>
                  <SelectItem v-for="dev in devices.audio" :key="dev.name" :value="dev.name">
                    {{ dev.description }}
                    <span v-if="dev.is_hdmi" class="text-xs text-muted-foreground ml-1">(HDMI)</span>
                  </SelectItem>
                </SelectContent>
              </Select>
              <p v-if="!devices.audio.length" class="text-xs text-muted-foreground">
                {{ t('setup.noAudioDevices') }}
              </p>
            </div>

            <!-- Advanced: Encoder Backend (Collapsible) -->
            <div class="mt-4 border rounded-lg">
              <button
                type="button"
                class="w-full flex items-center justify-between p-3 text-left hover:bg-muted/50 rounded-lg transition-colors"
                @click="showAdvancedEncoder = !showAdvancedEncoder"
              >
                <span class="text-sm font-medium">
                  {{ t('setup.advancedEncoder') }} ({{ t('common.optional') }})
                </span>
                <ChevronRight
                  class="h-4 w-4 transition-transform duration-200"
                  :class="{ 'rotate-90': showAdvancedEncoder }"
                />
              </button>
              <div v-if="showAdvancedEncoder" class="px-3 pb-3 space-y-3">
                <p class="text-xs text-muted-foreground">
                  {{ t('setup.encoderHint') }}
                </p>
                <Select v-model="encoderBackend">
                  <SelectTrigger>
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

            <div class="space-y-2">
              <Label for="hidBackend">{{ t('setup.hidBackend') }}</Label>
              <Select v-model="hidBackend">
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="ch9329">
                    CH9329 ({{ t('setup.serialHid') }})
                  </SelectItem>
                  <SelectItem value="otg">USB OTG</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <!-- CH9329 Settings -->
            <div v-if="hidBackend === 'ch9329'" class="space-y-4 p-4 rounded-lg bg-muted/50">
              <div class="flex items-start gap-2 text-sm text-muted-foreground mb-2">
                <HelpCircle class="w-4 h-4 mt-0.5 shrink-0" />
                <p>{{ t('setup.ch9329Help') }}</p>
              </div>

              <div class="space-y-2">
                <Label for="ch9329Port">{{ t('setup.serialPort') }}</Label>
                <Select v-model="ch9329Port">
                  <SelectTrigger>
                    <SelectValue :placeholder="t('setup.selectSerialPort')" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="port in devices.serial" :key="port.path" :value="port.path">
                      {{ port.name }} ({{ port.path }})
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p v-if="!devices.serial.length" class="text-xs text-muted-foreground">
                  {{ t('setup.noSerialDevices') }}
                </p>
              </div>

              <div class="space-y-2">
                <Label for="ch9329Baudrate">{{ t('setup.baudRate') }}</Label>
                <Select v-model="ch9329Baudrate">
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="rate in baudRates" :key="rate" :value="rate">
                      {{ rate }} bps
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <!-- OTG Settings -->
            <div v-if="hidBackend === 'otg'" class="space-y-4 p-4 rounded-lg bg-muted/50">
              <div class="flex items-start gap-2 text-sm text-muted-foreground mb-2">
                <HelpCircle class="w-4 h-4 mt-0.5 shrink-0" />
                <p>{{ t('setup.otgHelp') }}</p>
              </div>

              <div class="space-y-2">
                <Label for="otgUdc">{{ t('setup.udc') }}</Label>
                <Select v-model="otgUdc">
                  <SelectTrigger>
                    <SelectValue :placeholder="t('setup.selectUdc')" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="udc in devices.udc" :key="udc.name" :value="udc.name">
                      {{ udc.name }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p v-if="!devices.udc.length" class="text-xs text-muted-foreground">
                  {{ t('setup.noUdcDevices') }}
                </p>
              </div>
            </div>
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
          <p v-if="error" class="text-sm text-destructive text-center">{{ error }}</p>
        </Transition>

        <!-- Navigation Buttons -->
        <div class="flex gap-3">
          <Button v-if="step > 1" variant="outline" class="flex-1" @click="prevStep">
            <ChevronLeft class="w-4 h-4 mr-2" />
            {{ t('common.back') }}
          </Button>

          <Button v-if="step < totalSteps" class="flex-1" @click="nextStep">
            {{ t('common.next') }}
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
          {{ t('common.next') }}
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
