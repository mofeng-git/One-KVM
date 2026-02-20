<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSystemStore } from '@/stores/system'
import { useConfigStore } from '@/stores/config'
import { useAuthStore } from '@/stores/auth'
import {
  authApi,
  configApi,
  hidApi,
  streamApi,
  atxConfigApi,
  extensionsApi,
  systemApi,
  updateApi,
  type EncoderBackendInfo,
  type AuthConfig,
  type RustDeskConfigResponse,
  type RustDeskStatusResponse,
  type RustDeskPasswordResponse,
  type RtspStatusResponse,
  type RtspConfigUpdate,
  type WebConfig,
  type UpdateOverviewResponse,
  type UpdateStatusResponse,
  type UpdateChannel,
} from '@/api'
import type {
  ExtensionsStatus,
  ExtensionStatus,
  AtxDriverType,
  ActiveLevel,
  AtxDevices,
  OtgHidProfile,
  OtgHidFunctions,
} from '@/types/generated'
import { setLanguage } from '@/i18n'
import { useClipboard } from '@/composables/useClipboard'
import AppLayout from '@/components/AppLayout.vue'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { Badge } from '@/components/ui/badge'
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Monitor,
  Keyboard,
  Info,
  Sun,
  Moon,
  Eye,
  EyeOff,
  Save,
  Check,
  HardDrive,
  Power,
  Server,
  Menu,
  Lock,
  User,
  RefreshCw,
  Terminal,
  Play,
  Square,
  ChevronRight,
  Plus,
  Trash2,
  ExternalLink,
  Copy,
  ScreenShare,
  Radio,
} from 'lucide-vue-next'

const { t, te, locale } = useI18n()
const systemStore = useSystemStore()
const configStore = useConfigStore()
const authStore = useAuthStore()

// Settings state
const activeSection = ref('appearance')
const mobileMenuOpen = ref(false)
const loading = ref(false)
const saved = ref(false)

// Navigation structure
const navGroups = computed(() => [
  {
    title: t('settings.system'),
    items: [
      { id: 'appearance', label: t('settings.appearance'), icon: Sun },
      { id: 'account', label: t('settings.account'), icon: User },
      { id: 'access', label: t('settings.access'), icon: Lock },
    ]
  },
  {
    title: t('settings.hardware'),
    items: [
      { id: 'video', label: t('settings.video'), icon: Monitor, status: config.value.video_device ? t('settings.configured') : null },
      { id: 'hid', label: t('settings.hid'), icon: Keyboard, status: config.value.hid_backend.toUpperCase() },
      ...(config.value.msd_enabled ? [{ id: 'msd', label: t('settings.msd'), icon: HardDrive }] : []),
      { id: 'atx', label: t('settings.atx'), icon: Power },
      { id: 'environment', label: t('settings.environment'), icon: Server },
    ]
  },
  {
    title: t('settings.extensions'),
    items: [
      { id: 'ext-ttyd', label: t('extensions.ttyd.title'), icon: Terminal },
      { id: 'ext-rustdesk', label: t('extensions.rustdesk.title'), icon: ScreenShare },
      { id: 'ext-rtsp', label: t('extensions.rtsp.title'), icon: Radio },
      { id: 'ext-remote-access', label: t('extensions.remoteAccess.title'), icon: ExternalLink },
    ]
  },
  {
    title: t('settings.other'),
    items: [
      { id: 'about', label: t('settings.about'), icon: Info },
    ]
  }
])

function selectSection(id: string) {
  activeSection.value = id
  mobileMenuOpen.value = false
}

// Theme
const theme = ref<'light' | 'dark' | 'system'>('system')

// Account settings
const usernameInput = ref('')
const usernamePassword = ref('')
const usernameSaving = ref(false)
const usernameSaved = ref(false)
const usernameError = ref('')
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordSaving = ref(false)
const passwordSaved = ref(false)
const passwordError = ref('')
const showPasswords = ref(false)

// Auth config state
const authConfig = ref<AuthConfig>({
  session_timeout_secs: 3600 * 24,
  single_user_allow_multiple_sessions: false,
  totp_enabled: false,
  totp_secret: undefined,
})
const authConfigLoading = ref(false)

// Extensions management
const extensions = ref<ExtensionsStatus | null>(null)
const extensionsLoading = ref(false)
const extensionLogs = ref<Record<string, string[]>>({
  ttyd: [],
  gostc: [],
  easytier: [],
})
const showLogs = ref<Record<string, boolean>>({
  ttyd: false,
  gostc: false,
  easytier: false,
})

// Terminal dialog
const showTerminalDialog = ref(false)

// Extension config (local edit state)
const extConfig = ref({
  ttyd: { enabled: false, shell: '/bin/bash' },
  gostc: { enabled: false, addr: 'gostc.mofeng.run', key: '', tls: true },
  easytier: { enabled: false, network_name: '', network_secret: '', peer_urls: [] as string[], virtual_ip: '' },
})

// RustDesk config state
const rustdeskConfig = ref<RustDeskConfigResponse | null>(null)
const rustdeskStatus = ref<RustDeskStatusResponse | null>(null)
const rustdeskPassword = ref<RustDeskPasswordResponse | null>(null)
const rustdeskLoading = ref(false)
const rustdeskCopied = ref<'id' | 'password' | null>(null)
const { copy: clipboardCopy } = useClipboard()
const rustdeskLocalConfig = ref({
  enabled: false,
  rendezvous_server: '',
  relay_server: '',
  relay_key: '',
})

// RTSP config state
const rtspStatus = ref<RtspStatusResponse | null>(null)
const rtspLoading = ref(false)
const rtspLocalConfig = ref<RtspConfigUpdate & { password?: string }>({
  enabled: false,
  bind: '0.0.0.0',
  port: 8554,
  path: 'live',
  allow_one_client: true,
  codec: 'h264',
  username: '',
  password: '',
})
const rtspStreamUrl = computed(() => {
  const host = window.location.hostname || '127.0.0.1'
  const path = (rtspLocalConfig.value.path || 'live').trim().replace(/^\/+|\/+$/g, '') || 'live'
  const port = Number(rtspLocalConfig.value.port) || 8554
  return `rtsp://${host}:${port}/${path}`
})

// Web server config state
const webServerConfig = ref<WebConfig>({
  http_port: 8080,
  https_port: 8443,
  bind_address: '0.0.0.0',
  bind_addresses: ['0.0.0.0'],
  https_enabled: false,
})
const webServerLoading = ref(false)
const showRestartDialog = ref(false)
const restarting = ref(false)
const updateChannel = ref<UpdateChannel>('stable')
const updateOverview = ref<UpdateOverviewResponse | null>(null)
const updateStatus = ref<UpdateStatusResponse | null>(null)
const updateLoading = ref(false)
const updateSawRestarting = ref(false)
const updateSawRequestFailure = ref(false)
const updateAutoReloadTriggered = ref(false)
const updateRunning = computed(() => {
  const phase = updateStatus.value?.phase
  return phase === 'checking'
    || phase === 'downloading'
    || phase === 'verifying'
    || phase === 'installing'
    || phase === 'restarting'
})
let updateStatusTimer: number | null = null
type BindMode = 'all' | 'loopback' | 'custom'
const bindMode = ref<BindMode>('all')
const bindAllIpv6 = ref(false)
const bindLocalIpv6 = ref(false)
const bindAddressList = ref<string[]>([])
const bindAddressError = computed(() => {
  if (bindMode.value !== 'custom') return ''
  return normalizeBindAddresses(bindAddressList.value).length
    ? ''
    : t('settings.bindAddressListEmpty')
})
const effectiveBindAddresses = computed(() => {
  if (bindMode.value === 'all') {
    return bindAllIpv6.value ? ['0.0.0.0', '::'] : ['0.0.0.0']
  }
  if (bindMode.value === 'loopback') {
    return bindLocalIpv6.value ? ['127.0.0.1', '::1'] : ['127.0.0.1']
  }
  return normalizeBindAddresses(bindAddressList.value)
})

// Config
interface DeviceConfig {
  video: Array<{
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
  }>
  serial: Array<{ path: string; name: string }>
  audio: Array<{ name: string; description: string }>
  udc: Array<{ name: string }>
}

const devices = ref<DeviceConfig>({
  video: [],
  serial: [],
  audio: [],
  udc: [],
})

const config = ref({
  video_device: '',
  video_format: '',
  video_width: 1920,
  video_height: 1080,
  video_fps: 30,
  hid_backend: 'ch9329',
  hid_serial_device: '',
  hid_serial_baudrate: 9600,
  hid_otg_udc: '',
  hid_otg_profile: 'full' as OtgHidProfile,
  hid_otg_functions: {
    keyboard: true,
    mouse_relative: true,
    mouse_absolute: true,
    consumer: true,
  } as OtgHidFunctions,
  msd_enabled: false,
  msd_dir: '',
  encoder_backend: 'auto',
  // STUN/TURN settings
  stun_server: '',
  turn_server: '',
  turn_username: '',
  turn_password: '',
})

// 跟踪服务器是否已配置 TURN 密码
const hasTurnPassword = ref(false)
const configLoaded = ref(false)
const devicesLoaded = ref(false)
const hidProfileAligned = ref(false)

const isLowEndpointUdc = computed(() => {
  if (config.value.hid_otg_udc) {
    return /musb/i.test(config.value.hid_otg_udc)
  }
  return devices.value.udc.some((udc) => /musb/i.test(udc.name))
})

const showLowEndpointHint = computed(() =>
  config.value.hid_backend === 'otg' && isLowEndpointUdc.value
)

type OtgSelfCheckLevel = 'info' | 'warn' | 'error'
type OtgCheckGroupStatus = 'ok' | 'warn' | 'error' | 'skipped'

interface OtgSelfCheckItem {
  id: string
  ok: boolean
  level: OtgSelfCheckLevel
  message: string
  hint?: string
  path?: string
}

interface OtgSelfCheckResult {
  overall_ok: boolean
  error_count: number
  warning_count: number
  hid_backend: string
  selected_udc: string | null
  bound_udc: string | null
  udc_state: string | null
  udc_speed: string | null
  available_udcs: string[]
  other_gadgets: string[]
  checks: OtgSelfCheckItem[]
}

interface OtgCheckGroupDef {
  id: string
  titleKey: string
  checkIds: string[]
}

interface OtgCheckGroup {
  id: string
  titleKey: string
  status: OtgCheckGroupStatus
  okCount: number
  warningCount: number
  errorCount: number
  items: OtgSelfCheckItem[]
}

const otgSelfCheckLoading = ref(false)
const otgSelfCheckResult = ref<OtgSelfCheckResult | null>(null)
const otgSelfCheckError = ref('')
const otgRunButtonPressed = ref(false)

const otgCheckGroupDefs: OtgCheckGroupDef[] = [
  {
    id: 'udc',
    titleKey: 'settings.otgSelfCheck.groups.udc',
    checkIds: ['udc_dir_exists', 'udc_has_entries', 'configured_udc_valid'],
  },
  {
    id: 'gadget_config',
    titleKey: 'settings.otgSelfCheck.groups.gadgetConfig',
    checkIds: ['configfs_mounted', 'usb_gadget_dir_exists', 'libcomposite_loaded'],
  },
  {
    id: 'one_kvm',
    titleKey: 'settings.otgSelfCheck.groups.oneKvm',
    checkIds: ['one_kvm_gadget_exists', 'one_kvm_bound_udc', 'other_gadgets', 'udc_conflict'],
  },
  {
    id: 'functions',
    titleKey: 'settings.otgSelfCheck.groups.functions',
    checkIds: ['hid_functions_present', 'config_c1_exists', 'function_links_ok', 'hid_device_nodes'],
  },
  {
    id: 'link',
    titleKey: 'settings.otgSelfCheck.groups.link',
    checkIds: ['udc_state', 'udc_speed'],
  },
]

const otgCheckGroups = computed<OtgCheckGroup[]>(() => {
  const items = otgSelfCheckResult.value?.checks || []
  return otgCheckGroupDefs.map((group) => {
    const groupItems = items.filter(item => group.checkIds.includes(item.id))
    const errorCount = groupItems.filter(item => item.level === 'error').length
    const warningCount = groupItems.filter(item => item.level === 'warn').length
    const okCount = Math.max(0, groupItems.length - errorCount - warningCount)
    let status: OtgCheckGroupStatus = 'skipped'
    if (groupItems.length > 0) {
      if (errorCount > 0) status = 'error'
      else if (warningCount > 0) status = 'warn'
      else status = 'ok'
    }

    return {
      id: group.id,
      titleKey: group.titleKey,
      status,
      okCount,
      warningCount,
      errorCount,
      items: groupItems,
    }
  })
})

function otgCheckLevelClass(level: OtgSelfCheckLevel): string {
  if (level === 'error') return 'bg-red-500'
  if (level === 'warn') return 'bg-amber-500'
  return 'bg-blue-500'
}

function otgCheckStatusText(level: OtgSelfCheckLevel): string {
  if (level === 'error') return t('common.error')
  if (level === 'warn') return t('common.warning')
  return t('common.info')
}

function otgGroupStatusClass(status: OtgCheckGroupStatus): string {
  if (status === 'error') return 'bg-red-500'
  if (status === 'warn') return 'bg-amber-500'
  if (status === 'ok') return 'bg-emerald-500'
  return 'bg-muted-foreground/40'
}

function otgGroupStatusText(status: OtgCheckGroupStatus): string {
  return t(`settings.otgSelfCheck.status.${status}`)
}

function otgGroupSummary(group: OtgCheckGroup): string {
  if (group.items.length === 0) {
    return t('settings.otgSelfCheck.notRun')
  }
  return t('settings.otgSelfCheck.groupCounts', {
    ok: group.okCount,
    warnings: group.warningCount,
    errors: group.errorCount,
  })
}

function otgCheckMessage(item: OtgSelfCheckItem): string {
  const key = `settings.otgSelfCheck.messages.${item.id}`
  const label = te(key) ? t(key) : item.message
  const result = otgSelfCheckResult.value
  if (!result) return label

  const value = (name: string) => t(`settings.otgSelfCheck.values.${name}`)

  switch (item.id) {
    case 'udc_has_entries':
      return `${label}：${result.available_udcs.length ? result.available_udcs.join(', ') : value('missing')}`
    case 'configured_udc_valid':
      if (!result.selected_udc) return `${label}：${value('notConfigured')}`
      return `${label}：${item.ok ? result.selected_udc : `${value('missing')}/${result.selected_udc}`}`
    case 'configfs_mounted':
      return `${label}：${item.ok ? value('mounted') : value('unmounted')}`
    case 'usb_gadget_dir_exists':
      return `${label}：${item.ok ? value('available') : value('unavailable')}`
    case 'libcomposite_loaded':
      return `${label}：${item.ok ? value('available') : value('unavailable')}`
    case 'one_kvm_gadget_exists':
      return `${label}：${item.ok ? value('exists') : value('missing')}`
    case 'other_gadgets':
      return `${label}：${result.other_gadgets.length ? result.other_gadgets.join(', ') : value('none')}`
    case 'one_kvm_bound_udc':
      return `${label}：${result.bound_udc || value('unbound')}`
    case 'udc_conflict':
      return `${label}：${item.ok ? value('noConflict') : value('conflict')}`
    case 'udc_state':
      return `${label}：${result.udc_state || value('unknown')}`
    case 'udc_speed':
      return `${label}：${result.udc_speed || value('unknown')}`
    default:
      return `${label}：${item.ok ? value('normal') : value('abnormal')}`
  }
}

function otgCheckHint(item: OtgSelfCheckItem): string {
  if (!item.hint) return ''
  const key = `settings.otgSelfCheck.hints.${item.id}`
  return te(key) ? t(key) : item.hint
}

async function runOtgSelfCheck() {
  otgSelfCheckLoading.value = true
  otgSelfCheckError.value = ''
  try {
    otgSelfCheckResult.value = await hidApi.otgSelfCheck()
  } catch (e) {
    console.error('Failed to run OTG self-check:', e)
    otgSelfCheckError.value = t('settings.otgSelfCheck.failed')
  } finally {
    otgSelfCheckLoading.value = false
  }
}

async function onRunOtgSelfCheckClick() {
  if (!otgSelfCheckLoading.value) {
    otgRunButtonPressed.value = true
    window.setTimeout(() => {
      otgRunButtonPressed.value = false
    }, 160)
  }
  await runOtgSelfCheck()
}

function alignHidProfileForLowEndpoint() {
  if (hidProfileAligned.value) return
  if (!configLoaded.value || !devicesLoaded.value) return
  if (config.value.hid_backend !== 'otg') {
    hidProfileAligned.value = true
    return
  }
  if (!isLowEndpointUdc.value) {
    hidProfileAligned.value = true
    return
  }
  if (config.value.hid_otg_profile === 'full') {
    config.value.hid_otg_profile = 'full_no_consumer' as OtgHidProfile
  } else if (config.value.hid_otg_profile === 'full_no_msd') {
    config.value.hid_otg_profile = 'full_no_consumer_no_msd' as OtgHidProfile
  }
  hidProfileAligned.value = true
}

const isHidFunctionSelectionValid = computed(() => {
  if (config.value.hid_backend !== 'otg') return true
  if (config.value.hid_otg_profile !== 'custom') return true
  const f = config.value.hid_otg_functions
  return !!(f.keyboard || f.mouse_relative || f.mouse_absolute || f.consumer)
})

// OTG Descriptor settings
const otgVendorIdHex = ref('1d6b')
const otgProductIdHex = ref('0104')
const otgManufacturer = ref('One-KVM')
const otgProduct = ref('One-KVM USB Device')
const otgSerialNumber = ref('')

// Validate hex input
const validateHex = (event: Event, _field: string) => {
  const input = event.target as HTMLInputElement
  input.value = input.value.replace(/[^0-9a-fA-F]/g, '').toLowerCase()
}

watch(() => config.value.msd_enabled, (enabled) => {
  if (!enabled && activeSection.value === 'msd') {
    activeSection.value = 'hid'
  }
})

watch(bindMode, (mode) => {
  if (mode === 'custom' && bindAddressList.value.length === 0) {
    bindAddressList.value = ['']
  }
})

// ATX config state
const atxConfig = ref({
  enabled: false,
  power: {
    driver: 'none' as AtxDriverType,
    device: '',
    pin: 0,
    active_level: 'high' as ActiveLevel,
    baud_rate: 9600,
  },
  reset: {
    driver: 'none' as AtxDriverType,
    device: '',
    pin: 0,
    active_level: 'high' as ActiveLevel,
    baud_rate: 9600,
  },
  led: {
    enabled: false,
    gpio_chip: '',
    gpio_pin: 0,
    inverted: false,
  },
  wol_interface: '',
})

// ATX devices for discovery
const atxDevices = ref<AtxDevices>({
  gpio_chips: [],
  usb_relays: [],
  serial_ports: [],
})

// Encoder backend
const availableBackends = ref<EncoderBackendInfo[]>([])

const selectedBackendFormats = computed(() => {
  if (config.value.encoder_backend === 'auto') return []
  const backend = availableBackends.value.find(b => b.id === config.value.encoder_backend)
  return backend?.supported_formats || []
})

const isCh9329Backend = computed(() => config.value.hid_backend === 'ch9329')

const selectedDevice = computed(() => {
  return devices.value.video.find(d => d.path === config.value.video_device)
})

const availableFormats = computed(() => {
  if (!selectedDevice.value) return []
  return selectedDevice.value.formats
})

const selectedFormat = computed(() => {
  if (!selectedDevice.value || !config.value.video_format) return null
  return selectedDevice.value.formats.find(f => f.format === config.value.video_format)
})

const availableResolutions = computed(() => {
  if (!selectedFormat.value) return []
  const resMap = new Map<string, { width: number; height: number; fps: number[] }>()

  selectedFormat.value.resolutions.forEach(res => {
    const key = `${res.width}x${res.height}`
    if (!resMap.has(key)) {
      resMap.set(key, { ...res })
    } else {
      const existing = resMap.get(key)!
      const allFps = [...new Set([...existing.fps, ...res.fps])].sort((a, b) => b - a)
      existing.fps = allFps
    }
  })
  
  return Array.from(resMap.values()).sort((a, b) => (b.width * b.height) - (a.width * a.height))
})

const availableFps = computed(() => {
  const currentRes = availableResolutions.value.find(
    r => r.width === config.value.video_width && r.height === config.value.video_height
  )
  return currentRes ? currentRes.fps : []
})

// Watch for device change to set default format
watch(() => config.value.video_device, () => {
  if (availableFormats.value.length > 0) {
    const isValid = availableFormats.value.some(f => f.format === config.value.video_format)
    if (!isValid) {
      config.value.video_format = availableFormats.value[0]?.format || ''
    }
  } else {
    config.value.video_format = ''
  }
})

// Watch for format change to set default resolution
watch(() => config.value.video_format, () => {
  if (availableResolutions.value.length > 0) {
    const isValid = availableResolutions.value.some(
      r => r.width === config.value.video_width && r.height === config.value.video_height
    )
    if (!isValid) {
      const best = availableResolutions.value[0]
      if (best) {
        config.value.video_width = best.width
        config.value.video_height = best.height
        if (best.fps?.[0]) config.value.video_fps = best.fps[0]
      }
    }
  }
})

// Watch for resolution change to set default FPS
watch(() => [config.value.video_width, config.value.video_height], () => {
  const fpsList = availableFps.value
  if (fpsList.length > 0) {
    if (!fpsList.includes(config.value.video_fps)) {
      const firstFps = fpsList[0]
      if (typeof firstFps === 'number') {
        config.value.video_fps = firstFps
      }
    }
  }
})

watch(() => authStore.user, (value) => {
  if (value) {
    usernameInput.value = value
  }
})


// Format bytes to human readable string
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`
}

// Theme handling
function setTheme(newTheme: 'light' | 'dark' | 'system') {
  theme.value = newTheme
  localStorage.setItem('theme', newTheme)

  if (newTheme === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    document.documentElement.classList.toggle('dark', prefersDark)
  } else {
    document.documentElement.classList.toggle('dark', newTheme === 'dark')
  }
}

// Language handling
function handleLanguageChange(lang: string) {
  if (lang === 'zh-CN' || lang === 'en-US') {
    setLanguage(lang)
  }
}

// Account updates
async function changeUsername() {
  usernameError.value = ''
  usernameSaved.value = false

  if (usernameInput.value.length < 2) {
    usernameError.value = t('auth.enterUsername')
    return
  }
  if (!usernamePassword.value) {
    usernameError.value = t('auth.enterPassword')
    return
  }

  usernameSaving.value = true
  try {
    await authApi.changeUsername(usernameInput.value, usernamePassword.value)
    usernameSaved.value = true
    usernamePassword.value = ''
    await authStore.checkAuth()
    usernameInput.value = authStore.user || usernameInput.value
    setTimeout(() => {
      usernameSaved.value = false
    }, 2000)
  } catch (e) {
    usernameError.value = t('auth.invalidPassword')
  } finally {
    usernameSaving.value = false
  }
}

async function changePassword() {
  passwordError.value = ''
  passwordSaved.value = false

  if (!currentPassword.value) {
    passwordError.value = t('auth.enterPassword')
    return
  }
  if (newPassword.value.length < 4) {
    passwordError.value = t('setup.passwordHint')
    return
  }
  if (newPassword.value !== confirmPassword.value) {
    passwordError.value = t('setup.passwordMismatch')
    return
  }

  passwordSaving.value = true
  try {
    await authApi.changePassword(currentPassword.value, newPassword.value)
    currentPassword.value = ''
    newPassword.value = ''
    confirmPassword.value = ''
    passwordSaved.value = true
    setTimeout(() => {
      passwordSaved.value = false
    }, 2000)
  } catch (e) {
    passwordError.value = t('auth.invalidPassword')
  } finally {
    passwordSaving.value = false
  }
}

// Save config - 使用域分离 API
async function saveConfig() {
  loading.value = true
  saved.value = false

  try {
    // 根据当前激活的 section 只保存相关配置
    const savePromises: Promise<unknown>[] = []

    // Video 配置（包括编码器和 WebRTC/STUN/TURN 设置）
    if (activeSection.value === 'video') {
      savePromises.push(
        configStore.updateVideo({
          device: config.value.video_device || undefined,
          format: config.value.video_format || undefined,
          width: config.value.video_width,
          height: config.value.video_height,
          fps: config.value.video_fps,
        })
      )
      // 同时保存 Stream/Encoder 和 STUN/TURN 配置
      savePromises.push(
        configStore.updateStream({
          encoder: config.value.encoder_backend as any,
          stun_server: config.value.stun_server || undefined,
          turn_server: config.value.turn_server || undefined,
          turn_username: config.value.turn_username || undefined,
          turn_password: config.value.turn_password || undefined,
        })
      )
    }

    // HID 配置
    if (activeSection.value === 'hid') {
      if (!isHidFunctionSelectionValid.value) {
        return
      }
      let desiredMsdEnabled = config.value.msd_enabled
      if (config.value.hid_backend === 'otg') {
        if (config.value.hid_otg_profile === 'full') {
          desiredMsdEnabled = true
        } else if (config.value.hid_otg_profile === 'full_no_msd') {
          desiredMsdEnabled = false
        } else if (config.value.hid_otg_profile === 'full_no_consumer') {
          desiredMsdEnabled = true
        } else if (config.value.hid_otg_profile === 'full_no_consumer_no_msd') {
          desiredMsdEnabled = false
        } else if (
          config.value.hid_otg_profile === 'legacy_keyboard'
          || config.value.hid_otg_profile === 'legacy_mouse_relative'
        ) {
          desiredMsdEnabled = false
        }
      }
      const hidUpdate: any = {
        backend: config.value.hid_backend as any,
        ch9329_port: config.value.hid_serial_device || undefined,
        ch9329_baudrate: config.value.hid_serial_baudrate,
      }
      // 如果是 OTG 后端，添加描述符配置
      if (config.value.hid_backend === 'otg') {
        hidUpdate.otg_descriptor = {
          vendor_id: parseInt(otgVendorIdHex.value, 16) || 0x1d6b,
          product_id: parseInt(otgProductIdHex.value, 16) || 0x0104,
          manufacturer: otgManufacturer.value || 'One-KVM',
          product: otgProduct.value || 'One-KVM USB Device',
          serial_number: otgSerialNumber.value || undefined,
        }
        hidUpdate.otg_profile = config.value.hid_otg_profile
        hidUpdate.otg_functions = { ...config.value.hid_otg_functions }
      }
      savePromises.push(configStore.updateHid(hidUpdate))
      if (config.value.msd_enabled !== desiredMsdEnabled) {
        config.value.msd_enabled = desiredMsdEnabled
      }
      savePromises.push(
        configStore.updateMsd({
          enabled: desiredMsdEnabled,
        })
      )
    }

    // MSD 配置
    if (activeSection.value === 'msd') {
      savePromises.push(
        configStore.updateMsd({
          msd_dir: config.value.msd_dir || undefined,
        })
      )
    }

    await Promise.all(savePromises)
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch (e) {
    console.error('Failed to save config:', e)
  } finally {
    loading.value = false
  }
}

// Load config - 使用域分离 API
async function loadConfig() {
  try {
    // 并行加载所有域配置
    const [video, stream, hid, msd] = await Promise.all([
      configStore.refreshVideo(),
      configStore.refreshStream(),
      configStore.refreshHid(),
      configStore.refreshMsd(),
    ])

    config.value = {
      video_device: video.device || '',
      video_format: video.format || '',
      video_width: video.width || 1920,
      video_height: video.height || 1080,
      video_fps: video.fps || 30,
      hid_backend: hid.backend || 'none',
      hid_serial_device: hid.ch9329_port || '',
      hid_serial_baudrate: hid.ch9329_baudrate || 9600,
      hid_otg_udc: hid.otg_udc || '',
      hid_otg_profile: (hid.otg_profile || 'full') as OtgHidProfile,
      hid_otg_functions: {
        keyboard: hid.otg_functions?.keyboard ?? true,
        mouse_relative: hid.otg_functions?.mouse_relative ?? true,
        mouse_absolute: hid.otg_functions?.mouse_absolute ?? true,
        consumer: hid.otg_functions?.consumer ?? true,
      } as OtgHidFunctions,
      msd_enabled: msd.enabled || false,
      msd_dir: msd.msd_dir || '',
      encoder_backend: stream.encoder || 'auto',
      // STUN/TURN settings
      stun_server: stream.stun_server || '',
      turn_server: stream.turn_server || '',
      turn_username: stream.turn_username || '',
      turn_password: '', // 密码不从服务器返回，仅用于设置
    }

    // 设置是否已配置 TURN 密码
    hasTurnPassword.value = stream.has_turn_password || false

    // 加载 OTG 描述符配置
    if (hid.otg_descriptor) {
      otgVendorIdHex.value = hid.otg_descriptor.vendor_id?.toString(16).padStart(4, '0') || '1d6b'
      otgProductIdHex.value = hid.otg_descriptor.product_id?.toString(16).padStart(4, '0') || '0104'
      otgManufacturer.value = hid.otg_descriptor.manufacturer || 'One-KVM'
      otgProduct.value = hid.otg_descriptor.product || 'One-KVM USB Device'
      otgSerialNumber.value = hid.otg_descriptor.serial_number || ''
    }

  } catch (e) {
    console.error('Failed to load config:', e)
  } finally {
    configLoaded.value = true
    alignHidProfileForLowEndpoint()
  }
}

async function loadDevices() {
  try {
    devices.value = await configApi.listDevices()
  } catch (e) {
    console.error('Failed to load devices:', e)
  } finally {
    devicesLoaded.value = true
    alignHidProfileForLowEndpoint()
  }
}

async function loadBackends() {
  try {
    const result = await streamApi.getCodecs()
    availableBackends.value = result.backends || []
  } catch (e) {
    console.error('Failed to load encoder backends:', e)
  }
}

// Auth config functions
async function loadAuthConfig() {
  authConfigLoading.value = true
  try {
    authConfig.value = await configStore.refreshAuth()
  } catch (e) {
    console.error('Failed to load auth config:', e)
  } finally {
    authConfigLoading.value = false
  }
}

async function saveAuthConfig() {
  authConfigLoading.value = true
  try {
    authConfig.value = await configStore.updateAuth({
      single_user_allow_multiple_sessions: authConfig.value.single_user_allow_multiple_sessions,
    })
  } catch (e) {
    console.error('Failed to save auth config:', e)
  } finally {
    authConfigLoading.value = false
  }
}

// Extension management functions
async function loadExtensions() {
  extensionsLoading.value = true
  try {
    extensions.value = await extensionsApi.getAll()
    // Sync config from server
    if (extensions.value) {
      const ttyd = extensions.value.ttyd.config
      extConfig.value.ttyd = {
        enabled: ttyd.enabled,
        shell: ttyd.shell,
      }
      extConfig.value.gostc = { ...extensions.value.gostc.config }
      const easytier = extensions.value.easytier.config
      extConfig.value.easytier = {
        enabled: easytier.enabled,
        network_name: easytier.network_name,
        network_secret: easytier.network_secret,
        peer_urls: easytier.peer_urls || [],
        virtual_ip: easytier.virtual_ip || '',
      }
    }
  } catch (e) {
    console.error('Failed to load extensions:', e)
  } finally {
    extensionsLoading.value = false
  }
}

async function startExtension(id: 'ttyd' | 'gostc' | 'easytier') {
  try {
    await extensionsApi.start(id)
    await loadExtensions()
  } catch (e) {
    console.error(`Failed to start ${id}:`, e)
  }
}

async function stopExtension(id: 'ttyd' | 'gostc' | 'easytier') {
  try {
    await extensionsApi.stop(id)
    await loadExtensions()
  } catch (e) {
    console.error(`Failed to stop ${id}:`, e)
  }
}

async function refreshExtensionLogs(id: 'ttyd' | 'gostc' | 'easytier') {
  try {
    const result = await extensionsApi.logs(id, 100)
    extensionLogs.value[id] = result.logs
  } catch (e) {
    console.error(`Failed to load ${id} logs:`, e)
  }
}

async function saveExtensionConfig(id: 'ttyd' | 'gostc' | 'easytier') {
  loading.value = true
  try {
    if (id === 'ttyd') {
      await extensionsApi.updateTtyd(extConfig.value.ttyd)
    } else if (id === 'gostc') {
      await extensionsApi.updateGostc(extConfig.value.gostc)
    } else if (id === 'easytier') {
      await extensionsApi.updateEasytier(extConfig.value.easytier)
    }
    await loadExtensions()
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch (e) {
    console.error(`Failed to save ${id} config:`, e)
  } finally {
    loading.value = false
  }
}

function isExtRunning(status: ExtensionStatus | undefined): boolean {
  return status?.state === 'running'
}

function openTerminal() {
  showTerminalDialog.value = true
}

function openTerminalInNewTab() {
  window.open('/api/terminal/', '_blank')
}

function getExtStatusText(status: ExtensionStatus | undefined): string {
  if (!status) return t('extensions.stopped')
  switch (status.state) {
    case 'unavailable': return t('extensions.unavailable')
    case 'stopped': return t('extensions.stopped')
    case 'running': return t('extensions.running')
    case 'failed': return t('extensions.failed')
    default: return t('extensions.stopped')
  }
}

function getExtStatusClass(status: ExtensionStatus | undefined): string {
  if (!status) return 'bg-gray-400'
  switch (status.state) {
    case 'unavailable': return 'bg-gray-400'
    case 'stopped': return 'bg-gray-400'
    case 'running': return 'bg-green-500'
    case 'failed': return 'bg-red-500'
    default: return 'bg-gray-400'
  }
}

function addEasytierPeer() {
  if (!extConfig.value.easytier.peer_urls) {
    extConfig.value.easytier.peer_urls = []
  }
  extConfig.value.easytier.peer_urls.push('')
}

function removeEasytierPeer(index: number) {
  if (extConfig.value.easytier.peer_urls) {
    extConfig.value.easytier.peer_urls.splice(index, 1)
  }
}

// ATX management functions
async function loadAtxConfig() {
  try {
    const config = await configStore.refreshAtx()
    atxConfig.value = {
      enabled: config.enabled,
      power: { ...config.power },
      reset: { ...config.reset },
      led: { ...config.led },
      wol_interface: config.wol_interface || '',
    }
  } catch (e) {
    console.error('Failed to load ATX config:', e)
  }
}

async function loadAtxDevices() {
  try {
    atxDevices.value = await atxConfigApi.listDevices()
  } catch (e) {
    console.error('Failed to load ATX devices:', e)
  }
}

async function saveAtxConfig() {
  loading.value = true
  saved.value = false
  try {
    await configStore.updateAtx({
      enabled: atxConfig.value.enabled,
      power: {
        driver: atxConfig.value.power.driver,
        device: atxConfig.value.power.device || undefined,
        pin: atxConfig.value.power.pin,
        active_level: atxConfig.value.power.active_level,
        baud_rate: atxConfig.value.power.baud_rate,
      },
      reset: {
        driver: atxConfig.value.reset.driver,
        device: atxConfig.value.reset.device || undefined,
        pin: atxConfig.value.reset.pin,
        active_level: atxConfig.value.reset.active_level,
        baud_rate: atxConfig.value.reset.baud_rate,
      },
      led: {
        enabled: atxConfig.value.led.enabled,
        gpio_chip: atxConfig.value.led.gpio_chip || undefined,
        gpio_pin: atxConfig.value.led.gpio_pin,
        inverted: atxConfig.value.led.inverted,
      },
      wol_interface: atxConfig.value.wol_interface || undefined,
    })
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch (e) {
    console.error('Failed to save ATX config:', e)
  } finally {
    loading.value = false
  }
}

function getAtxDevicesForDriver(driver: string): string[] {
  if (driver === 'gpio') {
    return atxDevices.value.gpio_chips
  } else if (driver === 'serial') {
    return atxDevices.value.serial_ports
  } else if (driver === 'usbrelay') {
    return atxDevices.value.usb_relays
  }
  return []
}

// RustDesk management functions
async function loadRustdeskConfig() {
  rustdeskLoading.value = true
  try {
    const status = await configStore.refreshRustdeskStatus()
    const config = status.config
    rustdeskConfig.value = config
    rustdeskStatus.value = status
    rustdeskLocalConfig.value = {
      enabled: config.enabled,
      rendezvous_server: config.rendezvous_server,
      relay_server: config.relay_server || '',
      relay_key: '',
    }
  } catch (e) {
    console.error('Failed to load RustDesk config:', e)
  } finally {
    rustdeskLoading.value = false
  }
}

async function loadRustdeskPassword() {
  try {
    rustdeskPassword.value = await configStore.refreshRustdeskPassword()
  } catch (e) {
    console.error('Failed to load RustDesk password:', e)
  }
}

function normalizeRustdeskServer(value: string, defaultPort: number): string | undefined {
  const trimmed = value.trim()
  if (!trimmed) return undefined
  if (trimmed.includes(':')) return trimmed
  return `${trimmed}:${defaultPort}`
}

function normalizeRtspPath(path: string): string {
  return path.trim().replace(/^\/+|\/+$/g, '') || 'live'
}

function normalizeBindAddresses(addresses: string[]): string[] {
  return addresses.map(addr => addr.trim()).filter(Boolean)
}

function applyBindStateFromConfig(config: WebConfig) {
  const rawAddrs =
    config.bind_addresses && config.bind_addresses.length > 0
      ? config.bind_addresses
      : config.bind_address
        ? [config.bind_address]
        : []
  const addrs = normalizeBindAddresses(rawAddrs)
  const isAll = addrs.length > 0 && addrs.every(addr => addr === '0.0.0.0' || addr === '::') && addrs.includes('0.0.0.0')
  const isLoopback =
    addrs.length > 0 &&
    addrs.every(addr => addr === '127.0.0.1' || addr === '::1') &&
    addrs.includes('127.0.0.1')
  if (isAll) {
    bindMode.value = 'all'
    bindAllIpv6.value = addrs.includes('::')
    return
  }
  if (isLoopback) {
    bindMode.value = 'loopback'
    bindLocalIpv6.value = addrs.includes('::1')
    return
  }
  bindMode.value = 'custom'
  bindAddressList.value = addrs.length ? [...addrs] : ['']
}

function addBindAddress() {
  bindAddressList.value.push('')
}

function removeBindAddress(index: number) {
  bindAddressList.value.splice(index, 1)
  if (bindAddressList.value.length === 0) {
    bindAddressList.value.push('')
  }
}

// Web server config functions
async function loadWebServerConfig() {
  try {
    const config = await configStore.refreshWeb()
    webServerConfig.value = config
    applyBindStateFromConfig(config)
  } catch (e) {
    console.error('Failed to load web server config:', e)
  }
}

async function saveWebServerConfig() {
  if (bindAddressError.value) return
  webServerLoading.value = true
  try {
    const update = {
      http_port: webServerConfig.value.http_port,
      https_port: webServerConfig.value.https_port,
      https_enabled: webServerConfig.value.https_enabled,
      bind_addresses: effectiveBindAddresses.value,
    }
    const updated = await configStore.updateWeb(update)
    webServerConfig.value = updated
    applyBindStateFromConfig(updated)
    showRestartDialog.value = true
  } catch (e) {
    console.error('Failed to save web server config:', e)
  } finally {
    webServerLoading.value = false
  }
}

async function restartServer() {
  restarting.value = true
  try {
    await systemApi.restart()
    // Wait for server to restart, then reload page
    setTimeout(() => {
      const protocol = webServerConfig.value.https_enabled ? 'https' : 'http'
      const port = webServerConfig.value.https_enabled
        ? webServerConfig.value.https_port
        : webServerConfig.value.http_port
      const newUrl = `${protocol}://${window.location.hostname}:${port}`
      window.location.href = newUrl
    }, 3000)
  } catch (e) {
    console.error('Failed to restart server:', e)
    restarting.value = false
  }
}

async function loadUpdateOverview() {
  updateLoading.value = true
  try {
    updateOverview.value = await updateApi.overview(updateChannel.value)
  } catch (e) {
    console.error('Failed to load update overview:', e)
  } finally {
    updateLoading.value = false
  }
}

async function refreshUpdateStatus() {
  try {
    updateStatus.value = await updateApi.status()

    if (updateSawRestarting.value && !updateAutoReloadTriggered.value) {
      if (updateSawRequestFailure.value || updateStatus.value.phase === 'idle') {
        updateAutoReloadTriggered.value = true
        window.location.reload()
      }
    }
  } catch (e) {
    console.error('Failed to refresh update status:', e)
    if (updateSawRestarting.value) {
      updateSawRequestFailure.value = true
    }
  }
}

function stopUpdatePolling() {
  if (updateStatusTimer !== null) {
    window.clearInterval(updateStatusTimer)
    updateStatusTimer = null
  }
}

function startUpdatePolling() {
  if (updateStatusTimer !== null) return
  updateStatusTimer = window.setInterval(async () => {
    await refreshUpdateStatus()
    if (updateStatus.value?.phase === 'restarting') {
      updateSawRestarting.value = true
    }
    if (!updateRunning.value) {
      stopUpdatePolling()
      await loadUpdateOverview()
    }
  }, 1000)
}

async function startOnlineUpgrade() {
  try {
    updateSawRestarting.value = false
    updateSawRequestFailure.value = false
    updateAutoReloadTriggered.value = false
    await updateApi.upgrade({ channel: updateChannel.value })
    await refreshUpdateStatus()
    startUpdatePolling()
  } catch (e) {
    console.error('Failed to start upgrade:', e)
  }
}

function updatePhaseText(phase?: string): string {
  switch (phase) {
    case 'idle': return t('settings.updatePhaseIdle')
    case 'checking': return t('settings.updatePhaseChecking')
    case 'downloading': return t('settings.updatePhaseDownloading')
    case 'verifying': return t('settings.updatePhaseVerifying')
    case 'installing': return t('settings.updatePhaseInstalling')
    case 'restarting': return t('settings.updatePhaseRestarting')
    case 'success': return t('settings.updatePhaseSuccess')
    case 'failed': return t('settings.updatePhaseFailed')
    default: return t('common.unknown')
  }
}

function localizeUpdateMessage(message?: string): string | null {
  if (!message) return null

  if (message === 'Checking for updates') return t('settings.updateMsgChecking')
  if (message.startsWith('Downloading binary')) {
    return message.replace('Downloading binary', t('settings.updateMsgDownloading'))
  }
  if (message === 'Verifying sha256') return t('settings.updateMsgVerifying')
  if (message === 'Replacing binary') return t('settings.updateMsgInstalling')
  if (message === 'Restarting service') return t('settings.updateMsgRestarting')

  return message
}

function updateStatusBadgeText(): string {
  return localizeUpdateMessage(updateStatus.value?.message)
    || updatePhaseText(updateStatus.value?.phase)
}

async function saveRustdeskConfig() {
  loading.value = true
  saved.value = false
  try {
    const rendezvousServer = normalizeRustdeskServer(
      rustdeskLocalConfig.value.rendezvous_server,
      21116,
    )
    const relayServer = normalizeRustdeskServer(rustdeskLocalConfig.value.relay_server, 21117)
    await configStore.updateRustdesk({
      enabled: rustdeskLocalConfig.value.enabled,
      rendezvous_server: rendezvousServer,
      relay_server: relayServer,
      relay_key: rustdeskLocalConfig.value.relay_key || undefined,
    })
    await loadRustdeskConfig()
    // Clear relay_key input after save (it's a password field)
    rustdeskLocalConfig.value.relay_key = ''
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch (e) {
    console.error('Failed to save RustDesk config:', e)
  } finally {
    loading.value = false
  }
}

async function regenerateRustdeskId() {
  if (!confirm(t('extensions.rustdesk.confirmRegenerateId'))) return
  rustdeskLoading.value = true
  try {
    await configStore.regenerateRustdeskId()
    await loadRustdeskConfig()
    await loadRustdeskPassword()
  } catch (e) {
    console.error('Failed to regenerate RustDesk ID:', e)
  } finally {
    rustdeskLoading.value = false
  }
}

async function regenerateRustdeskPassword() {
  if (!confirm(t('extensions.rustdesk.confirmRegeneratePassword'))) return
  rustdeskLoading.value = true
  try {
    await configStore.regenerateRustdeskPassword()
    await loadRustdeskConfig()
    await loadRustdeskPassword()
  } catch (e) {
    console.error('Failed to regenerate RustDesk password:', e)
  } finally {
    rustdeskLoading.value = false
  }
}

async function startRustdesk() {
  rustdeskLoading.value = true
  try {
    // Enable and save config to start the service
    await configStore.updateRustdesk({ enabled: true })
    rustdeskLocalConfig.value.enabled = true
    await loadRustdeskConfig()
  } catch (e) {
    console.error('Failed to start RustDesk:', e)
  } finally {
    rustdeskLoading.value = false
  }
}

async function stopRustdesk() {
  rustdeskLoading.value = true
  try {
    // Disable and save config to stop the service
    await configStore.updateRustdesk({ enabled: false })
    rustdeskLocalConfig.value.enabled = false
    await loadRustdeskConfig()
  } catch (e) {
    console.error('Failed to stop RustDesk:', e)
  } finally {
    rustdeskLoading.value = false
  }
}

async function copyToClipboard(text: string, type: 'id' | 'password') {
  const success = await clipboardCopy(text)
  if (success) {
    rustdeskCopied.value = type
    setTimeout(() => (rustdeskCopied.value = null), 2000)
  }
}

function getRustdeskServiceStatusText(status: string | undefined): string {
  if (!status) return t('extensions.rustdesk.notConfigured')
  switch (status) {
    case 'running': return t('extensions.running')
    case 'starting': return t('extensions.starting')
    case 'stopped': return t('extensions.stopped')
    case 'not_initialized': return t('extensions.rustdesk.notInitialized')
    default:
      // Handle "error: xxx" format
      if (status.startsWith('error:')) return t('extensions.failed')
      return status
  }
}

function getRustdeskRendezvousStatusText(status: string | null | undefined): string {
  if (!status) return '-'
  switch (status) {
    case 'registered': return t('extensions.rustdesk.registered')
    case 'connected': return t('extensions.rustdesk.connected')
    case 'connecting': return t('extensions.rustdesk.connecting')
    case 'disconnected': return t('extensions.rustdesk.disconnected')
    default:
      // Handle "error: xxx" format
      if (status.startsWith('error:')) return t('extensions.failed')
      return status
  }
}

function getRustdeskStatusClass(status: string | null | undefined): string {
  switch (status) {
    case 'running':
    case 'registered':
    case 'connected': return 'bg-green-500'
    case 'starting':
    case 'connecting': return 'bg-yellow-500'
    case 'stopped':
    case 'not_initialized':
    case 'disconnected': return 'bg-gray-400'
    default:
      // Handle "error: xxx" format
      if (status?.startsWith('error:')) return 'bg-red-500'
      return 'bg-gray-400'
  }
}

async function loadRtspConfig() {
  rtspLoading.value = true
  try {
    const status = await configStore.refreshRtspStatus()
    rtspStatus.value = status
    rtspLocalConfig.value = {
      enabled: status.config.enabled,
      bind: status.config.bind,
      port: status.config.port,
      path: status.config.path,
      allow_one_client: status.config.allow_one_client,
      codec: status.config.codec,
      username: status.config.username || '',
      password: '',
    }
  } catch (e) {
    console.error('Failed to load RTSP config:', e)
  } finally {
    rtspLoading.value = false
  }
}

async function saveRtspConfig() {
  loading.value = true
  saved.value = false
  try {
    const update: RtspConfigUpdate = {
      enabled: !!rtspLocalConfig.value.enabled,
      bind: rtspLocalConfig.value.bind?.trim() || '0.0.0.0',
      port: Number(rtspLocalConfig.value.port) || 8554,
      path: normalizeRtspPath(rtspLocalConfig.value.path || 'live'),
      allow_one_client: !!rtspLocalConfig.value.allow_one_client,
      codec: rtspLocalConfig.value.codec || 'h264',
      username: (rtspLocalConfig.value.username || '').trim(),
    }

    const nextPassword = (rtspLocalConfig.value.password || '').trim()
    if (nextPassword) {
      update.password = nextPassword
    }

    await configStore.updateRtsp(update)
    await loadRtspConfig()
    rtspLocalConfig.value.password = ''
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch (e) {
    console.error('Failed to save RTSP config:', e)
  } finally {
    loading.value = false
  }
}

async function startRtsp() {
  rtspLoading.value = true
  try {
    await configStore.updateRtsp({ enabled: true })
    rtspLocalConfig.value.enabled = true
    await loadRtspConfig()
  } catch (e) {
    console.error('Failed to start RTSP:', e)
  } finally {
    rtspLoading.value = false
  }
}

async function stopRtsp() {
  rtspLoading.value = true
  try {
    await configStore.updateRtsp({ enabled: false })
    rtspLocalConfig.value.enabled = false
    await loadRtspConfig()
  } catch (e) {
    console.error('Failed to stop RTSP:', e)
  } finally {
    rtspLoading.value = false
  }
}

function getRtspServiceStatusText(status: string | undefined): string {
  if (!status) return t('extensions.stopped')
  switch (status) {
    case 'running': return t('extensions.running')
    case 'starting': return t('extensions.starting')
    case 'stopped': return t('extensions.stopped')
    default:
      if (status.startsWith('error:')) return t('extensions.failed')
      return status
  }
}

function getRtspStatusClass(status: string | undefined): string {
  switch (status) {
    case 'running': return 'bg-green-500'
    case 'starting': return 'bg-yellow-500'
    case 'stopped': return 'bg-gray-400'
    default:
      if (status?.startsWith('error:')) return 'bg-red-500'
      return 'bg-gray-400'
  }
}

// Lifecycle
onMounted(async () => {
  // Load theme preference
  const storedTheme = localStorage.getItem('theme') as 'light' | 'dark' | 'system' | null
  if (storedTheme) {
    theme.value = storedTheme
  }

  await Promise.all([
    systemStore.fetchSystemInfo(),
    loadConfig(),
    loadDevices(),
    loadBackends(),
    loadAuthConfig(),
    loadExtensions(),
    loadAtxConfig(),
    loadAtxDevices(),
    loadRustdeskConfig(),
    loadRustdeskPassword(),
    loadRtspConfig(),
    loadWebServerConfig(),
    loadUpdateOverview(),
    refreshUpdateStatus(),
  ])
  usernameInput.value = authStore.user || ''

  if (updateRunning.value) {
    startUpdatePolling()
  }

  await runOtgSelfCheck()
})

watch(updateChannel, async () => {
  await loadUpdateOverview()
})

watch(() => config.value.hid_backend, async () => {
  await runOtgSelfCheck()
})
</script>

<template>
  <AppLayout>
    <div class="flex h-full overflow-hidden">
      <!-- Mobile Header -->
      <div class="lg:hidden fixed top-16 left-0 right-0 z-20 flex items-center px-4 py-3 border-b bg-background">
        <Sheet v-model:open="mobileMenuOpen">
          <SheetTrigger as-child>
            <Button variant="ghost" size="icon" class="mr-2 h-9 w-9">
              <Menu class="h-4 w-4" />
              <span class="sr-only">{{ t('common.menu') }}</span>
            </Button>
          </SheetTrigger>
          <SheetContent side="left" class="w-72 p-0">
            <div class="p-6">
              <h2 class="text-lg font-semibold mb-4">{{ t('settings.title') }}</h2>
              <nav class="space-y-6">
                <div v-for="group in navGroups" :key="group.title" class="space-y-1">
                  <h3 class="px-3 text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">{{ group.title }}</h3>
                  <button
                    type="button"
                    v-for="item in group.items"
                    :key="item.id"
                    @click="selectSection(item.id)"
                    :class="[
                      'w-full flex items-center gap-3 px-3 py-2 text-sm rounded-md transition-colors',
                      activeSection === item.id
                        ? 'bg-primary text-primary-foreground'
                        : 'hover:bg-muted'
                    ]"
                  >
                    <component :is="item.icon" class="h-4 w-4" />
                    <span>{{ item.label }}</span>
                    <Badge v-if="item.status" variant="outline" :class="['ml-auto text-xs', activeSection === item.id ? 'border-primary-foreground/50 text-primary-foreground' : '']">{{ item.status }}</Badge>
                  </button>
                </div>
              </nav>
            </div>
          </SheetContent>
        </Sheet>
        <h1 class="text-lg font-semibold">{{ t('settings.title') }}</h1>
      </div>

      <!-- Desktop Sidebar -->
      <aside class="hidden lg:block w-64 shrink-0 border-r bg-muted/30">
        <div class="sticky top-0 p-6 space-y-6">
          <h1 class="text-xl font-semibold">{{ t('settings.title') }}</h1>
          <nav class="space-y-6">
            <div v-for="group in navGroups" :key="group.title" class="space-y-1">
              <h3 class="px-3 text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">{{ group.title }}</h3>
              <button
                type="button"
                v-for="item in group.items"
                :key="item.id"
                @click="activeSection = item.id"
                :class="[
                  'w-full flex items-center gap-3 px-3 py-2 text-sm rounded-md transition-colors',
                  activeSection === item.id
                    ? 'bg-primary text-primary-foreground'
                    : 'hover:bg-muted'
                ]"
              >
                <component :is="item.icon" class="h-4 w-4" />
                <span>{{ item.label }}</span>
                <Badge v-if="item.status" variant="outline" :class="['ml-auto text-xs', activeSection === item.id ? 'border-primary-foreground/50 text-primary-foreground' : '']">{{ item.status }}</Badge>
              </button>
            </div>
          </nav>
        </div>
      </aside>

      <!-- Main Content -->
      <main class="flex-1 overflow-y-auto">
        <div class="max-w-2xl mx-auto p-6 lg:p-8 pt-20 lg:pt-8 space-y-6">

          <!-- Appearance Section -->
          <div v-show="activeSection === 'appearance'" class="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.theme') }}</CardTitle>
                <CardDescription>{{ t('settings.themeDesc') }}</CardDescription>
              </CardHeader>
              <CardContent>
                <div class="flex gap-2">
                  <Button :variant="theme === 'light' ? 'default' : 'outline'" size="sm" @click="setTheme('light')">
                    <Sun class="h-4 w-4 mr-2" />{{ t('settings.lightMode') }}
                  </Button>
                  <Button :variant="theme === 'dark' ? 'default' : 'outline'" size="sm" @click="setTheme('dark')">
                    <Moon class="h-4 w-4 mr-2" />{{ t('settings.darkMode') }}
                  </Button>
                  <Button :variant="theme === 'system' ? 'default' : 'outline'" size="sm" @click="setTheme('system')">
                    <Monitor class="h-4 w-4 mr-2" />{{ t('settings.systemMode') }}
                  </Button>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.language') }}</CardTitle>
                <CardDescription>{{ t('settings.languageDesc') }}</CardDescription>
              </CardHeader>
              <CardContent>
                <div class="flex gap-2">
                  <Button :variant="locale === 'zh-CN' ? 'default' : 'outline'" size="sm" @click="handleLanguageChange('zh-CN')">中文</Button>
                  <Button :variant="locale === 'en-US' ? 'default' : 'outline'" size="sm" @click="handleLanguageChange('en-US')">English</Button>
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- Account Section -->
          <div v-show="activeSection === 'account'" class="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.username') }}</CardTitle>
                <CardDescription>{{ t('settings.usernameDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="account-username">{{ t('settings.username') }}</Label>
                  <Input id="account-username" v-model="usernameInput" />
                </div>
                <div class="space-y-2">
                  <Label for="account-username-password">{{ t('settings.currentPassword') }}</Label>
                  <Input id="account-username-password" v-model="usernamePassword" type="password" />
                </div>
                <p v-if="usernameError" class="text-xs text-destructive">{{ usernameError }}</p>
                <p v-else-if="usernameSaved" class="text-xs text-emerald-600">{{ t('common.success') }}</p>
                <div class="flex justify-end">
                  <Button @click="changeUsername" :disabled="usernameSaving">
                    <Save class="h-4 w-4 mr-2" />
                    {{ t('common.save') }}
                  </Button>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.changePassword') }}</CardTitle>
                <CardDescription>{{ t('settings.passwordDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="account-current-password">{{ t('settings.currentPassword') }}</Label>
                  <Input id="account-current-password" v-model="currentPassword" type="password" />
                </div>
                <div class="space-y-2">
                  <Label for="account-new-password">{{ t('settings.newPassword') }}</Label>
                  <Input id="account-new-password" v-model="newPassword" type="password" />
                </div>
                <div class="space-y-2">
                  <Label for="account-confirm-password">{{ t('auth.confirmPassword') }}</Label>
                  <Input id="account-confirm-password" v-model="confirmPassword" type="password" />
                </div>
                <p v-if="passwordError" class="text-xs text-destructive">{{ passwordError }}</p>
                <p v-else-if="passwordSaved" class="text-xs text-emerald-600">{{ t('common.success') }}</p>
                <div class="flex justify-end">
                  <Button @click="changePassword" :disabled="passwordSaving">
                    <Save class="h-4 w-4 mr-2" />
                    {{ t('common.save') }}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- Video Section -->
          <div v-show="activeSection === 'video'" class="space-y-6">
            <!-- Video Device Settings -->
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.videoSettings') }}</CardTitle>
                  <CardDescription>{{ t('settings.videoSettingsDesc') }}</CardDescription>
                </div>
                <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('common.refresh')" @click="loadDevices">
                  <RefreshCw class="h-4 w-4" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="video-device">{{ t('settings.videoDevice') }}</Label>
                  <select id="video-device" v-model="config.video_device" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option value="">{{ t('settings.selectDevice') }}</option>
                    <option v-for="dev in devices.video" :key="dev.path" :value="dev.path">{{ dev.name }} ({{ dev.path }})</option>
                  </select>
                </div>
                <div class="space-y-2">
                  <Label for="video-format">{{ t('settings.videoFormat') }}</Label>
                  <select id="video-format" v-model="config.video_format" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm" :disabled="!config.video_device">
                    <option value="">{{ t('settings.selectFormat') }}</option>
                    <option v-for="fmt in availableFormats" :key="fmt.format" :value="fmt.format">{{ fmt.format }} - {{ fmt.description }}</option>
                  </select>
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="video-resolution">{{ t('settings.resolution') }}</Label>
                    <select id="video-resolution" :value="`${config.video_width}x${config.video_height}`" @change="e => { const parts = (e.target as HTMLSelectElement).value.split('x').map(Number); if (parts[0] && parts[1]) { config.video_width = parts[0]; config.video_height = parts[1]; } }" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm" :disabled="!config.video_format">
                      <option v-for="res in availableResolutions" :key="`${res.width}x${res.height}`" :value="`${res.width}x${res.height}`">{{ res.width }}x{{ res.height }}</option>
                    </select>
                  </div>
                  <div class="space-y-2">
                    <Label for="video-fps">{{ t('settings.frameRate') }}</Label>
                    <select id="video-fps" v-model.number="config.video_fps" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm" :disabled="!config.video_format">
                      <option v-for="fps in availableFps" :key="fps" :value="fps">{{ fps }} FPS</option>
                      <option v-if="!availableFps.includes(config.video_fps)" :value="config.video_fps">{{ config.video_fps }} FPS</option>
                    </select>
                  </div>
                </div>
              </CardContent>
            </Card>

            <!-- Encoder Settings -->
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.encoderBackend') }}</CardTitle>
                <CardDescription>{{ t('settings.encoderBackendDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="encoder-backend">{{ t('settings.backend') }}</Label>
                  <select id="encoder-backend" v-model="config.encoder_backend" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option value="auto">{{ t('settings.autoRecommended') }}</option>
                    <option v-for="backend in availableBackends" :key="backend.id" :value="backend.id">{{ backend.name }} {{ backend.is_hardware ? `(${t('settings.hardware')})` : `(${t('settings.software')})` }}</option>
                  </select>
                </div>
                <div v-if="config.encoder_backend !== 'auto' && selectedBackendFormats.length > 0" class="space-y-2">
                  <Label>{{ t('settings.supportedFormats') }}</Label>
                  <div class="flex flex-wrap gap-2">
                    <Badge v-for="format in selectedBackendFormats" :key="format" variant="outline">{{ format.toUpperCase() }}</Badge>
                  </div>
                </div>
                <p class="text-xs text-muted-foreground">{{ t('settings.encoderHint') }}</p>
              </CardContent>
            </Card>

            <!-- WebRTC/STUN/TURN Settings -->
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.webrtcSettings') }}</CardTitle>
                <CardDescription>{{ t('settings.webrtcSettingsDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="stun-server">{{ t('settings.stunServer') }}</Label>
                  <Input
                    id="stun-server"
                    v-model="config.stun_server"
                    :placeholder="t('settings.stunServerPlaceholder')"
                  />
                  <p class="text-xs text-muted-foreground">{{ t('settings.stunServerHint') }}</p>
                </div>
                <Separator />
                <div class="space-y-2">
                  <Label for="turn-server">{{ t('settings.turnServer') }}</Label>
                  <Input
                    id="turn-server"
                    v-model="config.turn_server"
                    :placeholder="t('settings.turnServerPlaceholder')"
                  />
                  <p class="text-xs text-muted-foreground">{{ t('settings.turnServerHint') }}</p>
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="turn-username">{{ t('settings.turnUsername') }}</Label>
                    <Input
                      id="turn-username"
                      v-model="config.turn_username"
                      :disabled="!config.turn_server"
                    />
                  </div>
                  <div class="space-y-2">
                    <Label for="turn-password">{{ t('settings.turnPassword') }}</Label>
                    <div class="relative">
                      <Input
                        id="turn-password"
                        v-model="config.turn_password"
                        :type="showPasswords ? 'text' : 'password'"
                        :disabled="!config.turn_server"
                        :placeholder="hasTurnPassword ? '••••••••' : ''"
                      />
                      <button
                        type="button"
                        class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground"
                        :aria-label="showPasswords ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
                        @click="showPasswords = !showPasswords"
                      >
                        <Eye v-if="!showPasswords" class="h-4 w-4" />
                        <EyeOff v-else class="h-4 w-4" />
                      </button>
                    </div>
                    <p v-if="hasTurnPassword && !config.turn_password" class="text-xs text-muted-foreground">{{ t('settings.turnPasswordConfigured') }}</p>
                  </div>
                </div>
                <p class="text-xs text-muted-foreground">{{ t('settings.turnCredentialsHint') }}</p>
                <Separator />
                <p class="text-xs text-muted-foreground">{{ t('settings.iceConfigNote') }}</p>
              </CardContent>
            </Card>
          </div>

          <!-- HID Section -->
          <div v-show="activeSection === 'hid'" class="space-y-6">
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.hidSettings') }}</CardTitle>
                  <CardDescription>{{ t('settings.hidSettingsDesc') }}</CardDescription>
                </div>
                <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('common.refresh')" @click="loadDevices">
                  <RefreshCw class="h-4 w-4" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="hid-backend">{{ t('settings.hidBackend') }}</Label>
                  <select id="hid-backend" v-model="config.hid_backend" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option value="ch9329">CH9329 (Serial)</option>
                    <option value="otg">USB OTG</option>
                    <option value="none">{{ t('common.disabled') }}</option>
                  </select>
                </div>
                <div v-if="config.hid_backend === 'ch9329'" class="space-y-2">
                  <Label for="serial-device">{{ t('settings.serialDevice') }}</Label>
                  <select id="serial-device" v-model="config.hid_serial_device" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option value="">{{ t('settings.selectDevice') }}</option>
                    <option v-for="dev in devices.serial" :key="dev.path" :value="dev.path">{{ dev.name }} ({{ dev.path }})</option>
                  </select>
                </div>
                <div v-if="config.hid_backend === 'ch9329'" class="space-y-2">
                  <Label for="serial-baudrate">{{ t('settings.baudRate') }}</Label>
                  <select id="serial-baudrate" v-model.number="config.hid_serial_baudrate" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option :value="9600">9600</option>
                    <option :value="19200">19200</option>
                    <option :value="38400">38400</option>
                    <option :value="57600">57600</option>
                    <option :value="115200">115200</option>
                  </select>
                </div>

                <!-- OTG Descriptor Settings -->
                <template v-if="config.hid_backend === 'otg'">
                  <Separator class="my-4" />
                  <div class="space-y-4">
                    <div>
                      <h4 class="text-sm font-medium">{{ t('settings.otgHidProfile') }}</h4>
                      <p class="text-sm text-muted-foreground">{{ t('settings.otgHidProfileDesc') }}</p>
                    </div>
                    <div class="space-y-2">
                      <Label for="otg-profile">{{ t('settings.profile') }}</Label>
                      <select id="otg-profile" v-model="config.hid_otg_profile" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                        <option value="full">{{ t('settings.otgProfileFull') }}</option>
                        <option value="full_no_msd">{{ t('settings.otgProfileFullNoMsd') }}</option>
                        <option value="full_no_consumer">{{ t('settings.otgProfileFullNoConsumer') }}</option>
                        <option value="full_no_consumer_no_msd">{{ t('settings.otgProfileFullNoConsumerNoMsd') }}</option>
                        <option value="legacy_keyboard">{{ t('settings.otgProfileLegacyKeyboard') }}</option>
                        <option value="legacy_mouse_relative">{{ t('settings.otgProfileLegacyMouseRelative') }}</option>
                        <option value="custom">{{ t('settings.otgProfileCustom') }}</option>
                      </select>
                    </div>
                    <div v-if="config.hid_otg_profile === 'custom'" class="space-y-3 rounded-md border border-border/60 p-3">
                      <div class="flex items-center justify-between">
                        <div>
                          <Label>{{ t('settings.otgFunctionKeyboard') }}</Label>
                          <p class="text-xs text-muted-foreground">{{ t('settings.otgFunctionKeyboardDesc') }}</p>
                        </div>
                        <Switch v-model="config.hid_otg_functions.keyboard" />
                      </div>
                      <Separator />
                      <div class="flex items-center justify-between">
                        <div>
                          <Label>{{ t('settings.otgFunctionMouseRelative') }}</Label>
                          <p class="text-xs text-muted-foreground">{{ t('settings.otgFunctionMouseRelativeDesc') }}</p>
                        </div>
                        <Switch v-model="config.hid_otg_functions.mouse_relative" />
                      </div>
                      <Separator />
                      <div class="flex items-center justify-between">
                        <div>
                          <Label>{{ t('settings.otgFunctionMouseAbsolute') }}</Label>
                          <p class="text-xs text-muted-foreground">{{ t('settings.otgFunctionMouseAbsoluteDesc') }}</p>
                        </div>
                        <Switch v-model="config.hid_otg_functions.mouse_absolute" />
                      </div>
                      <Separator />
                      <div class="flex items-center justify-between">
                        <div>
                          <Label>{{ t('settings.otgFunctionConsumer') }}</Label>
                          <p class="text-xs text-muted-foreground">{{ t('settings.otgFunctionConsumerDesc') }}</p>
                        </div>
                        <Switch v-model="config.hid_otg_functions.consumer" />
                      </div>
                      <Separator />
                      <div class="flex items-center justify-between">
                        <div>
                          <Label>{{ t('settings.otgFunctionMsd') }}</Label>
                          <p class="text-xs text-muted-foreground">{{ t('settings.otgFunctionMsdDesc') }}</p>
                        </div>
                        <Switch v-model="config.msd_enabled" />
                      </div>
                    </div>
                    <p class="text-xs text-amber-600 dark:text-amber-400">
                      {{ t('settings.otgProfileWarning') }}
                    </p>
                    <p v-if="showLowEndpointHint" class="text-xs text-amber-600 dark:text-amber-400">
                      {{ t('settings.otgLowEndpointHint') }}
                    </p>
                  </div>
                  <Separator class="my-4" />
                  <div class="space-y-4">
                    <div>
                      <h4 class="text-sm font-medium">{{ t('settings.otgDescriptor') }}</h4>
                      <p class="text-sm text-muted-foreground">{{ t('settings.otgDescriptorDesc') }}</p>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                      <div class="space-y-2">
                        <Label for="otg-vid">{{ t('settings.vendorId') }}</Label>
                        <Input
                          id="otg-vid"
                          v-model="otgVendorIdHex"
                          placeholder="1d6b"
                          maxlength="4"
                          @input="validateHex($event, 'vid')"
                        />
                      </div>
                      <div class="space-y-2">
                        <Label for="otg-pid">{{ t('settings.productId') }}</Label>
                        <Input
                          id="otg-pid"
                          v-model="otgProductIdHex"
                          placeholder="0104"
                          maxlength="4"
                          @input="validateHex($event, 'pid')"
                        />
                      </div>
                    </div>
                    <div class="space-y-2">
                      <Label for="otg-manufacturer">{{ t('settings.manufacturer') }}</Label>
                      <Input
                        id="otg-manufacturer"
                        v-model="otgManufacturer"
                        placeholder="One-KVM"
                        maxlength="126"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label for="otg-product">{{ t('settings.productName') }}</Label>
                      <Input
                        id="otg-product"
                        v-model="otgProduct"
                        placeholder="One-KVM USB Device"
                        maxlength="126"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label for="otg-serial">{{ t('settings.serialNumber') }}</Label>
                      <Input
                        id="otg-serial"
                        v-model="otgSerialNumber"
                        :placeholder="t('settings.serialNumberAuto')"
                        maxlength="126"
                      />
                    </div>
                    <p class="text-sm text-amber-600 dark:text-amber-400">
                      {{ t('settings.descriptorWarning') }}
                    </p>
                  </div>
                </template>
              </CardContent>
            </Card>

          </div>

          <!-- Environment Section -->
          <div v-show="activeSection === 'environment'" class="space-y-4 max-w-3xl">
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.otgSelfCheck.title') }}</CardTitle>
                  <CardDescription>{{ t('settings.otgSelfCheck.desc') }}</CardDescription>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="otgSelfCheckLoading"
                  :class="[
                    'transition-all duration-150 active:scale-95 active:brightness-95',
                    otgRunButtonPressed ? 'scale-95 brightness-95' : ''
                  ]"
                  @click="onRunOtgSelfCheckClick"
                >
                  <RefreshCw class="h-4 w-4 mr-2" :class="{ 'animate-spin': otgSelfCheckLoading }" />
                  {{ t('settings.otgSelfCheck.run') }}
                </Button>
              </CardHeader>
              <CardContent class="space-y-3">
                <p v-if="otgSelfCheckError" class="text-xs text-red-600 dark:text-red-400">
                  {{ otgSelfCheckError }}
                </p>

                <template v-if="otgSelfCheckResult">
                  <div class="flex flex-wrap gap-2 text-xs">
                    <Badge
                      :variant="otgSelfCheckResult.overall_ok ? 'default' : 'destructive'"
                      class="font-medium"
                    >
                      {{ t('settings.otgSelfCheck.overall') }}：{{ otgSelfCheckResult.overall_ok ? t('settings.otgSelfCheck.ok') : t('settings.otgSelfCheck.hasIssues') }}
                    </Badge>
                    <Badge variant="outline" class="font-normal">
                      {{ t('settings.otgSelfCheck.counts', { errors: otgSelfCheckResult.error_count, warnings: otgSelfCheckResult.warning_count }) }}
                    </Badge>
                    <Badge variant="secondary" class="font-normal">
                      {{ t('settings.otgSelfCheck.selectedUdc') }}：{{ otgSelfCheckResult.selected_udc || '-' }}
                    </Badge>
                    <Badge variant="secondary" class="font-normal">
                      {{ t('settings.otgSelfCheck.boundUdc') }}：{{ otgSelfCheckResult.bound_udc || '-' }}
                    </Badge>
                  </div>

                  <div class="rounded-md border divide-y">
                    <details
                      v-for="group in otgCheckGroups"
                      :key="group.id"
                      :open="group.status === 'error' || group.status === 'warn'"
                      class="group"
                    >
                      <summary class="list-none cursor-pointer px-4 py-3 flex items-center justify-between gap-3 hover:bg-muted/40">
                        <div class="flex items-center gap-2 min-w-0">
                          <span class="inline-block h-2 w-2 rounded-full shrink-0" :class="otgGroupStatusClass(group.status)" />
                          <span class="text-sm font-medium truncate leading-6">{{ t(group.titleKey) }}</span>
                        </div>
                        <div class="flex items-center gap-2 shrink-0">
                          <span class="text-xs text-muted-foreground">{{ otgGroupSummary(group) }}</span>
                          <Badge variant="outline" class="text-[10px] h-5 px-1.5">{{ otgGroupStatusText(group.status) }}</Badge>
                        </div>
                      </summary>

                      <div v-if="group.items.length > 0" class="border-t bg-muted/20">
                        <div
                          v-for="item in group.items"
                          :key="item.id"
                          class="px-4 py-3 border-b last:border-b-0"
                        >
                          <div class="flex items-start gap-2">
                            <span class="inline-block h-2 w-2 rounded-full mt-1.5 shrink-0" :class="otgCheckLevelClass(item.level)" />
                            <div class="min-w-0 space-y-1">
                              <div class="flex items-center gap-2">
                                <p class="text-sm leading-5">{{ otgCheckMessage(item) }}</p>
                                <span class="text-[11px] text-muted-foreground shrink-0">{{ otgCheckStatusText(item.level) }}</span>
                              </div>
                              <div class="flex flex-wrap gap-x-2 gap-y-1 text-[11px] text-muted-foreground">
                                <span v-if="item.hint">{{ otgCheckHint(item) }}</span>
                                <code v-if="item.path" class="font-mono break-all">{{ item.path }}</code>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                      <div v-else class="border-t bg-muted/20 px-4 py-3 text-xs text-muted-foreground">
                        {{ t('settings.otgSelfCheck.notRun') }}
                      </div>
                    </details>
                  </div>
                </template>
                <p v-else-if="otgSelfCheckLoading" class="text-xs text-muted-foreground">
                  {{ t('common.loading') }}
                </p>
              </CardContent>
            </Card>
          </div>

          <!-- Access Section -->
          <div v-show="activeSection === 'access'" class="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.webServer') }}</CardTitle>
                <CardDescription>{{ t('settings.webServerDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <Label>{{ t('settings.httpsEnabled') }}</Label>
                    <p class="text-sm text-muted-foreground">{{ t('settings.httpsEnabledDesc') }}</p>
                  </div>
                  <Switch v-model="webServerConfig.https_enabled" />
                </div>

                <Separator />

                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label>{{ t('settings.httpPort') }}</Label>
                    <Input v-model.number="webServerConfig.http_port" type="number" min="1" max="65535" />
                  </div>
                  <div class="space-y-2">
                    <Label>{{ t('settings.httpsPort') }}</Label>
                    <Input v-model.number="webServerConfig.https_port" type="number" min="1" max="65535" />
                  </div>
                </div>

                <div class="space-y-2">
                  <Label>{{ t('settings.bindMode') }}</Label>
                  <select v-model="bindMode" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option value="all">{{ t('settings.bindModeAll') }}</option>
                    <option value="loopback">{{ t('settings.bindModeLocal') }}</option>
                    <option value="custom">{{ t('settings.bindModeCustom') }}</option>
                  </select>
                  <p class="text-sm text-muted-foreground">{{ t('settings.bindModeDesc') }}</p>
                </div>

                <div v-if="bindMode === 'all'" class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <Label>{{ t('settings.bindIpv6') }}</Label>
                    <p class="text-xs text-muted-foreground">{{ t('settings.bindAllDesc') }}</p>
                  </div>
                  <Switch v-model="bindAllIpv6" />
                </div>

                <div v-if="bindMode === 'loopback'" class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <Label>{{ t('settings.bindIpv6') }}</Label>
                    <p class="text-xs text-muted-foreground">{{ t('settings.bindLocalDesc') }}</p>
                  </div>
                  <Switch v-model="bindLocalIpv6" />
                </div>

                <div v-if="bindMode === 'custom'" class="space-y-2">
                  <Label>{{ t('settings.bindAddressList') }}</Label>
                  <div class="space-y-2">
                    <div v-for="(_, i) in bindAddressList" :key="`bind-${i}`" class="flex gap-2">
                      <Input v-model="bindAddressList[i]" placeholder="192.168.1.10" />
                      <Button variant="ghost" size="icon" :aria-label="t('common.delete')" @click="removeBindAddress(i)">
                        <Trash2 class="h-4 w-4" />
                      </Button>
                    </div>
                    <Button variant="outline" size="sm" @click="addBindAddress">
                      <Plus class="h-4 w-4 mr-1" />
                      {{ t('settings.addBindAddress') }}
                    </Button>
                  </div>
                  <p class="text-xs text-muted-foreground">{{ t('settings.bindAddressListDesc') }}</p>
                  <p v-if="bindAddressError" class="text-xs text-destructive">{{ bindAddressError }}</p>
                </div>

                <div class="flex justify-end pt-4">
                  <Button @click="saveWebServerConfig" :disabled="webServerLoading || !!bindAddressError">
                    <Save class="h-4 w-4 mr-2" />
                    {{ t('common.save') }}
                  </Button>
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.authSettings') }}</CardTitle>
                <CardDescription>{{ t('settings.authSettingsDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <Label>{{ t('settings.allowMultipleSessions') }}</Label>
                    <p class="text-xs text-muted-foreground">{{ t('settings.allowMultipleSessionsDesc') }}</p>
                  </div>
                  <Switch
                    v-model="authConfig.single_user_allow_multiple_sessions"
                    :disabled="authConfigLoading"
                  />
                </div>
                <Separator />
                <p class="text-xs text-muted-foreground">{{ t('settings.singleUserSessionNote') }}</p>
                <div class="flex justify-end pt-2">
                  <Button @click="saveAuthConfig" :disabled="authConfigLoading">
                    <Save class="h-4 w-4 mr-2" />
                    {{ t('common.save') }}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- MSD Section -->
          <div v-show="activeSection === 'msd' && config.msd_enabled" class="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.msdSettings') }}</CardTitle>
                <CardDescription>{{ t('settings.msdDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div v-if="isCh9329Backend" class="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900">
                  <p class="font-medium">{{ t('settings.msdCh9329Warning') }}</p>
                  <p class="text-xs text-amber-900/80">{{ t('settings.msdCh9329WarningDesc') }}</p>
                </div>
                <div class="space-y-4">
                  <div class="space-y-2">
                    <Label for="msd-dir">{{ t('settings.msdDir') }}</Label>
                    <Input id="msd-dir" v-model="config.msd_dir" placeholder="/etc/one-kvm/msd" :disabled="isCh9329Backend" />
                    <p class="text-xs text-muted-foreground">{{ t('settings.msdDirDesc') }}</p>
                  </div>
                  <p class="text-xs text-muted-foreground">{{ t('settings.msdDirHint') }}</p>
                </div>
                <Separator />
                <div class="flex items-center justify-between">
                  <div>
                    <p class="text-sm font-medium">{{ t('settings.msdStatus') }}</p>
                    <p class="text-xs text-muted-foreground">
                      {{ config.msd_enabled ? t('settings.willBeEnabledAfterSave') : t('settings.disabled') }}
                    </p>
                  </div>
                  <Badge :variant="config.msd_enabled ? 'default' : 'secondary'">
                    {{ config.msd_enabled ? t('common.enabled') : t('common.disabled') }}
                  </Badge>
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- ATX Section -->
          <div v-show="activeSection === 'atx'" class="space-y-6">
            <!-- Enable ATX -->
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.atxSettings') }}</CardTitle>
                  <CardDescription>{{ t('settings.atxSettingsDesc') }}</CardDescription>
                </div>
                <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('common.refresh')" @click="loadAtxDevices">
                  <RefreshCw class="h-4 w-4" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <Label for="atx-enabled">{{ t('settings.atxEnable') }}</Label>
                    <p class="text-xs text-muted-foreground">{{ t('settings.atxEnableDesc') }}</p>
                  </div>
                  <Switch
                    id="atx-enabled"
                    v-model="atxConfig.enabled"
                  />
                </div>
              </CardContent>
            </Card>

            <!-- Power Button Config -->
            <Card v-if="atxConfig.enabled">
              <CardHeader>
                <CardTitle>{{ t('settings.atxPowerButton') }}</CardTitle>
                <CardDescription>{{ t('settings.atxPowerButtonDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="power-driver">{{ t('settings.atxDriver') }}</Label>
                    <select id="power-driver" v-model="atxConfig.power.driver" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="none">{{ t('settings.atxDriverNone') }}</option>
                      <option value="gpio">{{ t('settings.atxDriverGpio') }}</option>
                      <option value="usbrelay">{{ t('settings.atxDriverUsbRelay') }}</option>
                      <option value="serial">Serial (LCUS)</option>
                    </select>
                  </div>
                  <div class="space-y-2">
                    <Label for="power-device">{{ t('settings.atxDevice') }}</Label>
                    <select id="power-device" v-model="atxConfig.power.device" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm" :disabled="atxConfig.power.driver === 'none'">
                      <option value="">{{ t('settings.selectDevice') }}</option>
                      <option v-for="dev in getAtxDevicesForDriver(atxConfig.power.driver)" :key="dev" :value="dev">{{ dev }}</option>
                    </select>
                  </div>
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="power-pin">{{ ['usbrelay', 'serial'].includes(atxConfig.power.driver) ? t('settings.atxChannel') : t('settings.atxPin') }}</Label>
                    <Input id="power-pin" type="number" v-model.number="atxConfig.power.pin" min="0" :disabled="atxConfig.power.driver === 'none'" />
                  </div>
                  <div v-if="atxConfig.power.driver === 'gpio'" class="space-y-2">
                    <Label for="power-level">{{ t('settings.atxActiveLevel') }}</Label>
                    <select id="power-level" v-model="atxConfig.power.active_level" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="high">{{ t('settings.atxLevelHigh') }}</option>
                      <option value="low">{{ t('settings.atxLevelLow') }}</option>
                    </select>
                  </div>
                  <div v-if="atxConfig.power.driver === 'serial'" class="space-y-2">
                    <Label for="power-baudrate">{{ t('settings.baudRate') }}</Label>
                    <select id="power-baudrate" v-model.number="atxConfig.power.baud_rate" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option :value="9600">9600</option>
                      <option :value="19200">19200</option>
                      <option :value="38400">38400</option>
                      <option :value="57600">57600</option>
                      <option :value="115200">115200</option>
                    </select>
                  </div>
                </div>
              </CardContent>
            </Card>

            <!-- Reset Button Config -->
            <Card v-if="atxConfig.enabled">
              <CardHeader>
                <CardTitle>{{ t('settings.atxResetButton') }}</CardTitle>
                <CardDescription>{{ t('settings.atxResetButtonDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="reset-driver">{{ t('settings.atxDriver') }}</Label>
                    <select id="reset-driver" v-model="atxConfig.reset.driver" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="none">{{ t('settings.atxDriverNone') }}</option>
                      <option value="gpio">{{ t('settings.atxDriverGpio') }}</option>
                      <option value="usbrelay">{{ t('settings.atxDriverUsbRelay') }}</option>
                      <option value="serial">Serial (LCUS)</option>
                    </select>
                  </div>
                  <div class="space-y-2">
                    <Label for="reset-device">{{ t('settings.atxDevice') }}</Label>
                    <select id="reset-device" v-model="atxConfig.reset.device" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm" :disabled="atxConfig.reset.driver === 'none'">
                      <option value="">{{ t('settings.selectDevice') }}</option>
                      <option v-for="dev in getAtxDevicesForDriver(atxConfig.reset.driver)" :key="dev" :value="dev">{{ dev }}</option>
                    </select>
                  </div>
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="reset-pin">{{ ['usbrelay', 'serial'].includes(atxConfig.reset.driver) ? t('settings.atxChannel') : t('settings.atxPin') }}</Label>
                    <Input id="reset-pin" type="number" v-model.number="atxConfig.reset.pin" min="0" :disabled="atxConfig.reset.driver === 'none'" />
                  </div>
                  <div v-if="atxConfig.reset.driver === 'gpio'" class="space-y-2">
                    <Label for="reset-level">{{ t('settings.atxActiveLevel') }}</Label>
                    <select id="reset-level" v-model="atxConfig.reset.active_level" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="high">{{ t('settings.atxLevelHigh') }}</option>
                      <option value="low">{{ t('settings.atxLevelLow') }}</option>
                    </select>
                  </div>
                  <div v-if="atxConfig.reset.driver === 'serial'" class="space-y-2">
                    <Label for="reset-baudrate">{{ t('settings.baudRate') }}</Label>
                    <select id="reset-baudrate" v-model.number="atxConfig.reset.baud_rate" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option :value="9600">9600</option>
                      <option :value="19200">19200</option>
                      <option :value="38400">38400</option>
                      <option :value="57600">57600</option>
                      <option :value="115200">115200</option>
                    </select>
                  </div>
                </div>
              </CardContent>
            </Card>

            <!-- LED Sensing Config -->
            <Card v-if="atxConfig.enabled">
              <CardHeader>
                <CardTitle>{{ t('settings.atxLedSensing') }}</CardTitle>
                <CardDescription>{{ t('settings.atxLedSensingDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <Label for="led-enabled">{{ t('settings.atxLedEnable') }}</Label>
                    <p class="text-xs text-muted-foreground">{{ t('settings.atxLedEnableDesc') }}</p>
                  </div>
                  <Switch
                    id="led-enabled"
                    v-model="atxConfig.led.enabled"
                  />
                </div>
                <template v-if="atxConfig.led.enabled">
                  <Separator />
                  <div class="grid gap-4 sm:grid-cols-2">
                    <div class="space-y-2">
                      <Label for="led-chip">{{ t('settings.atxLedChip') }}</Label>
                      <select id="led-chip" v-model="atxConfig.led.gpio_chip" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                        <option value="">{{ t('settings.selectDevice') }}</option>
                        <option v-for="dev in atxDevices.gpio_chips" :key="dev" :value="dev">{{ dev }}</option>
                      </select>
                    </div>
                    <div class="space-y-2">
                      <Label for="led-pin">{{ t('settings.atxLedPin') }}</Label>
                      <Input id="led-pin" type="number" v-model.number="atxConfig.led.gpio_pin" min="0" />
                    </div>
                  </div>
                  <div class="flex items-center justify-between">
                    <div class="space-y-0.5">
                      <Label for="led-inverted">{{ t('settings.atxLedInverted') }}</Label>
                      <p class="text-xs text-muted-foreground">{{ t('settings.atxLedInvertedDesc') }}</p>
                    </div>
                    <Switch
                      id="led-inverted"
                      v-model="atxConfig.led.inverted"
                    />
                  </div>
                </template>
              </CardContent>
            </Card>

            <!-- WOL Config -->
            <Card v-if="atxConfig.enabled">
              <CardHeader>
                <CardTitle>{{ t('settings.atxWolSettings') }}</CardTitle>
                <CardDescription>{{ t('settings.atxWolSettingsDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="wol-interface">{{ t('settings.atxWolInterface') }}</Label>
                  <Input
                    id="wol-interface"
                    v-model="atxConfig.wol_interface"
                    :placeholder="t('settings.atxWolInterfacePlaceholder')"
                  />
                  <p class="text-xs text-muted-foreground">{{ t('settings.atxWolInterfaceHint') }}</p>
                </div>
              </CardContent>
            </Card>

            <!-- Save Button -->
            <div class="flex justify-end">
              <Button :disabled="loading" @click="saveAtxConfig">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>
          </div>

          <!-- ttyd Section -->
          <div v-show="activeSection === 'ext-ttyd'" class="space-y-6">
            <Card>
              <CardHeader>
                <div class="flex items-center justify-between">
                  <div class="space-y-1.5">
                    <CardTitle>{{ t('extensions.ttyd.title') }}</CardTitle>
                    <CardDescription>{{ t('extensions.ttyd.desc') }}</CardDescription>
                  </div>
                  <Badge :variant="extensions?.ttyd?.available ? 'default' : 'destructive'">
                    {{ extensions?.ttyd?.available ? t('extensions.available') : t('extensions.unavailable') }}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div v-if="!extensions?.ttyd?.available" class="text-sm text-muted-foreground bg-muted p-3 rounded-md">
                  {{ t('extensions.binaryNotFound', { path: '/usr/bin/ttyd' }) }}
                </div>
                <template v-else>
                  <!-- Status and controls -->
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <div :class="['w-2 h-2 rounded-full', getExtStatusClass(extensions?.ttyd?.status)]" />
                      <span class="text-sm">{{ getExtStatusText(extensions?.ttyd?.status) }}</span>
                    </div>
                    <div class="flex gap-2">
                      <Button
                        v-if="isExtRunning(extensions?.ttyd?.status)"
                        size="sm"
                        variant="default"
                        @click="openTerminal"
                      >
                        <Terminal class="h-4 w-4 mr-1" />
                        {{ t('extensions.ttyd.open') }}
                      </Button>
                      <Button
                        v-if="!isExtRunning(extensions?.ttyd?.status)"
                        size="sm"
                        @click="startExtension('ttyd')"
                        :disabled="extensionsLoading"
                      >
                        <Play class="h-4 w-4 mr-1" />
                        {{ t('extensions.start') }}
                      </Button>
                      <Button
                        v-else
                        size="sm"
                        variant="outline"
                        @click="stopExtension('ttyd')"
                        :disabled="extensionsLoading"
                      >
                        <Square class="h-4 w-4 mr-1" />
                        {{ t('extensions.stop') }}
                      </Button>
                    </div>
                  </div>
                  <Separator />
                  <!-- Config -->
                  <div class="grid gap-4">
                    <div class="flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch v-model="extConfig.ttyd.enabled" :disabled="isExtRunning(extensions?.ttyd?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.ttyd.shell') }}</Label>
                      <Input v-model="extConfig.ttyd.shell" class="sm:col-span-3" placeholder="/bin/bash" :disabled="isExtRunning(extensions?.ttyd?.status)" />
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <button type="button" @click="showLogs.ttyd = !showLogs.ttyd; if (showLogs.ttyd) refreshExtensionLogs('ttyd')" class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
                      <ChevronRight :class="['h-4 w-4 transition-transform', showLogs.ttyd ? 'rotate-90' : '']" />
                      {{ t('extensions.viewLogs') }}
                    </button>
                    <div v-if="showLogs.ttyd" class="space-y-2">
                      <pre class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono">{{ (extensionLogs.ttyd || []).join('\n') || t('extensions.noLogs') }}</pre>
                      <Button variant="ghost" size="sm" @click="refreshExtensionLogs('ttyd')">
                        <RefreshCw class="h-3 w-3 mr-1" />
                        {{ t('common.refresh') }}
                      </Button>
                    </div>
                  </div>
                </template>
              </CardContent>
            </Card>
            <!-- Save button -->
            <div v-if="extensions?.ttyd?.available" class="flex justify-end">
              <Button :disabled="loading || isExtRunning(extensions?.ttyd?.status)" @click="saveExtensionConfig('ttyd')">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>
          </div>

          <!-- Remote Access Section -->
          <div v-show="activeSection === 'ext-remote-access'" class="space-y-6">
            <Card>
              <CardHeader>
                <div class="flex items-center justify-between">
                  <div class="space-y-1.5">
                    <CardTitle>{{ t('extensions.gostc.title') }}</CardTitle>
                    <CardDescription>{{ t('extensions.gostc.desc') }}</CardDescription>
                  </div>
                  <Badge :variant="extensions?.gostc?.available ? 'default' : 'destructive'">
                    {{ extensions?.gostc?.available ? t('extensions.available') : t('extensions.unavailable') }}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div v-if="!extensions?.gostc?.available" class="text-sm text-muted-foreground bg-muted p-3 rounded-md">
                  {{ t('extensions.binaryNotFound', { path: '/usr/bin/gostc' }) }}
                </div>
                <template v-else>
                  <!-- Status and controls -->
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <div :class="['w-2 h-2 rounded-full', getExtStatusClass(extensions?.gostc?.status)]" />
                      <span class="text-sm">{{ getExtStatusText(extensions?.gostc?.status) }}</span>
                    </div>
                    <div class="flex gap-2">
                      <Button
                        v-if="!isExtRunning(extensions?.gostc?.status)"
                        size="sm"
                        @click="startExtension('gostc')"
                        :disabled="extensionsLoading || !extConfig.gostc.key"
                      >
                        <Play class="h-4 w-4 mr-1" />
                        {{ t('extensions.start') }}
                      </Button>
                      <Button
                        v-else
                        size="sm"
                        variant="outline"
                        @click="stopExtension('gostc')"
                        :disabled="extensionsLoading"
                      >
                        <Square class="h-4 w-4 mr-1" />
                        {{ t('extensions.stop') }}
                      </Button>
                    </div>
                  </div>
                  <Separator />
                  <!-- Config -->
                  <div class="grid gap-4">
                    <div class="flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch v-model="extConfig.gostc.enabled" :disabled="isExtRunning(extensions?.gostc?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.gostc.addr') }}</Label>
                      <Input v-model="extConfig.gostc.addr" class="sm:col-span-3" placeholder="gostc.mofeng.run" :disabled="isExtRunning(extensions?.gostc?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.gostc.key') }}</Label>
                      <Input v-model="extConfig.gostc.key" type="password" class="sm:col-span-3" :disabled="isExtRunning(extensions?.gostc?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.gostc.tls') }}</Label>
                      <div class="sm:col-span-3">
                        <Switch v-model="extConfig.gostc.tls" :disabled="isExtRunning(extensions?.gostc?.status)" />
                      </div>
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <button type="button" @click="showLogs.gostc = !showLogs.gostc; if (showLogs.gostc) refreshExtensionLogs('gostc')" class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
                      <ChevronRight :class="['h-4 w-4 transition-transform', showLogs.gostc ? 'rotate-90' : '']" />
                      {{ t('extensions.viewLogs') }}
                    </button>
                    <div v-if="showLogs.gostc" class="space-y-2">
                      <pre class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono">{{ (extensionLogs.gostc || []).join('\n') || t('extensions.noLogs') }}</pre>
                      <Button variant="ghost" size="sm" @click="refreshExtensionLogs('gostc')">
                        <RefreshCw class="h-3 w-3 mr-1" />
                        {{ t('common.refresh') }}
                      </Button>
                    </div>
                  </div>
                </template>
              </CardContent>
            </Card>
            <!-- Save button -->
            <div v-if="extensions?.gostc?.available" class="flex justify-end">
              <Button :disabled="loading || isExtRunning(extensions?.gostc?.status)" @click="saveExtensionConfig('gostc')">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>

            <Card>
              <CardHeader>
                <div class="flex items-center justify-between">
                  <div class="space-y-1.5">
                    <CardTitle>{{ t('extensions.easytier.title') }}</CardTitle>
                    <CardDescription>{{ t('extensions.easytier.desc') }}</CardDescription>
                  </div>
                  <Badge :variant="extensions?.easytier?.available ? 'default' : 'destructive'">
                    {{ extensions?.easytier?.available ? t('extensions.available') : t('extensions.unavailable') }}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div v-if="!extensions?.easytier?.available" class="text-sm text-muted-foreground bg-muted p-3 rounded-md">
                  {{ t('extensions.binaryNotFound', { path: '/usr/bin/easytier-core' }) }}
                </div>
                <template v-else>
                  <!-- Status and controls -->
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <div :class="['w-2 h-2 rounded-full', getExtStatusClass(extensions?.easytier?.status)]" />
                      <span class="text-sm">{{ getExtStatusText(extensions?.easytier?.status) }}</span>
                    </div>
                    <div class="flex gap-2">
                      <Button
                        v-if="!isExtRunning(extensions?.easytier?.status)"
                        size="sm"
                        @click="startExtension('easytier')"
                        :disabled="extensionsLoading || !extConfig.easytier.network_name"
                      >
                        <Play class="h-4 w-4 mr-1" />
                        {{ t('extensions.start') }}
                      </Button>
                      <Button
                        v-else
                        size="sm"
                        variant="outline"
                        @click="stopExtension('easytier')"
                        :disabled="extensionsLoading"
                      >
                        <Square class="h-4 w-4 mr-1" />
                        {{ t('extensions.stop') }}
                      </Button>
                    </div>
                  </div>
                  <Separator />
                  <!-- Config -->
                  <div class="grid gap-4">
                    <div class="flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch v-model="extConfig.easytier.enabled" :disabled="isExtRunning(extensions?.easytier?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easytier.networkName') }}</Label>
                      <Input v-model="extConfig.easytier.network_name" class="sm:col-span-3" :disabled="isExtRunning(extensions?.easytier?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easytier.networkSecret') }}</Label>
                      <Input v-model="extConfig.easytier.network_secret" type="password" class="sm:col-span-3" :disabled="isExtRunning(extensions?.easytier?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easytier.peers') }}</Label>
                      <div class="sm:col-span-3 space-y-2">
                        <div v-for="(_, i) in extConfig.easytier.peer_urls" :key="i" class="flex gap-2">
                          <Input v-model="extConfig.easytier.peer_urls[i]" placeholder="tcp://1.2.3.4:11010" :disabled="isExtRunning(extensions?.easytier?.status)" />
                          <Button variant="ghost" size="icon" :aria-label="t('common.delete')" @click="removeEasytierPeer(i)" :disabled="isExtRunning(extensions?.easytier?.status)">
                            <Trash2 class="h-4 w-4" />
                          </Button>
                        </div>
                        <Button variant="outline" size="sm" @click="addEasytierPeer" :disabled="isExtRunning(extensions?.easytier?.status)">
                          <Plus class="h-4 w-4 mr-1" />
                          {{ t('extensions.easytier.addPeer') }}
                        </Button>
                      </div>
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easytier.virtualIp') }}</Label>
                      <div class="sm:col-span-3 space-y-1">
                        <Input v-model="extConfig.easytier.virtual_ip" placeholder="10.0.0.1/24" :disabled="isExtRunning(extensions?.easytier?.status)" />
                        <p class="text-xs text-muted-foreground">{{ t('extensions.easytier.virtualIpHint') }}</p>
                      </div>
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <button type="button" @click="showLogs.easytier = !showLogs.easytier; if (showLogs.easytier) refreshExtensionLogs('easytier')" class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
                      <ChevronRight :class="['h-4 w-4 transition-transform', showLogs.easytier ? 'rotate-90' : '']" />
                      {{ t('extensions.viewLogs') }}
                    </button>
                    <div v-if="showLogs.easytier" class="space-y-2">
                      <pre class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono">{{ (extensionLogs.easytier || []).join('\n') || t('extensions.noLogs') }}</pre>
                      <Button variant="ghost" size="sm" @click="refreshExtensionLogs('easytier')">
                        <RefreshCw class="h-3 w-3 mr-1" />
                        {{ t('common.refresh') }}
                      </Button>
                    </div>
                  </div>
                </template>
              </CardContent>
            </Card>
            <!-- Save button -->
            <div v-if="extensions?.easytier?.available" class="flex justify-end">
              <Button :disabled="loading || isExtRunning(extensions?.easytier?.status)" @click="saveExtensionConfig('easytier')">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>
          </div>

          <!-- RTSP Section -->
          <div v-show="activeSection === 'ext-rtsp'" class="space-y-6">
            <Card>
              <CardHeader>
                <div class="flex items-center justify-between">
                  <div class="space-y-1.5">
                    <CardTitle>{{ t('extensions.rtsp.title') }}</CardTitle>
                    <CardDescription>{{ t('extensions.rtsp.desc') }}</CardDescription>
                  </div>
                  <div class="flex items-center gap-2">
                    <Badge :variant="rtspStatus?.service_status === 'running' ? 'default' : 'secondary'">
                      {{ getRtspServiceStatusText(rtspStatus?.service_status) }}
                    </Badge>
                    <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('common.refresh')" @click="loadRtspConfig" :disabled="rtspLoading">
                      <RefreshCw :class="['h-4 w-4', rtspLoading ? 'animate-spin' : '']" />
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-2">
                    <div :class="['w-2 h-2 rounded-full', getRtspStatusClass(rtspStatus?.service_status)]" />
                    <span class="text-sm">{{ getRtspServiceStatusText(rtspStatus?.service_status) }}</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <Button
                      v-if="rtspStatus?.service_status !== 'running'"
                      size="sm"
                      @click="startRtsp"
                      :disabled="rtspLoading"
                    >
                      <Play class="h-4 w-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      v-else
                      size="sm"
                      variant="outline"
                      @click="stopRtsp"
                      :disabled="rtspLoading"
                    >
                      <Square class="h-4 w-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                </div>
                <Separator />

                <div class="grid gap-4">
                  <div class="flex items-center justify-between">
                    <Label>{{ t('extensions.autoStart') }}</Label>
                    <Switch v-model="rtspLocalConfig.enabled" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.bind') }}</Label>
                    <Input v-model="rtspLocalConfig.bind" class="sm:col-span-3" placeholder="0.0.0.0" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.port') }}</Label>
                    <Input v-model.number="rtspLocalConfig.port" class="sm:col-span-3" type="number" min="1" max="65535" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.path') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input v-model="rtspLocalConfig.path" :placeholder="t('extensions.rtsp.pathPlaceholder')" />
                      <p class="text-xs text-muted-foreground">{{ t('extensions.rtsp.pathHint') }}</p>
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.codec') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <select v-model="rtspLocalConfig.codec" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                        <option value="h264">H.264</option>
                        <option value="h265">H.265</option>
                      </select>
                      <p class="text-xs text-muted-foreground">{{ t('extensions.rtsp.codecHint') }}</p>
                    </div>
                  </div>
                  <div class="flex items-center justify-between">
                    <Label>{{ t('extensions.rtsp.allowOneClient') }}</Label>
                    <Switch v-model="rtspLocalConfig.allow_one_client" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.username') }}</Label>
                    <Input v-model="rtspLocalConfig.username" class="sm:col-span-3" :placeholder="t('extensions.rtsp.usernamePlaceholder')" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.password') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rtspLocalConfig.password"
                        type="password"
                        :placeholder="rtspStatus?.config?.has_password ? t('extensions.rtsp.passwordSet') : t('extensions.rtsp.passwordPlaceholder')"
                      />
                      <p class="text-xs text-muted-foreground">{{ t('extensions.rtsp.passwordHint') }}</p>
                    </div>
                  </div>
                </div>

                <Separator />

                <div class="rounded-md border p-3 bg-muted/20 space-y-1">
                  <p class="text-sm font-medium">{{ t('extensions.rtsp.urlPreview') }}</p>
                  <code class="font-mono text-sm break-all">{{ rtspStreamUrl }}</code>
                </div>
              </CardContent>
            </Card>
            <div class="flex justify-end">
              <Button :disabled="loading || rtspLoading" @click="saveRtspConfig">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>
          </div>

          <!-- RustDesk Section -->
          <div v-show="activeSection === 'ext-rustdesk'" class="space-y-6">
            <Card>
              <CardHeader>
                <div class="flex items-center justify-between">
                  <div class="space-y-1.5">
                    <CardTitle>{{ t('extensions.rustdesk.title') }}</CardTitle>
                    <CardDescription>{{ t('extensions.rustdesk.desc') }}</CardDescription>
                  </div>
                  <div class="flex items-center gap-2">
                    <Badge :variant="rustdeskStatus?.service_status === 'running' ? 'default' : 'secondary'">
                      {{ getRustdeskServiceStatusText(rustdeskStatus?.service_status) }}
                    </Badge>
                    <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('common.refresh')" @click="loadRustdeskConfig" :disabled="rustdeskLoading">
                      <RefreshCw :class="['h-4 w-4', rustdeskLoading ? 'animate-spin' : '']" />
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <!-- Status and controls -->
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-2">
                    <div :class="['w-2 h-2 rounded-full', getRustdeskStatusClass(rustdeskStatus?.service_status)]" />
                    <span class="text-sm">{{ getRustdeskServiceStatusText(rustdeskStatus?.service_status) }}</span>
                    <template v-if="rustdeskStatus?.rendezvous_status">
                      <span class="text-muted-foreground">|</span>
                      <div :class="['w-2 h-2 rounded-full', getRustdeskStatusClass(rustdeskStatus?.rendezvous_status)]" />
                      <span class="text-sm text-muted-foreground">{{ getRustdeskRendezvousStatusText(rustdeskStatus?.rendezvous_status) }}</span>
                    </template>
                  </div>
                  <div class="flex items-center gap-2">
                    <Button
                      v-if="rustdeskStatus?.service_status !== 'running'"
                      size="sm"
                      @click="startRustdesk"
                      :disabled="rustdeskLoading"
                    >
                      <Play class="h-4 w-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      v-else
                      size="sm"
                      variant="outline"
                      @click="stopRustdesk"
                      :disabled="rustdeskLoading"
                    >
                      <Square class="h-4 w-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                </div>
                <Separator />

                <!-- Config -->
                <div class="grid gap-4">
                  <div class="flex items-center justify-between">
                    <Label>{{ t('extensions.autoStart') }}</Label>
                    <Switch v-model="rustdeskLocalConfig.enabled" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.rendezvousServer') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rustdeskLocalConfig.rendezvous_server"
                        :placeholder="t('extensions.rustdesk.rendezvousServerPlaceholder')"
                      />
                      <p class="text-xs text-muted-foreground">{{ t('extensions.rustdesk.rendezvousServerHint') }}</p>
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.relayServer') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rustdeskLocalConfig.relay_server"
                        :placeholder="t('extensions.rustdesk.relayServerPlaceholder')"
                      />
                      <p class="text-xs text-muted-foreground">{{ t('extensions.rustdesk.relayServerHint') }}</p>
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.relayKey') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rustdeskLocalConfig.relay_key"
                        type="password"
                        :placeholder="rustdeskStatus?.config?.has_relay_key ? t('extensions.rustdesk.relayKeySet') : t('extensions.rustdesk.relayKeyPlaceholder')"
                      />
                      <p class="text-xs text-muted-foreground">{{ t('extensions.rustdesk.relayKeyHint') }}</p>
                    </div>
                  </div>
                </div>
                <Separator />

                <!-- Device Info -->
                <div class="space-y-3">
                  <h4 class="text-sm font-medium">{{ t('extensions.rustdesk.deviceInfo') }}</h4>

                  <!-- Device ID -->
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.deviceId') }}</Label>
                    <div class="sm:col-span-3 flex items-center gap-2">
                      <code class="font-mono text-lg bg-muted px-3 py-1 rounded">{{ rustdeskConfig?.device_id || '-' }}</code>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8"
                        :aria-label="t('extensions.rustdesk.copyId')"
                        @click="copyToClipboard(rustdeskConfig?.device_id || '', 'id')"
                        :disabled="!rustdeskConfig?.device_id"
                      >
                        <Check v-if="rustdeskCopied === 'id'" class="h-4 w-4 text-green-500" />
                        <Copy v-else class="h-4 w-4" />
                      </Button>
                      <Button variant="outline" size="sm" @click="regenerateRustdeskId" :disabled="rustdeskLoading">
                        <RefreshCw class="h-4 w-4 mr-1" />
                        {{ t('extensions.rustdesk.regenerateId') }}
                      </Button>
                    </div>
                  </div>

                  <!-- Device Password (直接显示) -->
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.devicePassword') }}</Label>
                    <div class="sm:col-span-3 flex items-center gap-2">
                      <code class="font-mono text-lg bg-muted px-3 py-1 rounded">{{ rustdeskPassword?.device_password || '-' }}</code>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8"
                        :aria-label="t('extensions.rustdesk.copyPassword')"
                        @click="copyToClipboard(rustdeskPassword?.device_password || '', 'password')"
                        :disabled="!rustdeskPassword?.device_password"
                      >
                        <Check v-if="rustdeskCopied === 'password'" class="h-4 w-4 text-green-500" />
                        <Copy v-else class="h-4 w-4" />
                      </Button>
                      <Button variant="outline" size="sm" @click="regenerateRustdeskPassword" :disabled="rustdeskLoading">
                        <RefreshCw class="h-4 w-4 mr-1" />
                        {{ t('extensions.rustdesk.regeneratePassword') }}
                      </Button>
                    </div>
                  </div>

                  <!-- Keypair Status -->
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.keypairGenerated') }}</Label>
                    <div class="sm:col-span-3">
                      <Badge :variant="rustdeskConfig?.has_keypair ? 'default' : 'secondary'">
                        {{ rustdeskConfig?.has_keypair ? t('common.yes') : t('common.no') }}
                      </Badge>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>
            <!-- Save button -->
            <div class="flex justify-end">
              <Button :disabled="loading" @click="saveRustdeskConfig">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>
          </div>

          <!-- About Section -->
          <div v-show="activeSection === 'about'" class="space-y-6">
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.onlineUpgrade') }}</CardTitle>
                  <CardDescription>{{ t('settings.onlineUpgradeDesc') }}</CardDescription>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-8 w-8"
                  :aria-label="t('common.refresh')"
                  :disabled="updateRunning || updateLoading"
                  @click="loadUpdateOverview"
                >
                  <RefreshCw :class="['h-4 w-4', (updateLoading || updateRunning) ? 'animate-spin' : '']" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label>{{ t('settings.currentVersion') }}</Label>
                    <Badge variant="outline">
                      {{ updateOverview?.current_version || systemStore.version || t('common.unknown') }}
                      ({{ systemStore.buildDate || t('common.unknown') }})
                    </Badge>
                  </div>
                  <div class="space-y-2">
                    <Label>{{ t('settings.latestVersion') }}</Label>
                    <Badge variant="outline">{{ updateOverview?.latest_version || t('common.unknown') }}</Badge>
                  </div>
                </div>

                <div class="space-y-2">
                  <Label>{{ t('settings.updateChannel') }}</Label>
                  <select v-model="updateChannel" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm" :disabled="updateRunning">
                    <option value="stable">Stable</option>
                    <option value="beta">Beta</option>
                  </select>
                </div>

                <div class="space-y-2">
                  <div class="flex items-center justify-between">
                    <Label>{{ t('settings.updateStatus') }}</Label>
                    <Badge
                      variant="outline"
                      class="max-w-[60%] truncate"
                      :title="updateStatusBadgeText()"
                    >
                      {{ updateStatusBadgeText() }}
                    </Badge>
                  </div>
                  <div v-if="updateRunning || updateStatus?.phase === 'failed' || updateStatus?.phase === 'success'" class="w-full h-2 bg-muted rounded overflow-hidden">
                    <div class="h-full bg-primary transition-all" :style="{ width: `${Math.max(0, Math.min(100, updateStatus?.progress || 0))}%` }" />
                  </div>
                  <p v-if="updateStatus?.last_error" class="text-xs text-destructive">{{ updateStatus.last_error }}</p>
                </div>

                <div class="space-y-2">
                  <Label>{{ t('settings.releaseNotes') }}</Label>
                  <div v-if="updateLoading" class="text-sm text-muted-foreground">{{ t('common.loading') }}</div>
                  <div v-else-if="!updateOverview?.notes_between?.length" class="text-sm text-muted-foreground">{{ t('settings.noUpdates') }}</div>
                  <div v-else class="space-y-3 max-h-56 overflow-y-auto pr-1">
                    <div v-for="item in updateOverview.notes_between" :key="item.version" class="rounded border p-3 space-y-2">
                      <div class="flex items-center justify-between">
                        <span class="font-medium">v{{ item.version }}</span>
                        <span class="text-xs text-muted-foreground">{{ item.published_at }}</span>
                      </div>
                      <ul class="list-disc pl-5 text-sm space-y-1">
                        <li v-for="(note, idx) in item.notes" :key="`${item.version}-${idx}`">{{ note }}</li>
                      </ul>
                    </div>
                  </div>
                </div>

                <div class="flex justify-end gap-2">
                  <Button
                    :disabled="updateRunning || !updateOverview?.upgrade_available"
                    @click="startOnlineUpgrade"
                  >
                    <RefreshCw class="h-4 w-4 mr-2" :class="updateRunning ? 'animate-spin' : ''" />
                    {{ t('settings.startUpgrade') }}
                  </Button>
                </div>
              </CardContent>
            </Card>

            <!-- Device Info Card -->
            <Card v-if="systemStore.deviceInfo">
              <CardHeader>
                <CardTitle>{{ t('settings.deviceInfo') }}</CardTitle>
                <CardDescription>{{ t('settings.deviceInfoDesc') }}</CardDescription>
              </CardHeader>
              <CardContent>
                <div class="space-y-3">
                  <div class="flex justify-between items-center py-2 border-b">
                    <span class="text-sm text-muted-foreground">{{ t('settings.hostname') }}</span>
                    <span class="text-sm font-medium">{{ systemStore.deviceInfo.hostname }}</span>
                  </div>
                  <div class="flex justify-between items-center py-2 border-b">
                    <span class="text-sm text-muted-foreground">{{ t('settings.cpuModel') }}</span>
                    <span class="text-sm font-medium truncate max-w-[60%] text-right">{{ systemStore.deviceInfo.cpu_model }}</span>
                  </div>
                  <div class="flex justify-between items-center py-2 border-b">
                    <span class="text-sm text-muted-foreground">{{ t('settings.cpuUsage') }}</span>
                    <span class="text-sm font-medium">{{ systemStore.deviceInfo.cpu_usage.toFixed(1) }}%</span>
                  </div>
                  <div class="flex justify-between items-center py-2 border-b">
                    <span class="text-sm text-muted-foreground">{{ t('settings.memoryUsage') }}</span>
                    <span class="text-sm font-medium">{{ formatBytes(systemStore.deviceInfo.memory_used) }} / {{ formatBytes(systemStore.deviceInfo.memory_total) }}</span>
                  </div>
                  <div class="py-2">
                    <span class="text-sm text-muted-foreground">{{ t('settings.networkAddresses') }}</span>
                    <div class="mt-2 space-y-1">
                      <div v-for="addr in systemStore.deviceInfo.network_addresses" :key="addr.interface" class="flex justify-between items-center text-sm">
                        <span class="text-muted-foreground">{{ addr.interface }}</span>
                        <code class="font-mono bg-muted px-2 py-0.5 rounded">{{ addr.ip }}</code>
                      </div>
                      <div v-if="systemStore.deviceInfo.network_addresses.length === 0" class="text-sm text-muted-foreground">
                        {{ t('common.unknown') }}
                      </div>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>

            <p class="text-xs text-muted-foreground text-center">{{ t('settings.builtWith') }}</p>
          </div>

          <!-- Save Button (sticky) -->
          <div v-if="['video', 'hid', 'msd'].includes(activeSection)" class="sticky bottom-0 pt-4 pb-2 bg-background border-t -mx-6 px-6 lg:-mx-8 lg:px-8">
            <div class="flex justify-end">
              <div class="flex items-center gap-3">
                <p v-if="activeSection === 'hid' && !isHidFunctionSelectionValid" class="text-xs text-amber-600 dark:text-amber-400">
                  {{ t('settings.otgFunctionMinWarning') }}
                </p>
                <Button :disabled="loading || (activeSection === 'hid' && !isHidFunctionSelectionValid)" @click="saveConfig">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
                </Button>
              </div>
            </div>
          </div>

        </div>
      </main>
    </div>

    <!-- Terminal Dialog -->
    <Dialog v-model:open="showTerminalDialog">
      <DialogContent class="w-[95vw] max-w-5xl h-[85dvh] max-h-[720px] p-0 flex flex-col overflow-hidden">
        <DialogHeader class="px-4 py-3 border-b shrink-0">
          <DialogTitle class="flex items-center justify-between w-full">
            <div class="flex items-center gap-2">
              <Terminal class="h-5 w-5" />
              {{ t('extensions.ttyd.title') }}
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8 mr-8"
              @click="openTerminalInNewTab"
              :aria-label="t('extensions.ttyd.openInNewTab')"
              :title="t('extensions.ttyd.openInNewTab')"
            >
              <ExternalLink class="h-4 w-4" />
            </Button>
          </DialogTitle>
        </DialogHeader>
        <div class="flex-1 min-h-0">
          <iframe
            v-if="showTerminalDialog"
            src="/api/terminal/"
            class="w-full h-full border-0"
            allow="clipboard-read; clipboard-write"
            scrolling="no"
          />
        </div>
      </DialogContent>
    </Dialog>

    <!-- Restart Confirmation Dialog -->
    <Dialog v-model:open="showRestartDialog">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t('settings.restartRequired') }}</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground py-4">
          {{ t('settings.restartMessage') }}
        </p>
        <DialogFooter>
          <Button variant="outline" @click="showRestartDialog = false" :disabled="restarting">
            {{ t('common.later') }}
          </Button>
          <Button @click="restartServer" :disabled="restarting">
            <RefreshCw v-if="restarting" class="h-4 w-4 mr-2 animate-spin" />
            {{ restarting ? t('settings.restarting') : t('common.restartNow') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </AppLayout>
</template>
