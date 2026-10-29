<script setup lang="ts">
import { usesCh9329, supportsOtgFunctions } from '@/lib/hidGuide'
import '@/styles/settings.css'
import CardTitle from '@/components/SettingsCardTitle.vue'
import SidebarMenuButton from '@/components/SettingsSidebarButton.vue'
import AboutSystemCards from '@/components/AboutSystemCards.vue'
import SystemDeviceControls from '@/components/SystemDeviceControls.vue'

import McpSettingsCard from '@/components/McpSettingsCard.vue'
import HidDeviceSettings from '@/components/HidDeviceSettings.vue'
import ConsoleLayoutPreview from '@/components/ConsoleLayoutPreview.vue'
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { ApiError } from '@/api/request'
import { useSystemStore } from '@/stores/system'
import { useConfigStore } from '@/stores/config'
import { useAuthStore } from '@/stores/auth'
import {
  authApi,
  configApi,
  otgNetworkApi,
  uacApi,
  hidApi,
  streamApi,
  atxConfigApi,
  extensionsApi,
  redfishConfigApi,
  systemApi,
  updateApi,
  usbApi,
  watchdogConfigApi,
  type EncoderBackendInfo,
  type AuthConfig,
  type WebConfig,
  type UpdateOverviewResponse,
  type UpdateStatusResponse,
  type UpdateChannel,
  type VideoEncoderSelfCheckResponse,
  type DeviceList,
} from '@/api'
import type {
  ExtensionsStatus,
  ExtensionStatus,
  AtxDriverType,
  ActiveLevel,
  AtxDevices,
  HidConfigUpdate,
  OtgHidProfile,
  OtgHidFunctions,
  Ch9329DescriptorConfig,
  Ch9329DescriptorState,
  NetworkInterfaceInfo,
  OtgNetworkStatus,
  WatchdogConfigResponse,
} from '@/types/generated'
import { EasytierConfigMode, FrpProxyType, FrpcConfigMode } from '@/types/generated'
import { toConfigFps } from '@/lib/fps'
import { useClipboard } from '@/composables/useClipboard'
import { useRemoteAccessSettings } from '@/composables/settings/useRemoteAccessSettings'
import { formatHostForUrl } from '@/lib/url'
import { useFeatureVisibility } from '@/composables/useFeatureVisibility'
import { useTheme } from '@/composables/useTheme'
import { useConsoleLayout, type ConsoleLayout } from '@/composables/useConsoleLayout'
import { useVideoDeviceConfiguration } from '@/composables/useVideoDeviceConfiguration'
import { getVideoFormatState } from '@/lib/video-format-support'
import { formatVideoDeviceLabel } from '@/lib/video-device-label'
import AppLayout from '@/components/AppLayout.vue'
import LanguageToggleButton from '@/components/LanguageToggleButton.vue'
import TerminalDialog from '@/components/TerminalDialog.vue'
import TotpSettingsCard from '@/components/TotpSettingsCard.vue'
import VideoInputFields from '@/components/VideoInputFields.vue'
import ExtensionConfigModeEditor from '@/components/ExtensionConfigModeEditor.vue'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia } from '@/components/ui/empty'
import { Skeleton } from '@/components/ui/skeleton'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Textarea } from '@/components/ui/textarea'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import {
  ArrowLeft,
  Monitor,
  Keyboard,
  Info,
  Sun,
  Moon,
  Eye,
  EyeOff,
  Save,
  Check,
  Power,
  Server,
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
  Globe,
  Loader2,
  AlertTriangle,
  Bot,
  ClipboardPaste,
  Wrench,
  PanelTop,
  PanelLeft,
  GalleryHorizontalEnd,
} from 'lucide-vue-next'

const { t, te } = useI18n()
const route = useRoute()
const router = useRouter()
const systemStore = useSystemStore()
const configStore = useConfigStore()
const authStore = useAuthStore()
const featureVisibility = useFeatureVisibility()
const { theme, setTheme } = useTheme()
const { consoleLayout, setConsoleLayout } = useConsoleLayout()
const consoleLayoutOptions: Array<{
  value: ConsoleLayout
  icon: typeof PanelTop
}> = [
  { value: 'current', icon: PanelTop },
  { value: 'floating', icon: GalleryHorizontalEnd },
  { value: 'sidebar', icon: PanelLeft },
]
const EMPTY_SELECT_VALUE = '__one-kvm-empty-select-value__'

const isWindows = computed(() => systemStore.platform?.mode === 'windows')
const otgSupported = computed(() => systemStore.platform?.otg.available === true)
const msdSupported = computed(() => systemStore.platform?.msd.available === true)
const usbResetSupported = computed(() => systemStore.platform?.mode === 'linux')
const onlineUpgradeSupported = computed(() => systemStore.platform?.mode === 'linux')

const activeSection = ref<SettingsSectionId>('appearance')
const loading = ref(false)
const saveError = ref('')
const SETTINGS_SECTION_IDS = [
  'appearance',
  'account',
  'network',
  'video',
  'hid',
  'atx',
  'environment',
  'other',
  'ext-ttyd',
  'third-party-access',
  'mcp',
  'ext-remote-access',
  'about',
] as const
type SettingsSectionId = (typeof SETTINGS_SECTION_IDS)[number]
const SETTINGS_SECTION_ID_SET = new Set<string>(SETTINGS_SECTION_IDS)

const navGroups = computed(() => [
  {
    id: 'software',
    title: t('settings.software'),
    items: [
      { id: 'appearance', label: t('settings.appearance'), icon: Sun },
      { id: 'account', label: t('settings.account'), icon: User },
      { id: 'network', label: t('settings.network'), icon: Globe },
      { id: 'about', label: t('settings.about'), icon: Info },
    ],
  },
  {
    id: 'system',
    title: t('settings.system'),
    items: [
      { id: 'video', label: t('settings.video'), icon: Monitor },
      { id: 'hid', label: t('settings.hid'), icon: Keyboard },
      { id: 'atx', label: t('settings.atx'), icon: Power },
      { id: 'environment', label: t('settings.environment'), icon: Server },
      { id: 'other', label: t('settings.other'), icon: Wrench },
    ],
  },
  {
    id: 'extensions',
    title: t('settings.extensions'),
    items: [
      { id: 'ext-ttyd', label: t('extensions.ttyd.title'), icon: Terminal },
      { id: 'mcp', label: t('mcp.title'), icon: Bot },
      {
        id: 'third-party-access',
        label: t('extensions.thirdPartyAccess.title'),
        icon: ScreenShare,
      },
      { id: 'ext-remote-access', label: t('extensions.remoteAccess.title'), icon: ExternalLink },
    ],
  },
])

const sectionMeta = computed(() => {
  const fallback = { icon: Info, title: t('settings.title'), description: '' }
  for (const group of navGroups.value) {
    for (const item of group.items) {
      if (item.id === activeSection.value) {
        const subtitleKey = `settings.${sectionSubtitleKey(item.id)}`
        return {
          icon: item.icon,
          title: item.label,
          description: te(subtitleKey) ? t(subtitleKey) : '',
        }
      }
    }
  }
  return fallback
})

function sectionSubtitleKey(id: string): string {
  switch (id) {
    case 'ext-ttyd':
      return 'extTtydSubtitle'
    case 'third-party-access':
      return 'thirdPartyAccessSubtitle'
    case 'ext-remote-access':
      return 'extRemoteAccessSubtitle'
    default:
      return `${id}Subtitle`
  }
}

function isSettingsSectionId(value: string): value is SettingsSectionId {
  return SETTINGS_SECTION_ID_SET.has(value)
}

function selectSection(id: string) {
  if (!isSettingsSectionId(id)) return
  if (activeSection.value === id) return
  activeSection.value = id
  void router.replace({ query: { ...route.query, tab: id } })
  void loadSectionData(id)
}

function normalizeSettingsSection(value: unknown): SettingsSectionId | null {
  if (typeof value !== 'string') return null
  if (value === 'access-control') return 'account'
  if (value === 'ext-frpc') return 'ext-remote-access'
  if (value === 'redfish') return 'third-party-access'
  if (value === 'msd') return 'hid'
  if (value === 'ext-rustdesk' || value === 'ext-vnc' || value === 'ext-rtsp')
    return 'third-party-access'
  return isSettingsSectionId(value) ? value : null
}

function ensureVisibleSection() {
  if (!SETTINGS_SECTION_ID_SET.has(activeSection.value)) {
    activeSection.value = 'appearance'
  }
}

async function loadSectionData(section: SettingsSectionId) {
  switch (section) {
    case 'appearance':
      return
    case 'account':
      await loadAuthConfig()
      return
    case 'network':
      await loadWebServerConfig()
      return
    case 'video':
      await Promise.all([loadConfig(), loadDeviceConfig(), loadBackends()])
      return
    case 'hid':
      await Promise.all([loadConfig(), loadHidDeviceOptions()])
      return
    case 'atx':
      await Promise.all([loadConfig(), loadAtxConfig(), loadAtxDevices()])
      return
    case 'environment':
      return
    case 'other':
      await Promise.all([loadWatchdogConfig(), fetchUsbDevices()])
      return
    case 'ext-ttyd':
    case 'ext-remote-access':
      await loadExtensions()
      return
    case 'third-party-access':
      await Promise.all([
        loadWebServerConfig(),
        loadRedfishConfig(),
        loadRustdeskConfig(),
        loadRustdeskPassword(),
        loadRtspConfig(),
        loadVncConfig(),
      ])
      return
    case 'about':
      await Promise.all([loadUpdateOverview(), refreshUpdateStatus()])
      return
  }
}

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
const authConfig = ref<AuthConfig>({
  session_timeout_secs: 3600 * 24,
  single_user_allow_multiple_sessions: false,
})
const authConfigLoading = ref(false)

const watchdogStatus = ref<WatchdogConfigResponse | null>(null)
const watchdogLoading = ref(false)
const watchdogError = ref('')

const watchdogStatusKey = computed(() => {
  const status = watchdogStatus.value
  if (!status) return 'closed'
  if (!status.supported) return status.enabled ? 'error' : 'unsupported'
  if (status.running) return 'running'
  if (status.enabled) return 'error'
  return 'closed'
})

const watchdogDisplayReason = computed(() => {
  const reason = watchdogStatus.value?.reason
  if (!reason) return ''
  if (reason.includes('No hardware watchdog device found'))
    return t('settings.watchdog.unsupportedReason')
  if (reason.includes('discover')) return t('settings.watchdog.discoveryFailed')
  if (reason.includes('open a hardware watchdog')) return t('settings.watchdog.openFailed')
  if (reason.includes('keepalive')) return t('settings.watchdog.feedFailed')
  if (reason.includes('safely disabled') || reason.includes('nowayout'))
    return t('settings.watchdog.disableFailed')
  return t('settings.watchdog.abnormalReason')
})

function watchdogRequestError(error: unknown, action: 'enable' | 'disable'): string {
  const message = error instanceof Error ? error.message : ''
  if (message.includes('No hardware watchdog device found'))
    return t('settings.watchdog.unsupportedReason')
  if (message.includes('cannot be safely disabled') || message.includes('nowayout')) {
    return t('settings.watchdog.disableFailed')
  }
  return t(
    action === 'enable' ? 'settings.watchdog.enableFailed' : 'settings.watchdog.toggleFailed',
  )
}

const watchdogDraft = ref(false)

async function loadWatchdogConfig() {
  watchdogLoading.value = true
  watchdogError.value = ''
  try {
    watchdogStatus.value = await watchdogConfigApi.get()
    watchdogDraft.value = watchdogStatus.value.enabled
  } catch (error) {
    watchdogError.value = error instanceof Error ? error.message : t('settings.watchdog.loadFailed')
  } finally {
    watchdogLoading.value = false
  }
}

async function updateWatchdog(enabled: boolean) {
  if (!watchdogStatus.value || watchdogLoading.value) return
  watchdogLoading.value = true
  watchdogError.value = ''
  try {
    watchdogStatus.value = await watchdogConfigApi.update({ enabled })
    watchdogDraft.value = watchdogStatus.value.enabled
  } catch (error) {
    watchdogError.value = watchdogRequestError(error, enabled ? 'enable' : 'disable')
    try {
      watchdogStatus.value = await watchdogConfigApi.get()
    } catch {
      // Preserve the last confirmed state when status refresh also fails.
    }
  } finally {
    watchdogLoading.value = false
  }
}

const extensions = ref<ExtensionsStatus | null>(null)
const extensionsLoading = ref(false)
const extensionLogs = ref<Record<string, string[]>>({
  ttyd: [],
  easyfrp: [],
  easytier: [],
  frpc: [],
})
const showLogs = ref<Record<string, boolean>>({
  ttyd: false,
  easyfrp: false,
  easytier: false,
  frpc: false,
})

const showTerminalDialog = ref(false)

const extConfig = ref({
  ttyd: { enabled: false, shell: '/bin/bash' },
  easyfrp: { enabled: false, control_urls: [] as string[] },
  easytier: {
    enabled: false,
    config_mode: EasytierConfigMode.Quick,
    network_name: '',
    network_secret: '',
    peer_urls: [] as string[],
    virtual_ip: '',
    custom_toml: '',
  },
  frpc: {
    enabled: false,
    config_mode: FrpcConfigMode.Quick,
    proxy_name: '',
    proxy_type: FrpProxyType.Tcp,
    server_addr: '',
    server_port: 7000,
    token: '',
    local_ip: '127.0.0.1',
    local_port: 22,
    remote_port: undefined as number | undefined,
    custom_domain: '',
    secret_key: '',
    tls: true,
    custom_toml: '',
  },
})

const easyfrpUrlsText = computed({
  get: () => extConfig.value.easyfrp.control_urls.join('\n'),
  set: (value: string) => {
    extConfig.value.easyfrp.control_urls = value
      .split(/\r?\n/)
      .map((url) => url.trim())
      .filter(Boolean)
  },
})

const easyfrpValidationMessage = computed(() => {
  const urls = extConfig.value.easyfrp.control_urls
  if (urls.length === 0) return t('extensions.easyfrp.urlsRequired')
  for (const value of urls) {
    try {
      const url = new URL(value)
      if (
        /\s/.test(value) ||
        !['ws:', 'wss:', 'http:', 'https:', 'txt:'].includes(url.protocol) ||
        !url.hostname ||
        !url.pathname.replace(/\//g, '') ||
        url.username ||
        url.password ||
        url.hash
      ) {
        return t('extensions.easyfrp.urlsInvalid')
      }
    } catch {
      return t('extensions.easyfrp.urlsInvalid')
    }
  }
  return ''
})

const easytierValidationMessage = computed(() => {
  if (extConfig.value.easytier.config_mode === EasytierConfigMode.Full) {
    if (!extConfig.value.easytier.custom_toml?.trim())
      return t('extensions.easytier.fullConfigRequired')
    return ''
  }
  if (!extConfig.value.easytier.network_name?.trim())
    return t('extensions.easytier.networkNameRequired')
  return ''
})

const frpcRemotePortRequired = computed(() =>
  ['tcp', 'udp'].includes(extConfig.value.frpc.proxy_type),
)
const showFrpcRemotePort = computed(() =>
  ['tcp', 'udp', 'stcp', 'sudp', 'xtcp'].includes(extConfig.value.frpc.proxy_type),
)
const showFrpcCustomDomain = computed(() =>
  ['http', 'https'].includes(extConfig.value.frpc.proxy_type),
)
const showFrpcSecretKey = computed(() =>
  ['stcp', 'sudp', 'xtcp'].includes(extConfig.value.frpc.proxy_type),
)
const frpcQuickMode = computed(() => extConfig.value.frpc.config_mode === FrpcConfigMode.Quick)

const frpcValidationMessage = computed(() => {
  if (extConfig.value.frpc.config_mode === FrpcConfigMode.Full) {
    if (!extConfig.value.frpc.custom_toml?.trim()) return t('extensions.frpc.fullConfigRequired')
    return ''
  }
  if (!extConfig.value.frpc.proxy_name?.trim()) return t('extensions.frpc.proxyNameRequired')
  if (!extConfig.value.frpc.server_addr?.trim()) return t('extensions.frpc.serverAddrRequired')
  if (!extConfig.value.frpc.token) return t('extensions.frpc.tokenRequired')
  if (!extConfig.value.frpc.local_ip?.trim()) return t('extensions.frpc.localIpRequired')
  if (frpcRemotePortRequired.value && !extConfig.value.frpc.remote_port)
    return t('extensions.frpc.remotePortRequired')
  return ''
})

const { copy: clipboardCopy } = useClipboard()
const {
  rustdeskConfig,
  rustdeskStatus,
  rustdeskPassword,
  rustdeskLoading,
  rustdeskCopied,
  rustdeskLocalConfig,
  rustdeskValidationMessage,
  rtspStatus,
  rtspLoading,
  rtspLocalConfig,
  vncStatus,
  vncLoading,
  vncLocalConfig,
  rtspStreamUrl,
  vncStreamUrl,
  serviceSaving,
  loadRustdeskConfig,
  loadRustdeskPassword,
  saveRustdeskConfig,
  regenerateRustdeskId,
  regenerateRustdeskPassword,
  startRustdesk,
  stopRustdesk,
  copyToClipboard,
  loadRtspConfig,
  saveRtspConfig,
  startRtsp,
  stopRtsp,
  loadVncConfig,
  saveVncConfig,
  startVnc,
  stopVnc,
} = useRemoteAccessSettings({ loading, showValidationError })

const webServerConfig = ref<WebConfig>({
  http_port: 8080,
  https_port: 8443,
  bind_address: '0.0.0.0',
  bind_addresses: ['0.0.0.0'],
  https_enabled: false,
  has_custom_cert: false,
})
const webServerLoading = ref(false)
const redfishEnabled = ref(false)
const redfishSaving = ref(false)
const sslCertPem = ref('')
const sslKeyPem = ref('')
const certSaving = ref(false)
const certClearing = ref(false)
const showRestartDialog = ref(false)
const restarting = ref(false)
const autoRestarting = ref(false)
const autoRestartFailed = ref(false)
// For HTTPS targets: can't poll (self-signed cert), show manual link instead
const autoRestartManualUrl = ref<string | null>(null)
const autoRestartCountdown = ref(0)
const updateChannel = ref<UpdateChannel>('stable')
const updateOverview = ref<UpdateOverviewResponse | null>(null)
const updateStatus = ref<UpdateStatusResponse | null>(null)
const updateLoading = ref(false)
const updateSawRestarting = ref(false)
const updateSawRequestFailure = ref(false)
const updateAutoReloadTriggered = ref(false)
const updateRunning = computed(() => {
  const phase = updateStatus.value?.phase
  return (
    phase === 'checking' ||
    phase === 'downloading' ||
    phase === 'verifying' ||
    phase === 'installing' ||
    phase === 'restarting'
  )
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

/** 预览当前配置生效后的访问 URL（取第一个非通配地址显示） */
const previewAccessUrl = computed(() => {
  const https = webServerConfig.value.https_enabled
  const port = https ? webServerConfig.value.https_port : webServerConfig.value.http_port
  const scheme = https ? 'https' : 'http'
  // 对通配地址，用当前浏览器 hostname 替代
  const addrs = effectiveBindAddresses.value
  const firstAddr = addrs.find((a) => a !== '0.0.0.0' && a !== '::') ?? window.location.hostname
  const host = firstAddr.includes(':') ? `[${firstAddr}]` : firstAddr
  return `${scheme}://${host}:${port}`
})
const redfishAccessUrl = computed(() => `${previewAccessUrl.value}/redfish/v1/`)

const previewUrlCopied = ref(false)
let previewUrlCopiedTimer: ReturnType<typeof setTimeout> | null = null

async function copyPreviewUrl() {
  const ok = await clipboardCopy(previewAccessUrl.value)
  if (!ok) return
  previewUrlCopied.value = true
  if (previewUrlCopiedTimer) clearTimeout(previewUrlCopiedTimer)
  previewUrlCopiedTimer = setTimeout(() => {
    previewUrlCopied.value = false
  }, 1500)
}

function openPreviewUrl() {
  window.open(previewAccessUrl.value, '_blank', 'noopener,noreferrer')
}

const devices = ref<Pick<DeviceList, 'video' | 'serial' | 'audio' | 'udc'>>({
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
  hid_otg_profile: 'custom' as OtgHidProfile,
  hid_otg_functions: {
    keyboard: true,
    mouse_relative: true,
    mouse_absolute: true,
    consumer: true,
  } as OtgHidFunctions,
  hid_otg_keyboard_leds: false,
  msd_enabled: false,
  msd_dir: '',
  msd_flash_inquiry_string: 'One-KVM Virtual Flash',
  msd_cdrom_inquiry_string: 'One-KVM Virtual CD-ROM',
  otg_network_enabled: false,
  otg_network_driver: 'ncm' as 'ncm' | 'ecm' | 'rndis',
  otg_network_interface: '',
  encoder_backend: 'auto',
  stun_server: '',
  turn_server: '',
  turn_username: '',
  turn_password: '',
  uac_enabled: false,
})

const otgNetworkInterfaces = ref<NetworkInterfaceInfo[]>([])
const otgNetworkInterfacesLoaded = ref(false)
const otgNetworkInterfacesError = ref('')
const otgNetworkStatus = ref<OtgNetworkStatus | null>(null)

function syncOtgNetworkInterface() {
  const firstInterface = otgNetworkInterfaces.value[0]?.name || ''
  const selectedInterfaceExists = otgNetworkInterfaces.value.some(
    (item) => item.name === config.value.otg_network_interface,
  )
  if (!selectedInterfaceExists) {
    config.value.otg_network_interface = firstInterface
  }
}

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
    checkIds: [
      'hid_functions_present',
      'config_c1_exists',
      'function_links_ok',
      'hid_device_nodes',
    ],
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
    const groupItems = items.filter((item) => group.checkIds.includes(item.id))
    const errorCount = groupItems.filter((item) => item.level === 'error').length
    const warningCount = groupItems.filter((item) => item.level === 'warn').length
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
  if (level === 'error') return 'bg-destructive'
  if (level === 'warn') return 'bg-warning'
  return 'bg-info'
}

function otgCheckStatusText(level: OtgSelfCheckLevel): string {
  if (level === 'error') return t('common.error')
  if (level === 'warn') return t('common.warning')
  return t('common.info')
}

function otgGroupStatusClass(status: OtgCheckGroupStatus): string {
  if (status === 'error') return 'bg-destructive'
  if (status === 'warn') return 'bg-warning'
  if (status === 'ok') return 'bg-status-active'
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
  if (!otgSupported.value) return
  otgSelfCheckLoading.value = true
  otgSelfCheckError.value = ''
  try {
    otgSelfCheckResult.value = await hidApi.otgSelfCheck()
  } catch {
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

type VideoEncoderSelfCheckCell = VideoEncoderSelfCheckResponse['rows'][number]['cells'][number]
type VideoEncoderSelfCheckRow = VideoEncoderSelfCheckResponse['rows'][number]

const videoEncoderSelfCheckLoading = ref(false)
const videoEncoderSelfCheckResult = ref<VideoEncoderSelfCheckResponse | null>(null)
const videoEncoderSelfCheckError = ref('')
const videoEncoderRunButtonPressed = ref(false)

function videoEncoderCell(
  row: VideoEncoderSelfCheckRow,
  codecId: string,
): VideoEncoderSelfCheckCell | undefined {
  return row.cells.find((cell) => cell.codec_id === codecId)
}

const currentHardwareEncoderText = computed(() =>
  videoEncoderSelfCheckResult.value?.current_hardware_encoder === 'None'
    ? t('settings.encoderSelfCheck.none')
    : videoEncoderSelfCheckResult.value?.current_hardware_encoder ||
      t('settings.encoderSelfCheck.none'),
)

function videoEncoderCodecLabel(codecId: string, codecName: string): string {
  return codecId === 'h265' ? 'H.265' : codecName
}

function videoEncoderCellClass(ok: boolean | undefined): string {
  return ok ? 'text-success' : 'text-destructive'
}

function videoEncoderCellSymbol(ok: boolean | undefined): string {
  return ok ? '✓' : '✗'
}

function videoEncoderCellTime(cell: VideoEncoderSelfCheckCell | undefined): string {
  if (!cell || typeof cell.elapsed_ms !== 'number') return '-'
  return `${cell.elapsed_ms}ms`
}

async function runVideoEncoderSelfCheck() {
  videoEncoderSelfCheckLoading.value = true
  videoEncoderSelfCheckError.value = ''
  try {
    videoEncoderSelfCheckResult.value = await streamApi.encoderSelfCheck()
  } catch {
    videoEncoderSelfCheckError.value = t('settings.encoderSelfCheck.failed')
  } finally {
    videoEncoderSelfCheckLoading.value = false
  }
}

async function onRunVideoEncoderSelfCheckClick() {
  if (!videoEncoderSelfCheckLoading.value) {
    videoEncoderRunButtonPressed.value = true
    window.setTimeout(() => {
      videoEncoderRunButtonPressed.value = false
    }, 160)
  }
  await runVideoEncoderSelfCheck()
}

const usbDevices = ref<import('@/api').UsbDeviceInfo[]>([])
const usbDevicesLoading = ref(false)
const usbDevicesError = ref('')
const usbResetTarget = ref<import('@/api').UsbDeviceInfo | null>(null)
const usbResetLoading = ref(false)

async function fetchUsbDevices() {
  if (!usbResetSupported.value) {
    usbDevices.value = []
    return
  }
  usbDevicesLoading.value = true
  usbDevicesError.value = ''
  try {
    usbDevices.value = await usbApi.listDevices()
  } catch {
    usbDevicesError.value = t('settings.usbDevices.loadFailed')
  } finally {
    usbDevicesLoading.value = false
  }
}

async function confirmUsbReset() {
  if (!usbResetSupported.value || !usbResetTarget.value) return
  usbResetLoading.value = true
  try {
    await usbApi.resetDevice(usbResetTarget.value.bus_num, usbResetTarget.value.dev_num)
  } catch {
  } finally {
    usbResetLoading.value = false
    usbResetTarget.value = null
    setTimeout(() => fetchUsbDevices(), 1500)
  }
}

function usbSpeedLabel(speed?: string): string {
  if (!speed) return '-'
  const map: Record<string, string> = {
    '1.5': '1.5 Mbps',
    '12': '12 Mbps',
    '480': '480 Mbps',
    '5000': '5 Gbps',
    '10000': '10 Gbps',
  }
  return map[speed] || `${speed} Mbps`
}

const effectiveOtgFunctions = computed(() => ({ ...config.value.hid_otg_functions }))

const isKeyboardLedToggleDisabled = computed(
  () => config.value.hid_backend !== 'otg' || !effectiveOtgFunctions.value.keyboard,
)

const isHidFunctionSelectionValid = computed(() => {
  if (config.value.hid_backend !== 'otg') return true
  const f = config.value.hid_otg_functions
  return !!(f.keyboard || f.mouse_relative || f.mouse_absolute || f.consumer)
})

function isValidInquiryString(value: string): boolean {
  return /^[\x20-\x7e]{1,28}$/.test(value.trim())
}

const areMsdInquiryStringsValid = computed(
  () =>
    !config.value.msd_enabled ||
    (isValidInquiryString(config.value.msd_flash_inquiry_string) &&
      isValidInquiryString(config.value.msd_cdrom_inquiry_string)),
)

const otgVendorIdHex = ref('1d6b')
const otgProductIdHex = ref('0104')
const otgManufacturer = ref('One-KVM')
const otgProduct = ref('One-KVM USB Device')
const otgSerialNumber = ref('')
const ch9329VendorIdHex = ref('1a86')
const ch9329ProductIdHex = ref('e129')
const ch9329Manufacturer = ref('WCH.CN')
const ch9329Product = ref('CH9329')
const ch9329SerialNumber = ref('')
const ch9329DescriptorLoaded = ref(false)
const ch9329DescriptorLoading = ref(false)
const ch9329DescriptorError = ref('')
const ch9329DescriptorSource = ref<{ port: string; baudrate: number } | null>(null)
const ch9329DescriptorBaseline = ref<{
  vendorId: string
  productId: string
  manufacturer: string
  product: string
  serialNumber: string
} | null>(null)
const utf8Encoder = new TextEncoder()

const validateHex = (event: Event, _field: string) => {
  const input = event.target as HTMLInputElement
  input.value = input.value.replace(/[^0-9a-fA-F]/g, '').toLowerCase()
}

function utf8ByteLength(value: string): number {
  return utf8Encoder.encode(value).length
}

function applyCh9329DescriptorForm(descriptor: Ch9329DescriptorConfig, defaults = false) {
  ch9329VendorIdHex.value = descriptor.vendor_id?.toString(16).padStart(4, '0') || '1a86'
  ch9329ProductIdHex.value = descriptor.product_id?.toString(16).padStart(4, '0') || 'e129'
  ch9329Manufacturer.value = descriptor.manufacturer || (defaults ? 'WCH.CN' : '')
  ch9329Product.value = descriptor.product || (defaults ? 'CH9329' : '')
  ch9329SerialNumber.value = descriptor.serial_number || ''
}

function applyCh9329DescriptorState(state: Ch9329DescriptorState) {
  applyCh9329DescriptorForm(state.descriptor)
  ch9329DescriptorBaseline.value = currentCh9329DescriptorForm()
  if (!state.config_mode_available) {
    ch9329DescriptorError.value = t('settings.ch9329ConfigModeUnavailable')
  }
}

function currentCh9329DescriptorForm() {
  return {
    vendorId: ch9329VendorIdHex.value.toLowerCase().padStart(4, '0'),
    productId: ch9329ProductIdHex.value.toLowerCase().padStart(4, '0'),
    manufacturer: ch9329Manufacturer.value,
    product: ch9329Product.value,
    serialNumber: ch9329SerialNumber.value,
  }
}

function currentCh9329DescriptorSource() {
  return {
    port: config.value.hid_serial_device || '',
    baudrate: Number(config.value.hid_serial_baudrate) || 9600,
  }
}

function clearCh9329DescriptorState() {
  ch9329DescriptorLoaded.value = false
  ch9329DescriptorLoading.value = false
  ch9329DescriptorError.value = ''
  ch9329DescriptorSource.value = null
  ch9329DescriptorBaseline.value = null
}

const isCh9329DescriptorSourceCurrent = computed(() => {
  if (!usesCh9329(config.value.hid_backend)) return false
  const source = ch9329DescriptorSource.value
  if (!source) return false
  const current = currentCh9329DescriptorSource()
  return source.port === current.port && source.baudrate === current.baudrate
})

async function loadCh9329Descriptor() {
  if (!usesCh9329(config.value.hid_backend)) return
  const source = currentCh9329DescriptorSource()
  ch9329DescriptorLoading.value = true
  ch9329DescriptorLoaded.value = false
  ch9329DescriptorSource.value = null
  ch9329DescriptorError.value = ''
  try {
    const state = await hidApi.ch9329Descriptor({
      port: source.port,
      baudRate: source.baudrate,
    })
    applyCh9329DescriptorState(state)
    ch9329DescriptorLoaded.value = true
    ch9329DescriptorSource.value = source
  } catch (e) {
    ch9329DescriptorError.value =
      e instanceof Error ? e.message : t('settings.ch9329DescriptorLoadFailed')
  } finally {
    ch9329DescriptorLoading.value = false
  }
}

const isCh9329DescriptorValid = computed(() => {
  if (!usesCh9329(config.value.hid_backend)) return true
  return (
    utf8ByteLength(ch9329Manufacturer.value) <= 23 &&
    utf8ByteLength(ch9329Product.value) <= 23 &&
    utf8ByteLength(ch9329SerialNumber.value) <= 23
  )
})

const canEditCh9329Descriptor = computed(
  () =>
    usesCh9329(config.value.hid_backend) &&
    ch9329DescriptorLoaded.value &&
    isCh9329DescriptorSourceCurrent.value &&
    !ch9329DescriptorLoading.value,
)

const isCh9329DescriptorDirty = computed(() => {
  if (!canEditCh9329Descriptor.value || !ch9329DescriptorBaseline.value) return false
  const current = currentCh9329DescriptorForm()
  const baseline = ch9329DescriptorBaseline.value
  return (
    current.vendorId !== baseline.vendorId ||
    current.productId !== baseline.productId ||
    current.manufacturer !== baseline.manufacturer ||
    current.product !== baseline.product ||
    current.serialNumber !== baseline.serialNumber
  )
})

const hasMacosDragInterfaceConflict = computed(
  () =>
    config.value.hid_backend === 'otg' &&
    configStore.hid?.mouse_macos_drag &&
    (!effectiveOtgFunctions.value.mouse_relative || !effectiveOtgFunctions.value.mouse_absolute),
)

const isHidSettingsValid = computed(
  () =>
    isHidFunctionSelectionValid.value &&
    !hasMacosDragInterfaceConflict.value &&
    isCh9329DescriptorValid.value &&
    areMsdInquiryStringsValid.value,
)

watch(bindMode, (mode) => {
  if (mode === 'custom' && bindAddressList.value.length === 0) {
    bindAddressList.value = ['']
  }
})

const atxConfig = ref({
  enabled: false,
  driver: 'none' as AtxDriverType,
  device: '',
  baud_rate: 9600,
  power: {
    enabled: false,
    device: '',
    pin: 1,
    active_level: 'high' as ActiveLevel,
  },
  reset: {
    enabled: false,
    device: '',
    pin: 1,
    active_level: 'high' as ActiveLevel,
  },
  led: {
    enabled: false,
    device: '',
    pin: 0,
    active_level: 'high' as ActiveLevel,
  },
  hdd: {
    enabled: false,
    device: '',
    pin: 0,
    active_level: 'high' as ActiveLevel,
  },
  wol_interface: '',
})

const atxSaving = ref(false)
const atxSaved = ref(false)
const wolSaving = ref(false)
const wolSaved = ref(false)

const atxDevices = ref<AtxDevices>({
  gpio_chips: [],
  usb_relays: [],
  serial_ports: [],
})

const ch9329ReservedSerialDevice = computed(() => {
  if (!usesCh9329(config.value.hid_backend)) return ''
  return config.value.hid_serial_device.trim()
})

const atxDriverOptions = computed(() => {
  const options = [
    { value: 'none' as AtxDriverType, label: t('settings.atxDriverNone') },
    { value: 'gpio' as AtxDriverType, label: t('settings.atxDriverGpio') },
    { value: 'usbrelay' as AtxDriverType, label: t('settings.atxDriverUsbRelay') },
    { value: 'serial' as AtxDriverType, label: t('settings.atxDriverSerial') },
  ]
  return isWindows.value
    ? options.filter((option) => ['none', 'serial'].includes(option.value))
    : options
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
  active: computed(() => activeSection.value === 'video'),
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

watch(
  () => authStore.user,
  (value) => {
    if (value) {
      usernameInput.value = value
    }
  },
)

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
      await configStore.updateStream({ encoder: config.value.encoder_backend as any })
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

const hidSaveScope = ref<'features' | 'identity'>('features')
async function saveHidSettings(scope: 'features' | 'identity' = 'features') {
  if (loading.value) return
  hidSaveScope.value = scope
  loading.value = true
  saveError.value = ''

  try {
    if (activeSection.value === 'hid') {
      if (scope === 'features' ? !isHidSettingsValid.value : !isCh9329DescriptorValid.value) {
        return
      }
      const hidUpdate: HidConfigUpdate = {}
      if (
        scope === 'identity' &&
        usesCh9329(config.value.hid_backend) &&
        isCh9329DescriptorDirty.value
      ) {
        hidUpdate.ch9329_descriptor = {
          vendor_id: parseInt(ch9329VendorIdHex.value, 16) || 0x1a86,
          product_id: parseInt(ch9329ProductIdHex.value, 16) || 0xe129,
          manufacturer: ch9329Manufacturer.value,
          product: ch9329Product.value,
          serial_number: ch9329SerialNumber.value || '',
        }
      }
      if (scope === 'identity' && supportsOtgFunctions(config.value.hid_backend)) {
        hidUpdate.otg_descriptor = {
          vendor_id: parseInt(otgVendorIdHex.value, 16) || 0x1d6b,
          product_id: parseInt(otgProductIdHex.value, 16) || 0x0104,
          manufacturer: otgManufacturer.value || 'One-KVM',
          product: otgProduct.value || 'One-KVM USB Device',
          serial_number: otgSerialNumber.value || undefined,
        }
      }
      if (scope === 'features' && config.value.hid_backend === 'otg') {
        hidUpdate.otg_profile = 'custom' as OtgHidProfile
        hidUpdate.otg_functions = { ...config.value.hid_otg_functions }
        hidUpdate.otg_keyboard_leds = config.value.hid_otg_keyboard_leds
      }
      if (scope === 'identity') {
        await configStore.updateHid(hidUpdate)
      } else if (supportsOtgFunctions(configStore.hid?.backend)) {
        const otgEnabled = supportsOtgFunctions(config.value.hid_backend)
        const response = await configStore.updateOtg({
          hid: hidUpdate,
          msd: {
            enabled: otgEnabled && config.value.msd_enabled,
            msd_dir: config.value.msd_dir || undefined,
            flash_inquiry_string: config.value.msd_flash_inquiry_string,
            cdrom_inquiry_string: config.value.msd_cdrom_inquiry_string,
          },
          network: {
            enabled: otgEnabled && config.value.otg_network_enabled,
            driver_mode: config.value.otg_network_driver as any,
            bridge_interface: config.value.otg_network_interface,
          },
          uac: {
            enabled: otgEnabled && config.value.uac_enabled,
            sample_rate: 48000,
            channels: 2,
          },
        })
        otgNetworkStatus.value = response.status

      } else if (usesCh9329(configStore.hid?.backend)) {
        await configStore.updateHid(hidUpdate)
      }
      if (scope === 'features') hidFeatureBaseline.value = hidFeatureSnapshot()
      else {
        otgIdentityBaseline.value = otgIdentitySnapshot()
        if (configStore.hid?.ch9329_descriptor)
          applyCh9329DescriptorForm(configStore.hid.ch9329_descriptor, true)
      }
    }

    if (activeSection.value !== 'hid') {
      await loadSectionData(activeSection.value)
    }
  } catch (error) {
    saveError.value = error instanceof Error ? error.message : t('api.operationFailedDesc')
  } finally {
    loading.value = false
  }
}

const identityOpen = ref(false)
const otgIdentityBaseline = ref('')
function otgIdentitySnapshot() {
  return JSON.stringify([
    otgVendorIdHex.value,
    otgProductIdHex.value,
    otgManufacturer.value,
    otgProduct.value,
    otgSerialNumber.value,
  ])
}
const hidFeatureBaseline = ref('')
function hidFeatureSnapshot() {
  return JSON.stringify({
    fields: Object.fromEntries(
      Object.entries(config.value).filter(
        ([key]) =>
          key.startsWith('msd_') ||
          key.startsWith('otg_network_') ||
          key.startsWith('uac_') ||
          ['hid_otg_functions', 'hid_otg_keyboard_leds'].includes(key),
      ),
    ),
  })
}
const hidFeaturesDirty = computed(
  () =>
    !!hidFeatureBaseline.value &&
    (hidFeatureBaseline.value !== hidFeatureSnapshot() ||
      isCh9329DescriptorDirty.value ||
      (!!otgIdentityBaseline.value && otgIdentityBaseline.value !== otgIdentitySnapshot())),
)

async function loadConfig() {
  try {
    const [video, stream, hid, msd, otgNetwork, uac] = await Promise.all([
      configStore.refreshVideo(),
      configStore.refreshStream(),
      configStore.refreshHid(),
      msdSupported.value ? configStore.refreshMsd() : Promise.resolve(null),
      otgSupported.value
        ? otgNetworkApi.get().catch(() => ({
            enabled: false,
            driver_mode: 'ncm' as const,
            bridge_interface: '',
            host_mac: '',
            device_mac: '',
          }))
        : Promise.resolve(null),
      otgSupported.value
        ? uacApi.get().catch(() => ({ enabled: false, sample_rate: 48000, channels: 2 }))
        : Promise.resolve(null),
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
      hid_otg_profile: 'custom' as OtgHidProfile,
      hid_otg_functions: {
        keyboard: hid.otg_functions?.keyboard ?? true,
        mouse_relative: hid.otg_functions?.mouse_relative ?? true,
        mouse_absolute: hid.otg_functions?.mouse_absolute ?? true,
        consumer: hid.otg_functions?.consumer ?? true,
      } as OtgHidFunctions,
      hid_otg_keyboard_leds: hid.otg_keyboard_leds ?? false,
      msd_enabled: msd?.enabled || false,
      msd_dir: msd?.msd_dir || '',
      msd_flash_inquiry_string: msd?.flash_inquiry_string || 'One-KVM Virtual Flash',
      msd_cdrom_inquiry_string: msd?.cdrom_inquiry_string || 'One-KVM Virtual CD-ROM',
      otg_network_enabled: otgNetwork?.enabled ?? false,
      otg_network_driver: otgNetwork?.driver_mode ?? 'ncm',
      uac_enabled: uac?.enabled ?? false,
      otg_network_interface: otgNetwork?.bridge_interface ?? '',
      encoder_backend: stream.encoder || 'auto',
      stun_server: stream.stun_server || '',
      turn_server: stream.turn_server || '',
      turn_username: stream.turn_username || '',
      turn_password: stream.turn_password || '',
    }
    if (otgNetworkInterfacesLoaded.value) {
      syncOtgNetworkInterface()
    }

    if (hid.otg_descriptor) {
      otgVendorIdHex.value = hid.otg_descriptor.vendor_id?.toString(16).padStart(4, '0') || '1d6b'
      otgProductIdHex.value = hid.otg_descriptor.product_id?.toString(16).padStart(4, '0') || '0104'
      otgManufacturer.value = hid.otg_descriptor.manufacturer || 'One-KVM'
      otgProduct.value = hid.otg_descriptor.product || 'One-KVM USB Device'
      otgSerialNumber.value = hid.otg_descriptor.serial_number || ''
    }
    if (hid.ch9329_descriptor) {
      if (!usesCh9329(hid.backend)) {
        applyCh9329DescriptorForm(hid.ch9329_descriptor, true)
      }
    }
    if (usesCh9329(hid.backend)) {
      await loadCh9329Descriptor()
    } else {
      clearCh9329DescriptorState()
    }
    otgNetworkStatus.value = otgSupported.value
      ? await otgNetworkApi.status().catch(() => null)
      : null
    await nextTick()
    hidFeatureBaseline.value = hidFeatureSnapshot()
    otgIdentityBaseline.value = otgIdentitySnapshot()
  } catch {}
}

async function loadDeviceConfig() {
  try {
    devices.value = await configApi.listDevices()
  } catch {}
}

async function loadOtgNetworkInterfaces() {
  otgNetworkInterfacesError.value = ''
  if (!otgSupported.value) {
    otgNetworkInterfaces.value = []
    otgNetworkInterfacesLoaded.value = false
    return
  }
  try {
    otgNetworkInterfaces.value = await otgNetworkApi.interfaces()
    otgNetworkInterfacesLoaded.value = true
    syncOtgNetworkInterface()
  } catch {
    otgNetworkInterfaces.value = []
    otgNetworkInterfacesLoaded.value = false
    otgNetworkInterfacesError.value = t('settings.otgNetworkInterfacesLoadFailed')
  }
}

async function loadHidDeviceOptions() {
  await Promise.all([loadDeviceConfig(), loadOtgNetworkInterfaces()])
}

async function loadBackends() {
  try {
    const result = await streamApi.getCodecs()
    availableBackends.value = result.backends || []
  } catch {}
}

async function loadAuthConfig() {
  authConfigLoading.value = true
  try {
    authConfig.value = await configStore.refreshAuth()
  } catch {
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
  } catch {
  } finally {
    authConfigLoading.value = false
  }
}

async function loadExtensions(only?: ExtensionConfigId) {
  extensionsLoading.value = true
  try {
    extensions.value = await extensionsApi.getAll()
    if (extensions.value) {
      const ttyd = extensions.value.ttyd.config
      if (!only || only === 'ttyd')
        extConfig.value.ttyd = {
          enabled: ttyd.enabled,
          shell: ttyd.shell,
        }
      if (!only || only === 'easyfrp')
        extConfig.value.easyfrp = { ...extensions.value.easyfrp.config }
      const easytier = extensions.value.easytier.config
      if (!only || only === 'easytier')
        extConfig.value.easytier = {
          enabled: easytier.enabled,
          config_mode: easytier.config_mode || EasytierConfigMode.Quick,
          network_name: easytier.network_name,
          network_secret: easytier.network_secret,
          peer_urls: easytier.peer_urls || [],
          virtual_ip: easytier.virtual_ip || '',
          custom_toml: easytier.custom_toml || '',
        }
      const frpc = extensions.value.frpc.config
      if (!only || only === 'frpc')
        extConfig.value.frpc = {
          enabled: frpc.enabled,
          config_mode: frpc.config_mode || FrpcConfigMode.Quick,
          proxy_name: frpc.proxy_name,
          proxy_type: frpc.proxy_type,
          server_addr: frpc.server_addr,
          server_port: frpc.server_port,
          token: frpc.token,
          local_ip: frpc.local_ip,
          local_port: frpc.local_port,
          remote_port: frpc.remote_port,
          custom_domain: frpc.custom_domain || '',
          secret_key: frpc.secret_key,
          tls: frpc.tls,
          custom_toml: frpc.custom_toml || '',
        }
    }
  } catch {
  } finally {
    extensionsLoading.value = false
  }
}

type ExtensionConfigId = 'ttyd' | 'easyfrp' | 'easytier' | 'frpc'
type ValidatedExtensionConfigId = Exclude<ExtensionConfigId, 'ttyd'>

const extensionBusy = ref<Record<ExtensionConfigId, boolean>>({
  ttyd: false,
  easyfrp: false,
  easytier: false,
  frpc: false,
})
async function startExtension(id: ExtensionConfigId) {
  if (extensionBusy.value[id]) return
  if (id !== 'ttyd' && !validateExtensionConfig(id)) return

  extensionBusy.value[id] = true
  try {
    await persistExtensionConfig(id)
    await extensionsApi.start(id)
    await loadExtensions(id)
  } catch {
  } finally {
    extensionBusy.value[id] = false
  }
}

async function stopExtension(id: ExtensionConfigId) {
  if (extensionBusy.value[id]) return
  extensionBusy.value[id] = true
  try {
    await extensionsApi.stop(id)
    await loadExtensions(id)
  } catch {
  } finally {
    extensionBusy.value[id] = false
  }
}

async function refreshExtensionLogs(id: ExtensionConfigId) {
  try {
    const result = await extensionsApi.logs(id, 100)
    extensionLogs.value[id] = result.logs
  } catch {}
}

async function persistExtensionConfig(id: ExtensionConfigId) {
  if (id === 'ttyd') {
    await extensionsApi.updateTtyd(extConfig.value.ttyd)
  } else if (id === 'easyfrp') {
    await extensionsApi.updateEasyfrp(extConfig.value.easyfrp)
  } else if (id === 'easytier') {
    await extensionsApi.updateEasytier(extConfig.value.easytier)
  } else if (id === 'frpc') {
    const frpc = extConfig.value.frpc
    await extensionsApi.updateFrpc({
      ...frpc,
      remote_port: frpcQuickMode.value && showFrpcRemotePort.value ? frpc.remote_port : undefined,
      custom_domain:
        frpcQuickMode.value && showFrpcCustomDomain.value
          ? frpc.custom_domain || undefined
          : undefined,
      secret_key: frpcQuickMode.value && showFrpcSecretKey.value ? frpc.secret_key : '',
    })
  }
}

async function saveExtensionConfig(id: ExtensionConfigId) {
  if (extensionBusy.value[id]) return
  if (id !== 'ttyd') {
    const shouldValidate =
      extConfig.value[id].enabled ||
      (id === 'easytier' && extConfig.value.easytier.config_mode === EasytierConfigMode.Full) ||
      (id === 'frpc' && extConfig.value.frpc.config_mode === FrpcConfigMode.Full)
    if (shouldValidate && !validateExtensionConfig(id)) return
  }

  loading.value = true
  extensionBusy.value[id] = true
  try {
    await persistExtensionConfig(id)
    await loadExtensions(id)
    toast.success(t('settingsUi.saveDone'))
  } catch {
  } finally {
    extensionBusy.value[id] = false
    loading.value = false
  }
}

function isExtRunning(status: ExtensionStatus | undefined): boolean {
  return status?.state === 'running'
}

function openTerminal() {
  showTerminalDialog.value = true
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

async function loadAtxConfig() {
  try {
    const config = await configStore.refreshAtx()
    atxConfig.value = {
      enabled: config.enabled,
      driver: config.enabled ? config.driver : ('none' as AtxDriverType),
      device: config.device || '',
      baud_rate: config.baud_rate || 9600,
      power: {
        ...config.power,
        active_level: config.power.active_level || 'high',
      },
      reset: {
        ...config.reset,
        active_level: config.reset.active_level || 'high',
      },
      led: {
        ...config.led,
        active_level: config.led.active_level || 'high',
      },
      hdd: {
        ...config.hdd,
        active_level: config.hdd.active_level || 'high',
      },
      wol_interface: config.wol_interface || '',
    }
    clearAtxSerialDeviceConflicts()
    normalizeAtxRelayChannels()
  } catch {}
}

async function loadAtxDevices() {
  try {
    atxDevices.value = await atxConfigApi.listDevices()
  } catch {}
}

async function saveAtxSettings() {
  atxSaving.value = true
  atxSaved.value = false
  try {
    normalizeAtxRelayChannels()
    const isGpio = atxConfig.value.driver === 'gpio'
    const isRelay = ['usbrelay', 'serial'].includes(atxConfig.value.driver)
    await configStore.updateAtx({
      enabled: atxConfig.value.driver !== 'none',
      driver: atxConfig.value.driver,
      device: atxConfig.value.device || undefined,
      baud_rate: atxConfig.value.baud_rate,
      power: {
        enabled: isGpio ? !!atxConfig.value.power.device : isRelay,
        device: atxConfig.value.power.device || undefined,
        pin: atxConfig.value.power.pin,
        active_level: atxConfig.value.power.active_level,
      },
      reset: {
        enabled: isGpio ? !!atxConfig.value.reset.device : isRelay,
        device: atxConfig.value.reset.device || undefined,
        pin: atxConfig.value.reset.pin,
        active_level: atxConfig.value.reset.active_level,
      },
      led: {
        enabled: isGpio && !!atxConfig.value.led.device,
        device: atxConfig.value.led.device || undefined,
        pin: atxConfig.value.led.pin,
        active_level: atxConfig.value.led.active_level,
      },
      hdd: {
        enabled: isGpio && !!atxConfig.value.hdd.device,
        device: atxConfig.value.hdd.device || undefined,
        pin: atxConfig.value.hdd.pin,
        active_level: atxConfig.value.hdd.active_level,
      },
    })
    atxConfig.value.enabled = atxConfig.value.driver !== 'none'
    atxSaved.value = true
    setTimeout(() => (atxSaved.value = false), 2000)
  } catch {
  } finally {
    atxSaving.value = false
  }
}

async function saveWolSettings() {
  wolSaving.value = true
  wolSaved.value = false
  try {
    await configStore.updateAtx({
      wol_interface: atxConfig.value.wol_interface || undefined,
    })
    wolSaved.value = true
    setTimeout(() => (wolSaved.value = false), 2000)
  } catch {
  } finally {
    wolSaving.value = false
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

function isAtxSerialDeviceReserved(device: string): boolean {
  const reserved = ch9329ReservedSerialDevice.value
  return !!reserved && device === reserved
}

function formatAtxDeviceLabel(driver: string, device: string): string {
  if (driver === 'serial' && isAtxSerialDeviceReserved(device)) {
    return `${device} (CH9329 in use)`
  }
  return device
}

function clearAtxSerialDeviceConflicts() {
  const reserved = ch9329ReservedSerialDevice.value
  if (!reserved) return

  if (atxConfig.value.driver === 'serial' && atxConfig.value.device === reserved) {
    atxConfig.value.device = ''
  }
}

function normalizeAtxRelayChannels() {
  for (const key of [atxConfig.value.power, atxConfig.value.reset]) {
    if (['usbrelay', 'serial'].includes(atxConfig.value.driver) && key.pin < 1) {
      key.pin = 1
    }
  }
  if (atxConfig.value.driver !== 'gpio') {
    atxConfig.value.led.enabled = false
    atxConfig.value.hdd.enabled = false
  }
}

watch(
  () => [config.value.hid_backend, config.value.hid_serial_device],
  () => {
    clearAtxSerialDeviceConflicts()
  },
)

watch(
  () => atxConfig.value.driver,
  () => {
    normalizeAtxRelayChannels()
    clearAtxSerialDeviceConflicts()
  },
)

function showValidationError(message: string): boolean {
  toast.error(t('api.operationFailed'), {
    description: message,
    duration: 4000,
  })
  return false
}

function validateExtensionConfig(id: ValidatedExtensionConfigId): boolean {
  let message = ''
  if (id === 'easyfrp') {
    message = easyfrpValidationMessage.value
  } else if (id === 'easytier') {
    message = easytierValidationMessage.value
  } else {
    message = frpcValidationMessage.value
  }

  return !message || showValidationError(message)
}

function normalizeBindAddresses(addresses: string[]): string[] {
  return addresses.map((addr) => addr.trim()).filter(Boolean)
}

function applyBindStateFromConfig(config: WebConfig) {
  const rawAddrs =
    config.bind_addresses && config.bind_addresses.length > 0
      ? config.bind_addresses
      : config.bind_address
        ? [config.bind_address]
        : []
  const addrs = normalizeBindAddresses(rawAddrs)
  const isAll =
    addrs.length > 0 &&
    addrs.every((addr) => addr === '0.0.0.0' || addr === '::') &&
    addrs.includes('0.0.0.0')
  const isLoopback =
    addrs.length > 0 &&
    addrs.every((addr) => addr === '127.0.0.1' || addr === '::1') &&
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

async function loadWebServerConfig() {
  try {
    const config = await configStore.refreshWeb()
    webServerConfig.value = config
    applyBindStateFromConfig(config)
  } catch {}
}

async function loadRedfishConfig() {
  try {
    const data = await redfishConfigApi.get()
    redfishEnabled.value = data.enabled
  } catch {}
}

async function saveRedfishConfig() {
  redfishSaving.value = true
  try {
    const data = await redfishConfigApi.update({
      enabled: redfishEnabled.value,
    })
    redfishEnabled.value = data.enabled
    await triggerAutoRestart()
  } catch {
  } finally {
    redfishSaving.value = false
  }
}

async function saveWebServerConfig(scope: 'ports' | 'listen') {
  if (scope === 'listen' && bindAddressError.value) return
  webServerLoading.value = true
  try {
    const updated = await configStore.updateWeb(
      scope === 'ports'
        ? {
            http_port: webServerConfig.value.http_port,
            https_port: webServerConfig.value.https_port,
            https_enabled: webServerConfig.value.https_enabled,
          }
        : { bind_addresses: effectiveBindAddresses.value },
    )
    if (scope === 'ports') {
      webServerConfig.value.http_port = updated.http_port
      webServerConfig.value.https_port = updated.https_port
      webServerConfig.value.https_enabled = updated.https_enabled
    } else {
      applyBindStateFromConfig(updated)
    }
    await triggerAutoRestart()
  } catch {
  } finally {
    webServerLoading.value = false
  }
}

async function saveCertificate() {
  if (!sslCertPem.value.trim() || !sslKeyPem.value.trim()) return
  certSaving.value = true
  try {
    const updated = await configStore.updateWeb({
      ssl_cert_pem: sslCertPem.value,
      ssl_key_pem: sslKeyPem.value,
    })
    webServerConfig.value = updated
    sslCertPem.value = ''
    sslKeyPem.value = ''
    await triggerAutoRestart()
  } catch {
  } finally {
    certSaving.value = false
  }
}

async function clearCertificate() {
  certClearing.value = true
  try {
    const updated = await configStore.updateWeb({ clear_custom_cert: true })
    webServerConfig.value = updated
    await triggerAutoRestart()
  } catch {
  } finally {
    certClearing.value = false
  }
}

/** 手动点重启按钮（仅用于弹窗场景，保留兼容） */
async function restartServer() {
  restarting.value = true
  try {
    await systemApi.restart()
    setTimeout(() => {
      const protocol = webServerConfig.value.https_enabled ? 'https' : 'http'
      const port = webServerConfig.value.https_enabled
        ? webServerConfig.value.https_port
        : webServerConfig.value.http_port
      const host = formatHostForUrl(window.location.hostname || '127.0.0.1')
      window.location.href = `${protocol}://${host}:${port}`
    }, 3000)
  } catch {
    restarting.value = false
  }
}

/** 轮询目标地址 /api/health，最多等待 maxMs 毫秒 */
async function pollUntilReady(targetOrigin: string, maxMs = 30000): Promise<boolean> {
  const deadline = Date.now() + maxMs
  const healthUrl = targetOrigin.replace(/\/$/, '') + '/api/health'
  while (Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 800))
    try {
      const ctrl = new AbortController()
      const tid = setTimeout(() => ctrl.abort(), 1500)
      const res = await fetch(healthUrl, { signal: ctrl.signal })
      clearTimeout(tid)
      if (res.ok) return true
    } catch {}
  }
  return false
}

/**
 * 保存网络配置后自动重启并跳转。
 *
 * - HTTP 目标：轮询 /api/health，服务恢复后自动跳转。
 * - HTTPS 目标：自签名证书导致 fetch 被浏览器拦截（ERR_CERT_AUTHORITY_INVALID），
 *   无法自动轮询。改为倒计时结束后展示跳转链接，由用户点击并在浏览器中手动接受证书。
 */
async function triggerAutoRestart() {
  const https = webServerConfig.value.https_enabled
  const port = https ? webServerConfig.value.https_port : webServerConfig.value.http_port
  const protocol = https ? 'https' : 'http'
  const host = formatHostForUrl(window.location.hostname || '127.0.0.1')
  const targetOrigin = `${protocol}://${host}:${port}`

  autoRestarting.value = true
  autoRestartFailed.value = false
  autoRestartManualUrl.value = null

  try {
    await systemApi.restart()

    if (https) {
      // HTTPS：浏览器拒绝自签名证书，无法轮询。
      // 等待固定时间后展示手动跳转链接。
      const WAIT_SEC = 6
      autoRestartCountdown.value = WAIT_SEC
      for (let i = WAIT_SEC - 1; i >= 0; i--) {
        await new Promise((r) => setTimeout(r, 1000))
        autoRestartCountdown.value = i
      }
      autoRestartManualUrl.value = targetOrigin
      autoRestarting.value = false
    } else {
      // HTTP：可以安全轮询，服务恢复后自动跳转。
      await new Promise((r) => setTimeout(r, 1200))
      const ready = await pollUntilReady(targetOrigin)
      if (ready) {
        window.location.href = targetOrigin
      } else {
        autoRestartFailed.value = true
        autoRestarting.value = false
      }
    }
  } catch {
    autoRestartFailed.value = true
    autoRestarting.value = false
  }
}

async function loadUpdateOverview() {
  updateLoading.value = true
  try {
    updateOverview.value = await updateApi.overview(updateChannel.value)
  } catch (e) {
    const message = e instanceof Error ? e.message : t('settings.updateOverviewLoadFailed')
    toast.error(t('settings.updateOverviewLoadFailed'), {
      description: message,
      duration: 4000,
    })
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
        router.replace('/login')
      }
    }
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) {
      updateAutoReloadTriggered.value = true
      authStore.isAuthenticated = false
      authStore.user = null
      stopUpdatePolling()
      router.replace('/login')
      return
    }
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
  if (!onlineUpgradeSupported.value) return
  try {
    updateSawRestarting.value = false
    updateSawRequestFailure.value = false
    updateAutoReloadTriggered.value = false
    await updateApi.upgrade({ channel: updateChannel.value })
    await refreshUpdateStatus()
    startUpdatePolling()
  } catch {}
}

function updatePhaseText(phase?: string): string {
  switch (phase) {
    case 'idle':
      return t('settings.updatePhaseIdle')
    case 'checking':
      return t('settings.updatePhaseChecking')
    case 'downloading':
      return t('settings.updatePhaseDownloading')
    case 'verifying':
      return t('settings.updatePhaseVerifying')
    case 'installing':
      return t('settings.updatePhaseInstalling')
    case 'restarting':
      return t('settings.updatePhaseRestarting')
    case 'success':
      return t('settings.updatePhaseSuccess')
    case 'failed':
      return t('settings.updatePhaseFailed')
    default:
      return t('common.unknown')
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
  return (
    localizeUpdateMessage(updateStatus.value?.message) || updatePhaseText(updateStatus.value?.phase)
  )
}

onMounted(async () => {
  const initialSection = normalizeSettingsSection(route.query.tab)
  if (initialSection) {
    activeSection.value = initialSection
  }

  await systemStore.fetchSystemInfo()
  ensureVisibleSection()
  usernameInput.value = authStore.user || ''
  await loadSectionData(activeSection.value)

  if (updateRunning.value) {
    startUpdatePolling()
  }
})

watch(updateChannel, async () => {
  if (activeSection.value === 'about') {
    await loadUpdateOverview()
  }
})

watch(
  () => config.value.hid_backend,
  () => {
    otgSelfCheckResult.value = null
    otgSelfCheckError.value = ''
    if (usesCh9329(config.value.hid_backend)) {
      void loadCh9329Descriptor()
    } else {
      clearCh9329DescriptorState()
    }
  },
)

watch(
  () => [config.value.hid_serial_device, config.value.hid_serial_baudrate],
  () => {
    if (usesCh9329(config.value.hid_backend) && !isCh9329DescriptorSourceCurrent.value) {
      clearCh9329DescriptorState()
    }
  },
)

watch(
  () => route.query.tab,
  (tab) => {
    const section = normalizeSettingsSection(tab)
    if (section && activeSection.value !== section) {
      selectSection(section)
    }
  },
)

watch(isWindows, () => {
  ensureVisibleSection()
})
</script>

<template>
  <AppLayout>
    <SidebarProvider
      class="settings-layout h-full min-h-0 overflow-hidden"
      :style="{ '--sidebar-width': '14rem' }"
    >
      <Sidebar
        class="top-10 h-[calc(100dvh-2.5rem)] sm:top-14 sm:h-[calc(100dvh-3.5rem)]"
        collapsible="offcanvas"
      >
        <SidebarHeader class="gap-1 px-6 pt-4 pb-2 text-foreground md:pt-6">
          <h1 class="text-xl font-semibold">{{ t('settings.title') }}</h1>
        </SidebarHeader>
        <SidebarContent class="px-3 pb-6 md:pb-10">
          <SidebarGroup v-for="group in navGroups" :key="group.id" class="px-0 py-1">
            <SidebarGroupLabel class="text-[13px] text-muted-foreground">{{ group.title }}</SidebarGroupLabel>
            <SidebarMenu>
              <SidebarMenuItem v-for="item in group.items" :key="item.id">
                <SidebarMenuButton
                  :is-active="activeSection === item.id"
                  :tooltip="item.label"
                  class="h-8 px-3 text-foreground data-[active=true]:bg-primary data-[active=true]:text-primary-foreground data-[active=true]:shadow-sm data-[active=true]:hover:bg-primary data-[active=true]:hover:text-primary-foreground"
                  @click="selectSection(item.id)"
                >
                  <component :is="item.icon" />
                  <span>{{ item.label }}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem v-if="systemStore.openwrtSystem && group.id === 'extensions'">
                <SidebarMenuButton
                  as-child
                  :tooltip="t('nav.openwrtSystemManagement')"
                  class="h-8 px-3 text-foreground"
                >
                  <a
                    href="/api/system/openwrt"
                    target="_blank"
                    rel="noopener noreferrer"
                    :title="t('nav.openwrtSystemManagement')"
                  >
                    <Server aria-hidden="true" />
                    <span class="min-w-0 flex-1 truncate">{{
                      t('nav.openwrtSystemManagement')
                    }}</span>
                    <ExternalLink
                      class="!size-3.5 shrink-0 text-muted-foreground"
                      aria-hidden="true"
                    />
                  </a>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroup>
        </SidebarContent>
        <div class="settings-back mx-3 pb-3">
          <Button as-child variant="ghost" class="justify-start"
            ><RouterLink to="/"
              ><ArrowLeft class="size-4" />{{ t('settingsUi.back') }}</RouterLink
            ></Button
          >
        </div>
      </Sidebar>

      <SidebarInset class="min-w-0">
        <!-- Mobile Header -->
        <div
          class="md:hidden sticky top-0 z-20 flex items-center px-3 sm:px-4 py-2 sm:py-3 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/70"
        >
          <SidebarTrigger class="mr-1.5 size-8 sm:mr-2 sm:size-9" />
          <div class="flex items-center gap-2 min-w-0">
            <component :is="sectionMeta.icon" class="size-4 text-muted-foreground shrink-0" />
            <h1 class="text-sm sm:text-base font-semibold truncate">{{ sectionMeta.title }}</h1>
          </div>
        </div>
        <div class="settings-page" :data-section="activeSection">
          <!-- Section Header -->
          <header class="settings-section-heading">
            <div class="flex items-center gap-2.5">
              <component :is="sectionMeta.icon" class="size-5 text-muted-foreground" />
              <h1 class="text-xl font-semibold tracking-tight">{{ sectionMeta.title }}</h1>
            </div>
            <p v-if="sectionMeta.description" class="text-sm text-muted-foreground">
              {{ sectionMeta.description }}
            </p>
          </header>

          <!-- Appearance Section -->
          <div v-show="activeSection === 'appearance'" class="space-y-4">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.themeDesc')">{{
                  t('settings.theme')
                }}</CardTitle></CardHeader
              >
              <CardContent>
                <div class="grid grid-cols-3 gap-2 sm:max-w-md">
                  <Button
                    :variant="theme === 'light' ? 'default' : 'outline'"
                    size="sm"
                    class="justify-center"
                    @click="setTheme('light')"
                  >
                    <Sun class="size-4 mr-1.5" />{{ t('settings.lightMode') }}
                  </Button>
                  <Button
                    :variant="theme === 'dark' ? 'default' : 'outline'"
                    size="sm"
                    class="justify-center"
                    @click="setTheme('dark')"
                  >
                    <Moon class="size-4 mr-1.5" />{{ t('settings.darkMode') }}
                  </Button>
                  <Button
                    :variant="theme === 'system' ? 'default' : 'outline'"
                    size="sm"
                    class="justify-center"
                    @click="setTheme('system')"
                  >
                    <Monitor class="size-4 mr-1.5" />{{ t('settings.systemMode') }}
                  </Button>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.consoleLayoutDesc')">{{
                  t('settings.consoleLayout')
                }}</CardTitle></CardHeader
              >
              <CardContent>
                <div class="grid gap-3 sm:grid-cols-3">
                  <button
                    v-for="option in consoleLayoutOptions"
                    :key="option.value"
                    type="button"
                    class="group rounded-lg border p-3 text-left transition-colors hover:bg-accent"
                    :class="
                      consoleLayout === option.value
                        ? 'border-primary bg-primary/5 ring-1 ring-primary'
                        : 'border-border'
                    "
                    :aria-pressed="consoleLayout === option.value"
                    @click="setConsoleLayout(option.value)"
                  >
                    <ConsoleLayoutPreview :layout="option.value" class="mb-3" />
                    <div class="flex items-center gap-2">
                      <component :is="option.icon" class="size-4 text-muted-foreground" />
                      <span class="text-sm font-medium">{{
                        t(`settings.consoleLayoutOptions.${option.value}`)
                      }}</span>
                      <Check
                        v-if="consoleLayout === option.value"
                        class="ml-auto size-4 text-primary"
                      />
                    </div>
                  </button>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.languageDesc')">{{
                  t('settings.language')
                }}</CardTitle></CardHeader
              >
              <CardContent>
                <LanguageToggleButton variant="outline" size="sm" label-mode="current" />
              </CardContent>
            </Card>

            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.featureVisibilityDesc')">{{
                  t('settings.featureVisibility')
                }}</CardTitle></CardHeader
              >
              <CardContent class="space-y-1">
                <div class="settings-toggle flex items-center justify-between gap-4 px-3 py-3">
                  <Label for="feature-power" class="flex min-w-0 items-center gap-2 font-normal">
                    <Power class="size-4 shrink-0 text-muted-foreground" />
                    <span class="truncate">{{ t('actionbar.power') }}</span>
                  </Label>
                  <Switch id="feature-power" v-model="featureVisibility.power" />
                </div>
                <div class="settings-toggle flex items-center justify-between gap-4 px-3 py-3">
                  <Label
                    for="feature-paste-text"
                    class="flex min-w-0 items-center gap-2 font-normal"
                  >
                    <ClipboardPaste class="size-4 shrink-0 text-muted-foreground" />
                    <span class="truncate">{{ t('settings.pasteText') }}</span>
                  </Label>
                  <Switch id="feature-paste-text" v-model="featureVisibility.pasteText" />
                </div>
                <div class="settings-toggle flex items-center justify-between gap-4 px-3 py-3">
                  <Label
                    for="feature-web-terminal"
                    class="flex min-w-0 items-center gap-2 font-normal"
                  >
                    <Terminal class="size-4 shrink-0 text-muted-foreground" />
                    <span class="truncate">{{ t('actionbar.webTerminal') }}</span>
                  </Label>
                  <Switch id="feature-web-terminal" v-model="featureVisibility.webTerminal" />
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- Account Section -->
          <McpSettingsCard v-if="activeSection === 'mcp'" />

          <div v-show="activeSection === 'account'" class="space-y-4">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.usernameDesc')">{{
                  t('settings.username')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button @click="changeUsername" :disabled="usernameSaving">
                      <Loader2 v-if="usernameSaving" class="size-4 mr-2 animate-spin" />
                      <Save v-else class="size-4 mr-2" />
                      {{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-field space-y-2">
                  <Label for="account-username">{{ t('settings.username') }}</Label>
                  <Input id="account-username" v-model="usernameInput" autocomplete="username" />
                </div>
                <div class="settings-field space-y-2">
                  <Label for="account-username-password">{{ t('settings.currentPassword') }}</Label>
                  <Input
                    id="account-username-password"
                    v-model="usernamePassword"
                    type="password"
                    autocomplete="current-password"
                  />
                </div>
                <p v-if="usernameError" class="text-xs text-destructive">{{ usernameError }}</p>
                <p v-else-if="usernameSaved" class="flex items-center gap-1.5 text-xs text-success">
                  <Check class="size-3.5" />{{ t('common.success') }}
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.passwordDesc')">{{
                  t('settings.changePassword')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button @click="changePassword" :disabled="passwordSaving">
                      <Loader2 v-if="passwordSaving" class="size-4 mr-2 animate-spin" />
                      <Save v-else class="size-4 mr-2" />
                      {{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-field space-y-2">
                  <Label for="account-current-password">{{ t('settings.currentPassword') }}</Label>
                  <Input
                    id="account-current-password"
                    v-model="currentPassword"
                    type="password"
                    autocomplete="current-password"
                  />
                </div>
                <div class="settings-field space-y-2">
                  <Label for="account-new-password">{{ t('settings.newPassword') }}</Label>
                  <Input
                    id="account-new-password"
                    v-model="newPassword"
                    type="password"
                    autocomplete="new-password"
                  />
                </div>
                <div class="settings-field space-y-2">
                  <Label for="account-confirm-password">{{ t('auth.confirmPassword') }}</Label>
                  <Input
                    id="account-confirm-password"
                    v-model="confirmPassword"
                    type="password"
                    autocomplete="new-password"
                  />
                </div>
                <p v-if="passwordError" class="text-xs text-destructive">{{ passwordError }}</p>
                <p v-else-if="passwordSaved" class="flex items-center gap-1.5 text-xs text-success">
                  <Check class="size-3.5" />{{ t('common.success') }}
                </p>
              </CardContent>
            </Card>

            <TotpSettingsCard />

            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.authSettingsDesc')">{{
                  t('settings.authSettings')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button @click="saveAuthConfig" :disabled="authConfigLoading">
                      <Loader2 v-if="authConfigLoading" class="size-4 mr-2 animate-spin" />
                      <Save v-else class="size-4 mr-2" />
                      {{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-toggle flex items-start justify-between gap-4">
                  <Label>{{ t('settings.allowMultipleSessions') }}</Label>
                  <Switch
                    v-model="authConfig.single_user_allow_multiple_sessions"
                    :disabled="authConfigLoading"
                  />
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- Video Section -->
          <div v-show="activeSection === 'video'" class="space-y-4">
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

          <!-- HID Section -->
          <div v-show="activeSection === 'hid'" class="space-y-4">
            <HidDeviceSettings
              v-if="activeSection === 'hid'"
              :active="activeSection === 'hid'"
              :dirty="hidFeaturesDirty"
              @applied="loadConfig"
              @discard="loadConfig"
            />
            <Card v-if="supportsOtgFunctions(configStore.hid?.backend)" class="hid-feature-card"
              ><CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settingsUi.featuresHelp')">{{
                  t('hidGuide.features')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    :disabled="loading || !isHidSettingsValid"
                    @click="saveHidSettings('features')"
                    ><Loader2 v-if="loading" class="size-4 animate-spin" /><Save
                      v-else
                      class="size-4"
                    />{{ t('common.save') }}</Button
                  >
                </div></CardHeader
              ><CardContent class="space-y-4"
                ><div class="space-y-4">
                  <div>
                    <h4 class="text-sm font-medium">{{ t('settings.otgHidProfile') }}</h4>
                  </div>
                  <div class="space-y-3">
                    <div
                      class="grid gap-3"
                      :class="{ 'md:grid-cols-2': config.hid_backend === 'otg' }"
                    >
                      <div
                        v-if="config.hid_backend === 'otg'"
                        class="space-y-3 rounded-md border border-border/60 p-3"
                      >
                        <div class="flex items-center justify-between gap-4">
                          <div>
                            <Label>{{ t('settings.otgFunctionKeyboard') }}</Label>
                          </div>
                          <Switch v-model="config.hid_otg_functions.keyboard" />
                        </div>
                        <Separator />
                        <div class="flex items-center justify-between gap-4">
                          <div>
                            <Label>{{ t('settings.otgFunctionConsumer') }}</Label>
                          </div>
                          <Switch v-model="config.hid_otg_functions.consumer" />
                        </div>
                        <Separator />
                        <div class="flex items-center justify-between gap-4">
                          <div>
                            <Label>{{ t('settings.otgKeyboardLeds') }}</Label>
                          </div>
                          <Switch
                            v-model="config.hid_otg_keyboard_leds"
                            :disabled="isKeyboardLedToggleDisabled"
                          />
                        </div>
                      </div>
                      <div class="space-y-3 rounded-md border border-border/60 p-3">
                        <template v-if="config.hid_backend === 'otg'">
                          <div class="flex items-center justify-between gap-4">
                            <div>
                              <Label>{{ t('settings.otgFunctionMouseRelative') }}</Label>
                            </div>
                            <Switch v-model="config.hid_otg_functions.mouse_relative" />
                          </div>
                          <Separator />
                          <div class="flex items-center justify-between gap-4">
                            <div>
                              <Label>{{ t('settings.otgFunctionMouseAbsolute') }}</Label>
                            </div>
                            <Switch v-model="config.hid_otg_functions.mouse_absolute" />
                          </div>
                          <p v-if="hasMacosDragInterfaceConflict" class="text-xs text-warning">
                            {{ t('settings.mouseMacosDragRequiresBoth') }}
                          </p>
                          <Separator />
                        </template>
                        <div class="flex items-center justify-between gap-4">
                          <Label>{{ t('settings.uacMic') }}</Label>
                          <Switch v-model="config.uac_enabled" />
                        </div>
                      </div>
                    </div>
                    <div class="space-y-3 rounded-md border border-border/60 p-3">
                      <div class="flex items-center justify-between gap-4">
                        <div>
                          <Label>{{ t('settings.otgFunctionMsd') }}</Label>
                        </div>
                        <Switch v-model="config.msd_enabled" />
                      </div>
                      <fieldset :disabled="!config.msd_enabled" class="space-y-4">
                        <Separator />
                        <div class="space-y-2">
                          <Label for="msd-dir">{{ t('settings.msdDir') }}</Label>
                          <Input
                            id="msd-dir"
                            v-model="config.msd_dir"
                            placeholder="/etc/one-kvm/msd"
                          />
                        </div>
                        <div class="space-y-2">
                          <Label for="msd-flash-inquiry-string">{{
                            t('settings.msdFlashInquiryString')
                          }}</Label>
                          <Input
                            id="msd-flash-inquiry-string"
                            v-model="config.msd_flash_inquiry_string"
                            placeholder="One-KVM Virtual Flash"
                            maxlength="28"
                          />
                        </div>
                        <div class="space-y-2">
                          <Label for="msd-cdrom-inquiry-string">{{
                            t('settings.msdCdromInquiryString')
                          }}</Label>
                          <Input
                            id="msd-cdrom-inquiry-string"
                            v-model="config.msd_cdrom_inquiry_string"
                            placeholder="One-KVM Virtual CD-ROM"
                            maxlength="28"
                          />
                        </div>
                      </fieldset>
                    </div>
                    <div class="space-y-3 rounded-md border border-border/60 p-3">
                      <div class="flex items-center justify-between gap-4">
                        <div>
                          <Label>{{ t('settings.otgNetwork') }}</Label>
                        </div>
                        <Switch v-model="config.otg_network_enabled" />
                      </div>
                      <fieldset :disabled="!config.otg_network_enabled" class="space-y-4">
                        <Separator />
                        <div class="grid gap-4 sm:grid-cols-2">
                          <div class="space-y-2">
                            <Label for="otg-network-driver">{{
                              t('settings.otgNetworkDriver')
                            }}</Label>
                            <Select v-model="config.otg_network_driver">
                              <SelectTrigger id="otg-network-driver" class="w-full"
                                ><SelectValue
                              /></SelectTrigger>
                              <SelectContent>
                                <SelectItem value="ncm">NCM</SelectItem>
                                <SelectItem value="ecm">ECM</SelectItem>
                                <SelectItem value="rndis">RNDIS / Windows</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                          <div class="space-y-2">
                            <Label for="otg-network-interface">{{
                              t('settings.otgNetworkInterface')
                            }}</Label>
                            <Select
                              :model-value="config.otg_network_interface"
                              :disabled="otgNetworkInterfaces.length === 0"
                              @update:model-value="
                                (value) =>
                                  (config.otg_network_interface =
                                    value === EMPTY_SELECT_VALUE ? '' : String(value))
                              "
                            >
                              <SelectTrigger
                                id="otg-network-interface"
                                class="w-full"
                                :aria-invalid="Boolean(otgNetworkInterfacesError)"
                              >
                                <SelectValue
                                  :placeholder="
                                    otgNetworkInterfacesError || t('settings.otgNetworkNone')
                                  "
                                />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem
                                  v-if="otgNetworkInterfacesError"
                                  :value="config.otg_network_interface || EMPTY_SELECT_VALUE"
                                >
                                  {{ otgNetworkInterfacesError }}
                                </SelectItem>
                                <SelectItem
                                  v-else-if="otgNetworkInterfaces.length === 0"
                                  :value="EMPTY_SELECT_VALUE"
                                >
                                  {{ t('settings.otgNetworkNone') }}
                                </SelectItem>
                                <SelectItem
                                  v-for="item in otgNetworkInterfaces"
                                  :key="item.name"
                                  :value="item.name"
                                >
                                  {{ item.name }} · {{ item.interface_type }}
                                </SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                        </div>
                      </fieldset>
                      <p
                        v-if="otgNetworkStatus?.health === 'degraded'"
                        class="text-xs text-destructive"
                      >
                        {{ t('settings.otgRuntimeDegraded') }}:
                        {{ otgNetworkStatus.error || t('common.error') }}
                      </p>
                    </div>
                  </div>
                  <p v-if="config.hid_backend === 'otg'" class="text-xs text-warning">
                    {{ t('settings.otgProfileWarning') }}
                  </p>
                </div>
                <p v-if="!isHidFunctionSelectionValid" class="text-sm text-warning">
                  {{ t('settings.otgFunctionMinWarning') }}
                </p>
                <p
                  v-if="saveError && hidSaveScope === 'features'"
                  role="alert"
                  class="text-sm text-destructive"
                >
                  {{ saveError }}
                </p></CardContent
              ></Card
            >
            <Card v-if="supportsOtgFunctions(configStore.hid?.backend) || usesCh9329(configStore.hid?.backend)"
              ><CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settingsUi.identifiersHelp')">{{
                  t('settingsUi.identifiers')
                }}</CardTitle
                ><template v-if="identityOpen"
                  ><div class="settings-card-actions">
                    <Button
                      :disabled="
                        loading ||
                        !isCh9329DescriptorValid ||
                        (usesCh9329(config.hid_backend) && !canEditCh9329Descriptor)
                      "
                      @click="saveHidSettings('identity')"
                      ><Loader2 v-if="loading" class="size-4 animate-spin" /><Save
                        v-else
                        class="size-4"
                      />{{ t('common.save') }}</Button
                    >
                  </div></template
                ></CardHeader
              ><CardContent
                ><Collapsible v-model:open="identityOpen"
                  ><CollapsibleTrigger as-child
                    ><Button variant="ghost" class="text-muted-foreground"
                      ><ChevronRight :class="['size-4', identityOpen ? 'rotate-90' : '']" />VID /
                      PID · {{ t('settings.manufacturer') }} ·
                      {{ t('settings.productName') }}</Button
                    ></CollapsibleTrigger
                  ><CollapsibleContent class="space-y-4 pt-4"
                    ><template v-if="usesCh9329(config.hid_backend)">
                      <Separator class="my-4" />
                      <div class="settings-field-list space-y-4">
                        <div>
                          <div class="flex items-start justify-between gap-3">
                            <div>
                              <h4 class="text-sm font-medium">
                                {{ t('settings.ch9329Descriptor') }}
                              </h4>
                              <p class="text-sm text-muted-foreground">
                                {{ t('settings.ch9329DescriptorDesc') }}
                              </p>
                            </div>
                            <Button
                              variant="ghost"
                              size="icon-sm"
                              class="shrink-0"
                              :aria-label="t('common.refresh')"
                              :disabled="ch9329DescriptorLoading"
                              @click="loadCh9329Descriptor"
                            >
                              <RefreshCw
                                class="size-4"
                                :class="{ 'animate-spin': ch9329DescriptorLoading }"
                              />
                            </Button>
                          </div>
                        </div>
                        <p
                          v-if="ch9329DescriptorLoading"
                          class="text-sm text-muted-foreground flex items-center gap-2"
                        >
                          <Loader2 class="size-4 animate-spin" />
                          {{ t('settings.ch9329DescriptorLoading') }}
                        </p>
                        <p v-else-if="ch9329DescriptorError" class="text-sm text-destructive">
                          {{ ch9329DescriptorError }}
                        </p>
                        <div class="settings-field-list grid gap-4 sm:grid-cols-2">
                          <div class="settings-field space-y-2">
                            <Label for="ch9329-vid">{{ t('settings.vendorId') }}</Label>
                            <Input
                              id="ch9329-vid"
                              v-model="ch9329VendorIdHex"
                              placeholder="1a86"
                              maxlength="4"
                              :disabled="!canEditCh9329Descriptor"
                              @input="validateHex($event, 'ch9329-vid')"
                            />
                          </div>
                          <div class="settings-field space-y-2">
                            <Label for="ch9329-pid">{{ t('settings.productId') }}</Label>
                            <Input
                              id="ch9329-pid"
                              v-model="ch9329ProductIdHex"
                              placeholder="e129"
                              maxlength="4"
                              :disabled="!canEditCh9329Descriptor"
                              @input="validateHex($event, 'ch9329-pid')"
                            />
                          </div>
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="ch9329-manufacturer">{{ t('settings.manufacturer') }}</Label>
                          <Input
                            id="ch9329-manufacturer"
                            v-model="ch9329Manufacturer"
                            placeholder="WCH.CN"
                            maxlength="23"
                            :disabled="!canEditCh9329Descriptor"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="ch9329-product">{{ t('settings.productName') }}</Label>
                          <Input
                            id="ch9329-product"
                            v-model="ch9329Product"
                            placeholder="CH9329"
                            maxlength="23"
                            :disabled="!canEditCh9329Descriptor"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="ch9329-serial">{{ t('settings.serialNumber') }}</Label>
                          <Input
                            id="ch9329-serial"
                            v-model="ch9329SerialNumber"
                            :placeholder="t('settings.serialNumberAuto')"
                            maxlength="23"
                            :disabled="!canEditCh9329Descriptor"
                          />
                        </div>
                        <p
                          v-if="
                            !ch9329DescriptorLoading &&
                            !ch9329DescriptorLoaded &&
                            !ch9329DescriptorError
                          "
                          class="text-xs text-muted-foreground"
                        >
                          {{ t('settings.ch9329DescriptorReadRequired') }}
                        </p>
                        <p v-if="!isCh9329DescriptorValid" class="text-xs text-warning">
                          {{ t('settings.ch9329StringLengthWarning') }}
                        </p>
                        <p class="text-sm text-warning">
                          {{ t('settings.ch9329DescriptorWarning') }}
                        </p>
                      </div> </template
                    ><template v-if="supportsOtgFunctions(config.hid_backend)"
                      ><div class="settings-field-list space-y-4">
                        <div>
                          <h4 class="text-sm font-medium">{{ t('settings.otgDescriptor') }}</h4>
                        </div>
                        <div class="settings-field-list grid gap-4 sm:grid-cols-2">
                          <div class="settings-field space-y-2">
                            <Label for="otg-vid">{{ t('settings.vendorId') }}</Label>
                            <Input
                              id="otg-vid"
                              v-model="otgVendorIdHex"
                              placeholder="1d6b"
                              maxlength="4"
                              @input="validateHex($event, 'vid')"
                            />
                          </div>
                          <div class="settings-field space-y-2">
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
                        <div class="settings-field space-y-2">
                          <Label for="otg-manufacturer">{{ t('settings.manufacturer') }}</Label>
                          <Input
                            id="otg-manufacturer"
                            v-model="otgManufacturer"
                            placeholder="One-KVM"
                            maxlength="126"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="otg-product">{{ t('settings.productName') }}</Label>
                          <Input
                            id="otg-product"
                            v-model="otgProduct"
                            placeholder="One-KVM USB Device"
                            maxlength="126"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="otg-serial">{{ t('settings.serialNumber') }}</Label>
                          <Input
                            id="otg-serial"
                            v-model="otgSerialNumber"
                            :placeholder="t('settings.serialNumberAuto')"
                            maxlength="126"
                          />
                        </div>
                        <p class="text-sm text-warning">
                          {{ t('settings.descriptorWarning') }}
                        </p>
                      </div></template
                    >
                    <p
                      v-if="saveError && hidSaveScope === 'identity'"
                      role="alert"
                      class="text-sm text-destructive"
                    >
                      {{ saveError }}
                    </p></CollapsibleContent
                  ></Collapsible
                ></CardContent
              ></Card
            >
          </div>

          <!-- Environment Section -->
          <div v-show="activeSection === 'environment'" class="settings-environment">
            <Card v-if="otgSupported" :class="{ 'settings-wide': otgSelfCheckResult }">
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.otgSelfCheck.desc')">{{
                  t('settings.otgSelfCheck.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="otgSelfCheckLoading"
                    :class="[
                      'transition-all duration-150 active:scale-95 active:brightness-95',
                      otgRunButtonPressed ? 'scale-95 brightness-95' : '',
                    ]"
                    @click="onRunOtgSelfCheckClick"
                  >
                    <RefreshCw
                      class="size-4 mr-2"
                      :class="{ 'animate-spin': otgSelfCheckLoading }"
                    />
                    {{ t('settings.otgSelfCheck.run') }}
                  </Button>
                </div></CardHeader
              >
              <CardContent class="space-y-3">
                <p v-if="otgSelfCheckError" class="text-xs text-destructive">
                  {{ otgSelfCheckError }}
                </p>

                <template v-if="otgSelfCheckResult">
                  <div class="flex flex-wrap gap-2 text-xs">
                    <Badge
                      :variant="otgSelfCheckResult.overall_ok ? 'default' : 'destructive'"
                      class="font-medium"
                    >
                      {{ t('settings.otgSelfCheck.overall') }}：{{
                        otgSelfCheckResult.overall_ok
                          ? t('settings.otgSelfCheck.ok')
                          : t('settings.otgSelfCheck.hasIssues')
                      }}
                    </Badge>
                    <Badge variant="outline" class="font-normal">
                      {{
                        t('settings.otgSelfCheck.counts', {
                          errors: otgSelfCheckResult.error_count,
                          warnings: otgSelfCheckResult.warning_count,
                        })
                      }}
                    </Badge>
                    <Badge variant="secondary" class="font-normal">
                      {{ t('settings.otgSelfCheck.selectedUdc') }}：{{
                        otgSelfCheckResult.selected_udc || '-'
                      }}
                    </Badge>
                    <Badge variant="secondary" class="font-normal">
                      {{ t('settings.otgSelfCheck.boundUdc') }}：{{
                        otgSelfCheckResult.bound_udc || '-'
                      }}
                    </Badge>
                  </div>

                  <div class="rounded-md border divide-y">
                    <details
                      v-for="group in otgCheckGroups"
                      :key="group.id"
                      :open="group.status === 'error' || group.status === 'warn'"
                      class="group"
                    >
                      <summary
                        class="list-none cursor-pointer px-4 py-3 flex items-center justify-between gap-3 hover:bg-muted/40"
                      >
                        <div class="flex items-center gap-2 min-w-0">
                          <span
                            class="inline-block size-2 rounded-full shrink-0"
                            :class="otgGroupStatusClass(group.status)"
                          />
                          <span class="text-sm font-medium truncate leading-6">{{
                            t(group.titleKey)
                          }}</span>
                        </div>
                        <div class="flex items-center gap-2 shrink-0">
                          <span class="text-xs text-muted-foreground">{{
                            otgGroupSummary(group)
                          }}</span>
                          <Badge variant="outline" class="text-[10px] h-5 px-1.5">{{
                            otgGroupStatusText(group.status)
                          }}</Badge>
                        </div>
                      </summary>

                      <div v-if="group.items.length > 0" class="border-t bg-muted/20">
                        <div
                          v-for="item in group.items"
                          :key="item.id"
                          class="px-4 py-3 border-b last:border-b-0"
                        >
                          <div class="flex items-start gap-2">
                            <span
                              class="inline-block size-2 rounded-full mt-1.5 shrink-0"
                              :class="otgCheckLevelClass(item.level)"
                            />
                            <div class="min-w-0 space-y-1">
                              <div class="flex items-center gap-2">
                                <p class="text-sm leading-5">{{ otgCheckMessage(item) }}</p>
                                <span class="text-[11px] text-muted-foreground shrink-0">{{
                                  otgCheckStatusText(item.level)
                                }}</span>
                              </div>
                              <div
                                class="flex flex-wrap gap-x-2 gap-y-1 text-[11px] text-muted-foreground"
                              >
                                <span v-if="item.hint">{{ otgCheckHint(item) }}</span>
                                <code v-if="item.path" class="font-mono break-all">{{
                                  item.path
                                }}</code>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                      <div
                        v-else
                        class="border-t bg-muted/20 px-4 py-3 text-xs text-muted-foreground"
                      >
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

            <Card :class="{ 'settings-wide': videoEncoderSelfCheckResult }">
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.encoderSelfCheck.desc')">{{
                  t('settings.encoderSelfCheck.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="videoEncoderSelfCheckLoading"
                    :class="[
                      'transition-all duration-150 active:scale-95 active:brightness-95',
                      videoEncoderRunButtonPressed ? 'scale-95 brightness-95' : '',
                    ]"
                    @click="onRunVideoEncoderSelfCheckClick"
                  >
                    <RefreshCw
                      class="size-4 mr-2"
                      :class="{ 'animate-spin': videoEncoderSelfCheckLoading }"
                    />
                    {{ t('settings.encoderSelfCheck.run') }}
                  </Button>
                </div></CardHeader
              >
              <CardContent class="space-y-3">
                <p v-if="videoEncoderSelfCheckError" class="text-xs text-destructive">
                  {{ videoEncoderSelfCheckError }}
                </p>

                <template v-if="videoEncoderSelfCheckResult">
                  <div class="text-sm">
                    {{ t('settings.encoderSelfCheck.currentHardwareEncoder') }}：{{
                      currentHardwareEncoderText
                    }}
                  </div>

                  <div class="rounded-md border bg-card">
                    <Table class="table-fixed">
                      <TableHeader>
                        <TableRow>
                          <TableHead class="w-[18%] px-2">{{
                            t('settings.encoderSelfCheck.resolution')
                          }}</TableHead>
                          <TableHead
                            v-for="codec in videoEncoderSelfCheckResult.codecs"
                            :key="codec.id"
                            class="w-[20.5%] px-2 text-center"
                          >
                            {{ videoEncoderCodecLabel(codec.id, codec.name) }}
                          </TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        <TableRow
                          v-for="row in videoEncoderSelfCheckResult.rows"
                          :key="row.resolution_id"
                        >
                          <TableCell class="px-2 py-3 align-middle">
                            <div class="font-medium">{{ row.resolution_label }}</div>
                          </TableCell>
                          <TableCell
                            v-for="codec in videoEncoderSelfCheckResult.codecs"
                            :key="`${row.resolution_id}-${codec.id}`"
                            class="px-2 py-3 align-middle"
                          >
                            <div
                              class="flex flex-col items-center justify-center gap-1"
                              :class="videoEncoderCellClass(videoEncoderCell(row, codec.id)?.ok)"
                            >
                              <div class="text-lg leading-none font-semibold">
                                {{ videoEncoderCellSymbol(videoEncoderCell(row, codec.id)?.ok) }}
                              </div>
                              <div class="text-[11px] leading-4 text-foreground/70">
                                {{ videoEncoderCellTime(videoEncoderCell(row, codec.id)) }}
                              </div>
                            </div>
                          </TableCell>
                        </TableRow>
                      </TableBody>
                    </Table>
                  </div>
                </template>
                <p v-else-if="videoEncoderSelfCheckLoading" class="text-xs text-muted-foreground">
                  {{ t('common.loading') }}
                </p>
              </CardContent>
            </Card>
          </div>

          <div v-show="activeSection === 'other'" class="space-y-4">
            <SystemDeviceControls />
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.watchdog.description')">{{
                  t('settings.watchdog.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    :disabled="
                      watchdogLoading ||
                      !watchdogStatus ||
                      (!watchdogStatus.supported && !watchdogStatus.enabled)
                    "
                    @click="updateWatchdog(watchdogDraft)"
                    ><Loader2 v-if="watchdogLoading" class="size-4 animate-spin" /><Save
                      v-else
                      class="size-4"
                    />{{ t('common.save') }}</Button
                  >
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-toggle">
                  <Label for="watchdog-enabled">{{ t('settingsUi.watchdogAutoReset') }}</Label
                  ><Switch
                    id="watchdog-enabled"
                    v-model="watchdogDraft"
                    :disabled="
                      watchdogLoading ||
                      !watchdogStatus ||
                      (!watchdogStatus.supported && !watchdogStatus.enabled)
                    "
                  />
                </div>
                <p
                  v-if="watchdogError || watchdogDisplayReason"
                  class="text-sm"
                  :class="
                    watchdogStatusKey === 'error' || watchdogError
                      ? 'text-destructive'
                      : 'text-muted-foreground'
                  "
                >
                  {{ watchdogError || watchdogDisplayReason }}
                </p>
              </CardContent>
            </Card>

            <Card v-if="usbResetSupported">
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.usbDevices.desc')">{{
                  t('settings.usbDevices.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="usbDevicesLoading"
                    @click="fetchUsbDevices()"
                  >
                    <RefreshCw class="size-4 mr-2" :class="{ 'animate-spin': usbDevicesLoading }" />
                    {{ t('settings.usbDevices.refresh') }}
                  </Button>
                </div></CardHeader
              >
              <CardContent class="space-y-3">
                <p v-if="usbDevicesError" class="text-xs text-destructive">
                  {{ usbDevicesError }}
                </p>

                <template v-if="usbDevices.length > 0">
                  <div class="rounded-md border overflow-hidden">
                    <Table class="min-w-[540px]">
                      <TableHeader>
                        <TableRow class="bg-muted/40 hover:bg-muted/40">
                          <TableHead>{{ t('settings.usbDevices.colDevice') }}</TableHead>
                          <TableHead>VID:PID</TableHead>
                          <TableHead>{{ t('settings.usbDevices.colSpeed') }}</TableHead>
                          <TableHead>{{ t('settings.usbDevices.colVideo') }}</TableHead>
                          <TableHead class="text-right">{{
                            t('settings.usbDevices.colAction')
                          }}</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        <TableRow v-for="dev in usbDevices" :key="`${dev.bus_num}-${dev.dev_num}`">
                          <TableCell>
                            <div
                              class="font-medium truncate max-w-[180px]"
                              :title="
                                dev.product ||
                                dev.manufacturer ||
                                `${dev.id_vendor}:${dev.id_product}`
                              "
                            >
                              {{
                                dev.product ||
                                dev.manufacturer ||
                                `${dev.id_vendor}:${dev.id_product}`
                              }}
                            </div>
                          </TableCell>
                          <TableCell class="font-mono text-xs"
                            >{{ dev.id_vendor }}:{{ dev.id_product }}</TableCell
                          >
                          <TableCell class="text-xs">{{ usbSpeedLabel(dev.speed) }}</TableCell>
                          <TableCell class="text-xs">
                            <code v-if="dev.video_device">{{ dev.video_device }}</code>
                            <span v-else class="text-muted-foreground">-</span>
                          </TableCell>
                          <TableCell class="text-right">
                            <Button
                              v-if="dev.authorized != null"
                              variant="outline"
                              size="sm"
                              class="text-xs"
                              :disabled="usbResetLoading"
                              @click="usbResetTarget = dev"
                            >
                              {{ t('settings.usbDevices.reset') }}
                            </Button>
                          </TableCell>
                        </TableRow>
                      </TableBody>
                    </Table>
                  </div>
                </template>
                <Skeleton v-else-if="usbDevicesLoading" class="h-24 w-full" />
                <Empty v-else class="py-6">
                  <EmptyHeader>
                    <EmptyMedia variant="icon"><Monitor /></EmptyMedia>
                    <EmptyDescription>{{ t('settings.usbDevices.noDevices') }}</EmptyDescription>
                  </EmptyHeader>
                </Empty>
              </CardContent>
            </Card>

            <!-- USB Reset Confirmation Dialog -->
            <AlertDialog v-if="usbResetSupported" :open="usbResetTarget != null">
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>{{
                    t('settings.usbDevices.resetConfirmTitle')
                  }}</AlertDialogTitle>
                  <AlertDialogDescription>
                    {{
                      t('settings.usbDevices.resetConfirmDesc', {
                        device:
                          usbResetTarget?.product ||
                          `${usbResetTarget?.id_vendor}:${usbResetTarget?.id_product}`,
                      })
                    }}
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel @click="usbResetTarget = null">{{
                    t('common.cancel')
                  }}</AlertDialogCancel>
                  <AlertDialogAction
                    :disabled="usbResetLoading"
                    class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                    @click="confirmUsbReset()"
                  >
                    {{ t('settings.usbDevices.resetAction') }}
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </div>
          <div v-show="activeSection === 'network'" class="space-y-4">
            <!-- Auto-restart: restarting progress -->
            <div
              v-if="autoRestarting"
              class="flex items-center gap-3 rounded-lg border bg-card px-4 py-3 text-sm shadow-sm"
            >
              <RefreshCw class="size-4 animate-spin text-primary shrink-0" />
              <div class="flex-1 min-w-0">
                <p class="font-medium">{{ t('settings.autoRestarting') }}</p>
                <p class="text-xs text-muted-foreground">
                  {{
                    webServerConfig.https_enabled
                      ? t('settings.autoRestartingHttpsDesc', { sec: autoRestartCountdown })
                      : t('settings.autoRestartingDesc')
                  }}
                </p>
              </div>
              <span
                v-if="webServerConfig.https_enabled && autoRestartCountdown > 0"
                class="tabular-nums text-lg font-bold text-primary shrink-0"
              >
                {{ autoRestartCountdown }}
              </span>
            </div>

            <!-- Auto-restart: HTTPS manual redirect (cert must be accepted by user) -->
            <Alert v-if="autoRestartManualUrl" variant="warning">
              <Lock />
              <AlertTitle>{{ t('settings.httpsManualRedirectTitle') }}</AlertTitle>
              <AlertDescription>{{ t('settings.httpsManualRedirectDesc') }}</AlertDescription>
              <Button
                as-child
                class="col-start-2 mt-2 w-full bg-warning text-warning-foreground hover:bg-warning/90"
              >
                <a :href="autoRestartManualUrl"><ExternalLink />{{ autoRestartManualUrl }}</a>
              </Button>
            </Alert>

            <!-- Auto-restart: failure / timeout -->
            <Alert v-if="autoRestartFailed" variant="destructive">
              <AlertTriangle />
              <AlertDescription class="flex items-center justify-between gap-3">
                <span>{{ t('settings.autoRestartFailed') }}</span>
                <Button
                  variant="outline"
                  size="sm"
                  class="text-foreground"
                  @click="triggerAutoRestart"
                >
                  <RefreshCw class="size-3 mr-1" />
                  {{ t('common.retry') }}
                </Button>
              </AlertDescription>
            </Alert>

            <!-- Port Configuration Card -->
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.portConfigDesc')">{{
                  t('settings.portConfig')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button
                      @click="saveWebServerConfig('ports')"
                      :disabled="webServerLoading || autoRestarting"
                    >
                      <RefreshCw v-if="autoRestarting" class="size-4 mr-2 animate-spin" />
                      <Save v-else class="size-4 mr-2" />
                      {{ autoRestarting ? t('settings.restarting') : t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-5">
                <!-- HTTPS toggle -->
                <div class="settings-toggle flex items-start justify-between gap-4">
                  <Label>{{ t('settings.httpsEnabled') }}</Label>
                  <Switch v-model="webServerConfig.https_enabled" />
                </div>

                <Separator />

                <div class="settings-field-list">
                  <div class="settings-field">
                    <Label for="settings-http-port">{{ t('settings.httpPort') }}</Label
                    ><Input
                      id="settings-http-port"
                      v-model.number="webServerConfig.http_port"
                      type="number"
                      min="1"
                      max="65535"
                    />
                  </div>
                  <div class="settings-field">
                    <Label for="settings-https-port">{{ t('settings.httpsPort') }}</Label
                    ><Input
                      id="settings-https-port"
                      v-model.number="webServerConfig.https_port"
                      type="number"
                      min="1"
                      max="65535"
                    />
                  </div>
                </div>
                <!-- Preview URL -->
                <div class="rounded-md border bg-muted/40 p-3 space-y-1.5">
                  <p class="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    {{ t('settings.previewUrl') }}
                  </p>
                  <div class="flex items-center gap-2">
                    <code class="font-mono text-xs sm:text-sm break-all flex-1 min-w-0">{{
                      previewAccessUrl
                    }}</code>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      class="shrink-0"
                      :title="t('settings.copyUrl')"
                      :aria-label="t('settings.copyUrl')"
                      @click="copyPreviewUrl"
                    >
                      <Check v-if="previewUrlCopied" class="size-3.5 text-success" />
                      <Copy v-else class="size-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      class="shrink-0"
                      :title="t('settings.openInBrowser')"
                      :aria-label="t('settings.openInBrowser')"
                      @click="openPreviewUrl"
                    >
                      <ExternalLink class="size-3.5" />
                    </Button>
                  </div>
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <p class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                  <AlertTriangle class="size-3.5 shrink-0 text-warning" />
                  <span class="truncate">{{ t('settings.restartRequiredHint') }}</span>
                </p>
              </CardFooter>
            </Card>

            <!-- Listen Address Card -->
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.listenAddressDesc')">{{
                  t('settings.listenAddress')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button
                      @click="saveWebServerConfig('listen')"
                      :disabled="webServerLoading || !!bindAddressError || autoRestarting"
                    >
                      <RefreshCw v-if="autoRestarting" class="size-4 mr-2 animate-spin" />
                      <Save v-else class="size-4 mr-2" />
                      {{ autoRestarting ? t('settings.restarting') : t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <RadioGroup v-model="bindMode" class="space-y-3">
                  <!-- All addresses -->
                  <div class="space-y-2">
                    <div class="flex items-start gap-3">
                      <RadioGroupItem value="all" id="bind-all" class="mt-0.5" />
                      <div class="flex-1">
                        <Label for="bind-all" class="cursor-pointer">{{
                          t('settings.bindModeAll')
                        }}</Label>
                      </div>
                    </div>
                    <div
                      v-if="bindMode === 'all'"
                      class="settings-toggle ml-7 flex items-center justify-between rounded-md border border-dashed px-3 py-2"
                    >
                      <Label class="text-sm font-normal">{{ t('settings.bindIpv6') }}</Label>
                      <Switch v-model="bindAllIpv6" />
                    </div>
                  </div>

                  <Separator />

                  <!-- Loopback only -->
                  <div class="space-y-2">
                    <div class="flex items-start gap-3">
                      <RadioGroupItem value="loopback" id="bind-loopback" class="mt-0.5" />
                      <div class="flex-1">
                        <Label for="bind-loopback" class="cursor-pointer">{{
                          t('settings.bindModeLocal')
                        }}</Label>
                      </div>
                    </div>
                    <div
                      v-if="bindMode === 'loopback'"
                      class="settings-toggle ml-7 flex items-center justify-between rounded-md border border-dashed px-3 py-2"
                    >
                      <Label class="text-sm font-normal">{{ t('settings.bindIpv6') }}</Label>
                      <Switch v-model="bindLocalIpv6" />
                    </div>
                  </div>

                  <Separator />

                  <!-- Custom addresses -->
                  <div class="space-y-2">
                    <div class="flex items-start gap-3">
                      <RadioGroupItem value="custom" id="bind-custom" class="mt-0.5" />
                      <div class="flex-1">
                        <Label for="bind-custom" class="cursor-pointer">{{
                          t('settings.bindModeCustom')
                        }}</Label>
                      </div>
                    </div>
                    <div v-if="bindMode === 'custom'" class="ml-7 space-y-2">
                      <div v-for="(_, i) in bindAddressList" :key="`bind-${i}`" class="flex gap-2">
                        <Input v-model="bindAddressList[i]" placeholder="192.168.1.10" />
                        <Button
                          variant="ghost"
                          size="icon"
                          :aria-label="t('common.delete')"
                          @click="removeBindAddress(i)"
                        >
                          <Trash2 class="size-4" />
                        </Button>
                      </div>
                      <Button variant="outline" size="sm" @click="addBindAddress">
                        <Plus class="size-4 mr-1" />
                        {{ t('settings.addBindAddress') }}
                      </Button>
                      <p v-if="bindAddressError" class="text-xs text-destructive">
                        {{ bindAddressError }}
                      </p>
                    </div>
                  </div>
                </RadioGroup>

                <!-- Effective addresses preview -->
                <div
                  v-if="effectiveBindAddresses.length > 0"
                  class="rounded-md border bg-muted/40 p-3 space-y-1.5"
                >
                  <p class="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    {{ t('settings.effectiveAddresses') }}
                  </p>
                  <code class="font-mono text-xs sm:text-sm break-all block">{{
                    effectiveBindAddresses.join(', ')
                  }}</code>
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <p class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                  <AlertTriangle class="size-3.5 shrink-0 text-warning" />
                  <span class="truncate">{{ t('settings.restartRequiredHint') }}</span>
                </p>
              </CardFooter>
            </Card>

            <!-- SSL Certificate Card -->
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.sslCertificateDesc')">{{
                  t('settings.sslCertificate')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button
                      :disabled="
                        certSaving || autoRestarting || !sslCertPem.trim() || !sslKeyPem.trim()
                      "
                      @click="saveCertificate"
                    >
                      <RefreshCw
                        v-if="certSaving || autoRestarting"
                        class="size-4 mr-2 animate-spin"
                      />
                      <Save v-else class="size-4 mr-2" />
                      {{ autoRestarting ? t('settings.restarting') : t('settings.sslCertSave') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <!-- Active custom cert notice -->
                <Alert v-if="webServerConfig.has_custom_cert" variant="success">
                  <Check />
                  <AlertDescription class="flex items-center justify-between gap-3">
                    <span>{{ t('settings.sslCertActive') }}</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      class="text-destructive hover:text-destructive text-xs"
                      :disabled="certClearing || autoRestarting"
                      @click="clearCertificate"
                    >
                      <RefreshCw
                        v-if="certClearing || autoRestarting"
                        class="size-3 mr-1 animate-spin"
                      />
                      <Trash2 v-else class="size-3 mr-1" />
                      {{ autoRestarting ? t('settings.restarting') : t('settings.sslCertClear') }}
                    </Button>
                  </AlertDescription>
                </Alert>

                <!-- Certificate textarea -->
                <div class="settings-field space-y-2">
                  <Label>{{ t('settings.sslCertPem') }}</Label>
                  <Textarea
                    v-model="sslCertPem"
                    :placeholder="t('settings.sslCertPemPlaceholder')"
                    class="font-mono text-xs min-h-[110px] resize-y"
                    spellcheck="false"
                    autocomplete="off"
                  />
                </div>

                <!-- Key textarea -->
                <div class="settings-field space-y-2">
                  <Label>{{ t('settings.sslKeyPem') }}</Label>
                  <Textarea
                    v-model="sslKeyPem"
                    :placeholder="t('settings.sslKeyPemPlaceholder')"
                    class="font-mono text-xs min-h-[110px] resize-y"
                    spellcheck="false"
                    autocomplete="off"
                  />
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <p class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                  <AlertTriangle class="size-3.5 shrink-0 text-warning" />
                  <span class="truncate">{{ t('settings.restartRequiredHint') }}</span>
                </p>
              </CardFooter>
            </Card>
          </div>

          <!-- ATX Section -->
          <div v-show="activeSection === 'atx'" class="space-y-4">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settingsUi.featuresHelp')">{{
                  t('settings.atxPowerManagement')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    class="shrink-0"
                    :aria-label="t('common.refresh')"
                    @click="loadAtxDevices"
                  >
                    <RefreshCw class="size-4" />
                  </Button>
                  <div class="settings-save-action">
                    <Button :disabled="atxSaving" @click="saveAtxSettings">
                      <Loader2 v-if="atxSaving" class="size-4 mr-2 animate-spin" /><Check
                        v-else-if="atxSaved"
                        class="size-4 mr-2"
                      /><Save v-else class="size-4 mr-2" />{{
                        atxSaving
                          ? t('actionbar.applying')
                          : atxSaved
                            ? t('common.success')
                            : t('common.save')
                      }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-field-list flex items-center justify-between gap-3">
                  <div class="settings-field flex min-w-0 flex-1 items-center gap-3">
                    <Label for="atx-driver" class="shrink-0">{{ t('settings.atxDriver') }}</Label>
                    <Select v-model="atxConfig.driver">
                      <SelectTrigger id="atx-driver" class="w-full max-w-xs"
                        ><SelectValue
                      /></SelectTrigger>
                      <SelectContent>
                        <SelectItem
                          v-for="option in atxDriverOptions"
                          :key="option.value"
                          :value="option.value"
                          >{{ option.label }}</SelectItem
                        >
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <div
                  v-if="['usbrelay', 'serial'].includes(atxConfig.driver)"
                  class="settings-field-list grid gap-4 sm:grid-cols-2"
                >
                  <div
                    v-if="['usbrelay', 'serial'].includes(atxConfig.driver)"
                    class="settings-field space-y-2"
                  >
                    <Label for="atx-device">{{ t('settings.atxDevice') }}</Label>
                    <Select
                      :model-value="atxConfig.device"
                      @update:model-value="
                        (value) =>
                          (atxConfig.device = value === EMPTY_SELECT_VALUE ? '' : String(value))
                      "
                    >
                      <SelectTrigger id="atx-device" class="w-full"
                        ><SelectValue :placeholder="t('settings.selectDevice')"
                      /></SelectTrigger>
                      <SelectContent>
                        <SelectItem :value="EMPTY_SELECT_VALUE">{{
                          t('settings.selectDevice')
                        }}</SelectItem>
                        <SelectItem
                          v-for="dev in getAtxDevicesForDriver(atxConfig.driver)"
                          :key="dev"
                          :value="dev"
                          :disabled="
                            atxConfig.driver === 'serial' && isAtxSerialDeviceReserved(dev)
                          "
                        >
                          {{ formatAtxDeviceLabel(atxConfig.driver, dev) }}
                        </SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <div v-if="atxConfig.driver === 'serial'" class="settings-field space-y-2">
                    <Label for="atx-baudrate">{{ t('settings.baudRate') }}</Label>
                    <Select
                      :model-value="atxConfig.baud_rate"
                      @update:model-value="(value) => (atxConfig.baud_rate = Number(value))"
                    >
                      <SelectTrigger id="atx-baudrate" class="w-full"
                        ><SelectValue
                      /></SelectTrigger>
                      <SelectContent>
                        <SelectItem
                          v-for="baud in [9600, 19200, 38400, 57600, 115200]"
                          :key="baud"
                          :value="baud"
                          >{{ baud }}</SelectItem
                        >
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <template v-if="atxConfig.driver === 'gpio'">
                  <Separator />
                  <div class="settings-compare-grid">
                    <div class="settings-gpio-group space-y-4">
                      <Label>{{ t('settings.atxPowerButton') }}</Label>
                      <div class="settings-field-list grid gap-3 sm:grid-cols-3">
                        <div class="settings-field space-y-2">
                          <Label for="power-device">{{ t('settings.atxGpioChip') }}</Label>
                          <Select
                            :model-value="atxConfig.power.device"
                            @update:model-value="
                              (value) =>
                                (atxConfig.power.device =
                                  value === EMPTY_SELECT_VALUE ? '' : String(value))
                            "
                          >
                            <SelectTrigger id="power-device" class="w-full"
                              ><SelectValue :placeholder="t('settings.atxDriverNone')"
                            /></SelectTrigger>
                            <SelectContent>
                              <SelectItem :value="EMPTY_SELECT_VALUE">{{
                                t('settings.atxDriverNone')
                              }}</SelectItem>
                              <SelectItem
                                v-for="dev in atxDevices.gpio_chips"
                                :key="dev"
                                :value="dev"
                                >{{ dev }}</SelectItem
                              >
                            </SelectContent>
                          </Select>
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="power-pin">{{ t('settings.atxPin') }}</Label>
                          <Input
                            id="power-pin"
                            type="number"
                            v-model.number="atxConfig.power.pin"
                            min="0"
                            :disabled="!atxConfig.power.device"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="power-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select
                            v-model="atxConfig.power.active_level"
                            :disabled="!atxConfig.power.device"
                          >
                            <SelectTrigger id="power-active-level" class="w-full"
                              ><SelectValue
                            /></SelectTrigger>
                            <SelectContent
                              ><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem
                              ><SelectItem value="low">{{
                                t('settings.atxLevelLow')
                              }}</SelectItem></SelectContent
                            >
                          </Select>
                        </div>
                      </div>
                    </div>

                    <div class="settings-gpio-group space-y-4">
                      <Label>{{ t('settings.atxResetButton') }}</Label>
                      <div class="settings-field-list grid gap-3 sm:grid-cols-3">
                        <div class="settings-field space-y-2">
                          <Label for="reset-device">{{ t('settings.atxGpioChip') }}</Label>
                          <Select
                            :model-value="atxConfig.reset.device"
                            @update:model-value="
                              (value) =>
                                (atxConfig.reset.device =
                                  value === EMPTY_SELECT_VALUE ? '' : String(value))
                            "
                          >
                            <SelectTrigger id="reset-device" class="w-full"
                              ><SelectValue :placeholder="t('settings.atxDriverNone')"
                            /></SelectTrigger>
                            <SelectContent>
                              <SelectItem :value="EMPTY_SELECT_VALUE">{{
                                t('settings.atxDriverNone')
                              }}</SelectItem>
                              <SelectItem
                                v-for="dev in atxDevices.gpio_chips"
                                :key="dev"
                                :value="dev"
                                >{{ dev }}</SelectItem
                              >
                            </SelectContent>
                          </Select>
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="reset-pin">{{ t('settings.atxPin') }}</Label>
                          <Input
                            id="reset-pin"
                            type="number"
                            v-model.number="atxConfig.reset.pin"
                            min="0"
                            :disabled="!atxConfig.reset.device"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="reset-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select
                            v-model="atxConfig.reset.active_level"
                            :disabled="!atxConfig.reset.device"
                          >
                            <SelectTrigger id="reset-active-level" class="w-full"
                              ><SelectValue
                            /></SelectTrigger>
                            <SelectContent
                              ><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem
                              ><SelectItem value="low">{{
                                t('settings.atxLevelLow')
                              }}</SelectItem></SelectContent
                            >
                          </Select>
                        </div>
                      </div>
                    </div>

                    <div class="settings-gpio-group space-y-4">
                      <Label>{{ t('settings.atxLedSensing') }}</Label>
                      <div class="settings-field-list grid gap-3 sm:grid-cols-3">
                        <div class="settings-field space-y-2">
                          <Label for="led-device">{{ t('settings.atxGpioChip') }}</Label>
                          <Select
                            :model-value="atxConfig.led.device"
                            @update:model-value="
                              (value) =>
                                (atxConfig.led.device =
                                  value === EMPTY_SELECT_VALUE ? '' : String(value))
                            "
                          >
                            <SelectTrigger id="led-device" class="w-full"
                              ><SelectValue :placeholder="t('settings.atxDriverNone')"
                            /></SelectTrigger>
                            <SelectContent>
                              <SelectItem :value="EMPTY_SELECT_VALUE">{{
                                t('settings.atxDriverNone')
                              }}</SelectItem>
                              <SelectItem
                                v-for="dev in atxDevices.gpio_chips"
                                :key="dev"
                                :value="dev"
                                >{{ dev }}</SelectItem
                              >
                            </SelectContent>
                          </Select>
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="led-pin">{{ t('settings.atxPin') }}</Label>
                          <Input
                            id="led-pin"
                            type="number"
                            v-model.number="atxConfig.led.pin"
                            min="0"
                            :disabled="!atxConfig.led.device"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="led-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select
                            v-model="atxConfig.led.active_level"
                            :disabled="!atxConfig.led.device"
                          >
                            <SelectTrigger id="led-active-level" class="w-full"
                              ><SelectValue
                            /></SelectTrigger>
                            <SelectContent
                              ><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem
                              ><SelectItem value="low">{{
                                t('settings.atxLevelLow')
                              }}</SelectItem></SelectContent
                            >
                          </Select>
                        </div>
                      </div>
                    </div>

                    <div class="settings-gpio-group space-y-4">
                      <Label>{{ t('settings.atxHddSensing') }}</Label>
                      <div class="settings-field-list grid gap-3 sm:grid-cols-3">
                        <div class="settings-field space-y-2">
                          <Label for="hdd-device">{{ t('settings.atxGpioChip') }}</Label>
                          <Select
                            :model-value="atxConfig.hdd.device"
                            @update:model-value="
                              (value) =>
                                (atxConfig.hdd.device =
                                  value === EMPTY_SELECT_VALUE ? '' : String(value))
                            "
                          >
                            <SelectTrigger id="hdd-device" class="w-full"
                              ><SelectValue :placeholder="t('settings.atxDriverNone')"
                            /></SelectTrigger>
                            <SelectContent>
                              <SelectItem :value="EMPTY_SELECT_VALUE">{{
                                t('settings.atxDriverNone')
                              }}</SelectItem>
                              <SelectItem
                                v-for="dev in atxDevices.gpio_chips"
                                :key="dev"
                                :value="dev"
                                >{{ dev }}</SelectItem
                              >
                            </SelectContent>
                          </Select>
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="hdd-pin">{{ t('settings.atxPin') }}</Label>
                          <Input
                            id="hdd-pin"
                            type="number"
                            v-model.number="atxConfig.hdd.pin"
                            min="0"
                            :disabled="!atxConfig.hdd.device"
                          />
                        </div>
                        <div class="settings-field space-y-2">
                          <Label for="hdd-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select
                            v-model="atxConfig.hdd.active_level"
                            :disabled="!atxConfig.hdd.device"
                          >
                            <SelectTrigger id="hdd-active-level" class="w-full"
                              ><SelectValue
                            /></SelectTrigger>
                            <SelectContent
                              ><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem
                              ><SelectItem value="low">{{
                                t('settings.atxLevelLow')
                              }}</SelectItem></SelectContent
                            >
                          </Select>
                        </div>
                      </div>
                    </div>
                  </div>
                </template>

                <template v-else-if="['usbrelay', 'serial'].includes(atxConfig.driver)">
                  <Separator />
                  <div class="settings-compare-grid">
                    <div class="settings-gpio-group space-y-4">
                      <Label>{{ t('settings.atxPowerButton') }}</Label>
                      <div class="settings-field space-y-2">
                        <Label for="power-channel">{{ t('settings.atxChannel') }}</Label>
                        <Input
                          id="power-channel"
                          type="number"
                          v-model.number="atxConfig.power.pin"
                          min="1"
                        />
                      </div>
                    </div>

                    <div class="settings-gpio-group space-y-4">
                      <Label>{{ t('settings.atxResetButton') }}</Label>
                      <div class="settings-field space-y-2">
                        <Label for="reset-channel">{{ t('settings.atxChannel') }}</Label>
                        <Input
                          id="reset-channel"
                          type="number"
                          v-model.number="atxConfig.reset.pin"
                          min="1"
                        />
                      </div>
                    </div>
                  </div>
                </template>
              </CardContent>
            </Card>

            <!-- WOL Config -->
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.atxWolSettingsDesc')">{{
                  t('settings.atxWolSettings')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button :disabled="wolSaving" @click="saveWolSettings">
                      <Loader2 v-if="wolSaving" class="size-4 mr-2 animate-spin" /><Check
                        v-else-if="wolSaved"
                        class="size-4 mr-2"
                      /><Save v-else class="size-4 mr-2" />{{
                        wolSaving
                          ? t('actionbar.applying')
                          : wolSaved
                            ? t('common.success')
                            : t('common.save')
                      }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-field space-y-2">
                  <Label for="wol-interface">{{ t('settings.atxWolInterface') }}</Label>
                  <Input
                    id="wol-interface"
                    v-model="atxConfig.wol_interface"
                    :placeholder="t('settings.atxWolInterfacePlaceholder')"
                  />
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- ttyd Section -->
          <div v-show="activeSection === 'ext-ttyd'" class="space-y-4">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('extensions.ttyd.desc')">{{
                  t('extensions.ttyd.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-service-actions" v-if="extensions?.ttyd?.available">
                    <Button
                      variant="outline"
                      :aria-label="t('extensions.start') + ' ' + t('extensions.ttyd.title')"
                      :disabled="
                        extensionsLoading ||
                        isExtRunning(extensions?.ttyd?.status) ||
                        extensionBusy.ttyd
                      "
                      size="sm"
                      @click="startExtension('ttyd')"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      :aria-label="t('extensions.stop') + ' ' + t('extensions.ttyd.title')"
                      :disabled="
                        extensionsLoading ||
                        !isExtRunning(extensions?.ttyd?.status) ||
                        extensionBusy.ttyd
                      "
                      size="sm"
                      variant="outline"
                      @click="stopExtension('ttyd')"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                  <div class="settings-save-action" v-if="extensions?.ttyd?.available">
                    <Button
                      :disabled="
                        loading || isExtRunning(extensions?.ttyd?.status) || extensionBusy.ttyd
                      "
                      @click="saveExtensionConfig('ttyd')"
                    >
                      <Loader2 v-if="extensionBusy.ttyd" class="size-4 mr-2 animate-spin" /><Save v-else class="size-4 mr-2" />{{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div
                  v-if="!extensions?.ttyd?.available"
                  class="text-sm text-muted-foreground bg-muted p-3 rounded-md"
                >
                  {{
                    t('extensions.binaryNotFound', {
                      path: isWindows ? 'ttyd.win32.exe' : '/usr/bin/ttyd',
                    })
                  }}
                </div>
                <template v-else>
                  <!-- Status and controls -->
                  <div v-if="isExtRunning(extensions?.ttyd?.status)" class="settings-inline-actions">
                    <Button
                      v-if="isExtRunning(extensions?.ttyd?.status)"
                      size="sm"
                      variant="default"
                      @click="openTerminal"
                    >
                      <Terminal class="size-4 mr-1" />
                      {{ t('extensions.ttyd.open') }}
                    </Button>
                  </div>
                  <!-- Config -->
                  <div class="settings-field-list grid gap-4">
                    <div class="settings-toggle flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch
                        v-model="extConfig.ttyd.enabled"
                        :disabled="isExtRunning(extensions?.ttyd?.status)"
                      />
                    </div>
                    <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.ttyd.shell') }}</Label>
                      <Input
                        v-model="extConfig.ttyd.shell"
                        class="sm:col-span-3"
                        :placeholder="isWindows ? 'cmd' : '/bin/bash'"
                        :disabled="isExtRunning(extensions?.ttyd?.status)"
                      />
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <Collapsible
                      v-model:open="showLogs.ttyd"
                      @update:open="(open) => open && refreshExtensionLogs('ttyd')"
                    >
                      <CollapsibleTrigger as-child
                        ><Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          class="gap-2 text-muted-foreground"
                          ><ChevronRight
                            :class="[
                              'size-4 transition-transform',
                              showLogs.ttyd ? 'rotate-90' : '',
                            ]"
                          />{{ t('extensions.viewLogs') }}</Button
                        ></CollapsibleTrigger
                      >
                      <CollapsibleContent class="space-y-2 pt-2">
                        <pre
                          class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono"
                          >{{
                            (extensionLogs.ttyd || []).join('\n') || t('extensions.noLogs')
                          }}</pre
                        >
                        <Button variant="ghost" size="sm" @click="refreshExtensionLogs('ttyd')">
                          <RefreshCw class="size-3 mr-1" />
                          {{ t('common.refresh') }}
                        </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
            </Card>
          </div>

          <!-- Remote Access Section -->
          <div v-show="activeSection === 'ext-remote-access'" class="settings-section-services">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('extensions.easyfrp.desc')">{{
                  t('extensions.easyfrp.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-service-actions" v-if="extensions?.easyfrp?.available">
                    <Button
                      variant="outline"
                      :aria-label="t('extensions.start') + ' ' + t('extensions.easyfrp.title')"
                      :disabled="
                        extensionsLoading ||
                        !!easyfrpValidationMessage ||
                        isExtRunning(extensions?.easyfrp?.status) ||
                        extensionBusy.easyfrp
                      "
                      size="sm"
                      @click="startExtension('easyfrp')"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      :aria-label="t('extensions.stop') + ' ' + t('extensions.easyfrp.title')"
                      :disabled="
                        extensionsLoading ||
                        !isExtRunning(extensions?.easyfrp?.status) ||
                        extensionBusy.easyfrp
                      "
                      size="sm"
                      variant="outline"
                      @click="stopExtension('easyfrp')"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                  <div class="settings-save-action" v-if="extensions?.easyfrp?.available">
                    <Button
                      :disabled="
                        loading ||
                        isExtRunning(extensions?.easyfrp?.status) ||
                        extensionBusy.easyfrp
                      "
                      @click="saveExtensionConfig('easyfrp')"
                    >
                      <Loader2 v-if="extensionBusy.easyfrp" class="size-4 mr-2 animate-spin" /><Save v-else class="size-4 mr-2" />{{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div
                  v-if="!extensions?.easyfrp?.available"
                  class="text-sm text-muted-foreground bg-muted p-3 rounded-md"
                >
                  {{
                    t('extensions.binaryNotFound', {
                      path: isWindows ? 'easyfrpc.exe' : '/usr/bin/easyfrpc',
                    })
                  }}
                </div>
                <template v-else>
                  <!-- Status and controls -->

                  <!-- Config -->
                  <div class="settings-field-list grid gap-4">
                    <div class="settings-toggle flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch
                        v-model="extConfig.easyfrp.enabled"
                        :disabled="isExtRunning(extensions?.easyfrp?.status)"
                      />
                    </div>
                    <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easyfrp.urls') }}</Label>
                      <div class="sm:col-span-3 space-y-1">
                        <Textarea
                          v-model="easyfrpUrlsText"
                          :rows="3"
                          autocomplete="off"
                          :spellcheck="false"
                          :placeholder="t('extensions.easyfrp.urlsPlaceholder')"
                          :disabled="isExtRunning(extensions?.easyfrp?.status)"
                        />
                        <p class="text-xs text-muted-foreground">
                          {{ t('extensions.easyfrp.urlsHint') }}
                        </p>
                        <p
                          v-if="extConfig.easyfrp.enabled && easyfrpValidationMessage"
                          class="text-xs text-destructive"
                        >
                          {{ easyfrpValidationMessage }}
                        </p>
                      </div>
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <Collapsible
                      v-model:open="showLogs.easyfrp"
                      @update:open="(open) => open && refreshExtensionLogs('easyfrp')"
                    >
                      <CollapsibleTrigger as-child
                        ><Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          class="gap-2 text-muted-foreground"
                          ><ChevronRight
                            :class="[
                              'size-4 transition-transform',
                              showLogs.easyfrp ? 'rotate-90' : '',
                            ]"
                          />{{ t('extensions.viewLogs') }}</Button
                        ></CollapsibleTrigger
                      >
                      <CollapsibleContent class="space-y-2 pt-2">
                        <pre
                          class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono"
                          >{{
                            (extensionLogs.easyfrp || []).join('\n') || t('extensions.noLogs')
                          }}</pre
                        >
                        <Button variant="ghost" size="sm" @click="refreshExtensionLogs('easyfrp')">
                          <RefreshCw class="size-3 mr-1" />
                          {{ t('common.refresh') }}
                        </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
            </Card>

            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('extensions.easytier.desc')">{{
                  t('extensions.easytier.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-service-actions" v-if="extensions?.easytier?.available">
                    <Button
                      variant="outline"
                      :aria-label="t('extensions.start') + ' ' + t('extensions.easytier.title')"
                      :disabled="
                        extensionsLoading ||
                        !!easytierValidationMessage ||
                        isExtRunning(extensions?.easytier?.status) ||
                        extensionBusy.easytier
                      "
                      size="sm"
                      @click="startExtension('easytier')"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      :aria-label="t('extensions.stop') + ' ' + t('extensions.easytier.title')"
                      :disabled="
                        extensionsLoading ||
                        !isExtRunning(extensions?.easytier?.status) ||
                        extensionBusy.easytier
                      "
                      size="sm"
                      variant="outline"
                      @click="stopExtension('easytier')"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                  <div class="settings-save-action" v-if="extensions?.easytier?.available">
                    <Button
                      :disabled="
                        loading ||
                        isExtRunning(extensions?.easytier?.status) ||
                        extensionBusy.easytier
                      "
                      @click="saveExtensionConfig('easytier')"
                    >
                      <Loader2 v-if="extensionBusy.easytier" class="size-4 mr-2 animate-spin" /><Save v-else class="size-4 mr-2" />{{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div
                  v-if="!extensions?.easytier?.available"
                  class="text-sm text-muted-foreground bg-muted p-3 rounded-md"
                >
                  {{
                    t('extensions.binaryNotFound', {
                      path: isWindows ? 'easytier-core.exe' : '/usr/bin/easytier-core',
                    })
                  }}
                </div>
                <template v-else>
                  <!-- Status and controls -->

                  <!-- Config -->
                  <div class="grid gap-4">
                    <div class="settings-toggle flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch
                        v-model="extConfig.easytier.enabled"
                        :disabled="isExtRunning(extensions?.easytier?.status)"
                      />
                    </div>
                    <ExtensionConfigModeEditor
                      v-model:mode="extConfig.easytier.config_mode"
                      v-model:config="extConfig.easytier.custom_toml"
                      :disabled="isExtRunning(extensions?.easytier?.status)"
                      :quick-label="t('extensions.easytier.quickConfig')"
                      :full-label="t('extensions.easytier.fullConfig')"
                      :full-config-hint="t('extensions.easytier.fullConfigHint')"
                      :full-config-required="t('extensions.easytier.fullConfigRequired')"
                    >
                      <template #quick>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{
                            t('extensions.easytier.networkName')
                          }}</Label>
                          <div class="sm:col-span-3 space-y-1">
                            <Input
                              v-model="extConfig.easytier.network_name"
                              :disabled="isExtRunning(extensions?.easytier?.status)"
                            />
                            <p
                              v-if="
                                extConfig.easytier.enabled &&
                                !extConfig.easytier.network_name?.trim()
                              "
                              class="text-xs text-destructive"
                            >
                              {{ t('extensions.easytier.networkNameRequired') }}
                            </p>
                          </div>
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{
                            t('extensions.easytier.networkSecret')
                          }}</Label>
                          <Input
                            v-model="extConfig.easytier.network_secret"
                            type="password"
                            autocomplete="off"
                            class="sm:col-span-3"
                            :disabled="isExtRunning(extensions?.easytier?.status)"
                          />
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.easytier.peers') }}</Label>
                          <div class="sm:col-span-3 space-y-2">
                            <div
                              v-for="(_, i) in extConfig.easytier.peer_urls"
                              :key="i"
                              class="flex gap-2"
                            >
                              <Input
                                v-model="extConfig.easytier.peer_urls[i]"
                                placeholder="tcp://1.2.3.4:11010"
                                :disabled="isExtRunning(extensions?.easytier?.status)"
                              />
                              <Button
                                variant="ghost"
                                size="icon"
                                :aria-label="t('common.delete')"
                                @click="removeEasytierPeer(i)"
                                :disabled="isExtRunning(extensions?.easytier?.status)"
                              >
                                <Trash2 class="size-4" />
                              </Button>
                            </div>
                            <Button
                              variant="outline"
                              size="sm"
                              @click="addEasytierPeer"
                              :disabled="isExtRunning(extensions?.easytier?.status)"
                            >
                              <Plus class="size-4 mr-1" />
                              {{ t('extensions.easytier.addPeer') }}
                            </Button>
                          </div>
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{
                            t('extensions.easytier.virtualIp')
                          }}</Label>
                          <div class="sm:col-span-3 space-y-1">
                            <Input
                              v-model="extConfig.easytier.virtual_ip"
                              placeholder="10.0.0.1/24"
                              :disabled="isExtRunning(extensions?.easytier?.status)"
                            />
                          </div>
                        </div>
                      </template>
                    </ExtensionConfigModeEditor>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <Collapsible
                      v-model:open="showLogs.easytier"
                      @update:open="(open) => open && refreshExtensionLogs('easytier')"
                    >
                      <CollapsibleTrigger as-child
                        ><Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          class="gap-2 text-muted-foreground"
                          ><ChevronRight
                            :class="[
                              'size-4 transition-transform',
                              showLogs.easytier ? 'rotate-90' : '',
                            ]"
                          />{{ t('extensions.viewLogs') }}</Button
                        ></CollapsibleTrigger
                      >
                      <CollapsibleContent class="space-y-2 pt-2">
                        <pre
                          class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono"
                          >{{
                            (extensionLogs.easytier || []).join('\n') || t('extensions.noLogs')
                          }}</pre
                        >
                        <Button variant="ghost" size="sm" @click="refreshExtensionLogs('easytier')">
                          <RefreshCw class="size-3 mr-1" />
                          {{ t('common.refresh') }}
                        </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
            </Card>
            <!-- FRPC -->
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('extensions.frpc.desc')">{{
                  t('extensions.frpc.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-service-actions" v-if="extensions?.frpc?.available">
                    <Button
                      variant="outline"
                      :aria-label="t('extensions.start') + ' ' + t('extensions.frpc.title')"
                      :disabled="
                        extensionsLoading ||
                        !!frpcValidationMessage ||
                        isExtRunning(extensions?.frpc?.status) ||
                        extensionBusy.frpc
                      "
                      size="sm"
                      @click="startExtension('frpc')"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      :aria-label="t('extensions.stop') + ' ' + t('extensions.frpc.title')"
                      :disabled="
                        extensionsLoading ||
                        !isExtRunning(extensions?.frpc?.status) ||
                        extensionBusy.frpc
                      "
                      size="sm"
                      variant="outline"
                      @click="stopExtension('frpc')"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                  <div class="settings-save-action" v-if="extensions?.frpc?.available">
                    <Button
                      :disabled="
                        loading || isExtRunning(extensions?.frpc?.status) || extensionBusy.frpc
                      "
                      @click="saveExtensionConfig('frpc')"
                    >
                      <Loader2 v-if="extensionBusy.frpc" class="size-4 mr-2 animate-spin" /><Save v-else class="size-4 mr-2" />{{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div
                  v-if="!extensions?.frpc?.available"
                  class="text-sm text-muted-foreground bg-muted p-3 rounded-md"
                >
                  {{
                    t('extensions.binaryNotFound', {
                      path: isWindows ? 'frpc.exe' : '/usr/bin/frpc',
                    })
                  }}
                </div>
                <template v-else>
                  <div class="grid gap-4">
                    <div class="settings-toggle flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch
                        v-model="extConfig.frpc.enabled"
                        :disabled="isExtRunning(extensions?.frpc?.status)"
                      />
                    </div>
                    <ExtensionConfigModeEditor
                      v-model:mode="extConfig.frpc.config_mode"
                      v-model:config="extConfig.frpc.custom_toml"
                      :disabled="isExtRunning(extensions?.frpc?.status)"
                      :quick-label="t('extensions.frpc.quickConfig')"
                      :full-label="t('extensions.frpc.fullConfig')"
                      :full-config-hint="t('extensions.frpc.fullConfigHint')"
                      :full-config-required="t('extensions.frpc.fullConfigRequired')"
                    >
                      <template #quick>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.proxyType') }}</Label>
                          <div class="sm:col-span-3">
                            <RadioGroup
                              v-model="extConfig.frpc.proxy_type"
                              class="flex flex-wrap gap-4"
                              :disabled="isExtRunning(extensions?.frpc?.status)"
                            >
                              <div
                                v-for="type in [
                                  'tcp',
                                  'udp',
                                  'http',
                                  'https',
                                  'stcp',
                                  'sudp',
                                  'xtcp',
                                ]"
                                :key="type"
                                class="flex items-center space-x-2"
                              >
                                <RadioGroupItem :value="type" :id="`frpc-${type}`" />
                                <Label :for="`frpc-${type}`" class="cursor-pointer uppercase">{{
                                  type
                                }}</Label>
                              </div>
                            </RadioGroup>
                          </div>
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.proxyName') }}</Label>
                          <div class="sm:col-span-3 space-y-1">
                            <Input
                              v-model="extConfig.frpc.proxy_name"
                              :placeholder="t('extensions.frpc.proxyNamePlaceholder')"
                              :disabled="isExtRunning(extensions?.frpc?.status)"
                            />
                            <p
                              v-if="extConfig.frpc.enabled && !extConfig.frpc.proxy_name?.trim()"
                              class="text-xs text-destructive"
                            >
                              {{ t('extensions.frpc.proxyNameRequired') }}
                            </p>
                          </div>
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.serverAddr') }}</Label>
                          <div class="sm:col-span-3 space-y-1">
                            <Input
                              v-model="extConfig.frpc.server_addr"
                              :placeholder="t('extensions.frpc.serverAddrPlaceholder')"
                              :disabled="isExtRunning(extensions?.frpc?.status)"
                            />
                            <p
                              v-if="extConfig.frpc.enabled && !extConfig.frpc.server_addr?.trim()"
                              class="text-xs text-destructive"
                            >
                              {{ t('extensions.frpc.serverAddrRequired') }}
                            </p>
                          </div>
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.serverPort') }}</Label>
                          <Input
                            v-model.number="extConfig.frpc.server_port"
                            class="sm:col-span-3"
                            type="number"
                            min="1"
                            max="65535"
                            :disabled="isExtRunning(extensions?.frpc?.status)"
                          />
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.token') }}</Label>
                          <div class="sm:col-span-3 space-y-1">
                            <Input
                              v-model="extConfig.frpc.token"
                              type="password"
                              autocomplete="off"
                              :disabled="isExtRunning(extensions?.frpc?.status)"
                            />
                            <p
                              v-if="extConfig.frpc.enabled && !extConfig.frpc.token"
                              class="text-xs text-destructive"
                            >
                              {{ t('extensions.frpc.tokenRequired') }}
                            </p>
                          </div>
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.localIp') }}</Label>
                          <div class="sm:col-span-3 space-y-1">
                            <Input
                              v-model="extConfig.frpc.local_ip"
                              placeholder="127.0.0.1"
                              :disabled="isExtRunning(extensions?.frpc?.status)"
                            />
                            <p
                              v-if="extConfig.frpc.enabled && !extConfig.frpc.local_ip?.trim()"
                              class="text-xs text-destructive"
                            >
                              {{ t('extensions.frpc.localIpRequired') }}
                            </p>
                          </div>
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.localPort') }}</Label>
                          <Input
                            v-model.number="extConfig.frpc.local_port"
                            class="sm:col-span-3"
                            type="number"
                            min="1"
                            max="65535"
                            :disabled="isExtRunning(extensions?.frpc?.status)"
                          />
                        </div>
                        <div
                          v-if="showFrpcRemotePort"
                          class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                        >
                          <Label class="sm:text-right">{{ t('extensions.frpc.remotePort') }}</Label>
                          <div class="sm:col-span-3 space-y-1">
                            <Input
                              v-model.number="extConfig.frpc.remote_port"
                              type="number"
                              min="1"
                              max="65535"
                              :disabled="isExtRunning(extensions?.frpc?.status)"
                            />
                            <p
                              v-if="
                                extConfig.frpc.enabled &&
                                frpcRemotePortRequired &&
                                !extConfig.frpc.remote_port
                              "
                              class="text-xs text-destructive"
                            >
                              {{ t('extensions.frpc.remotePortRequired') }}
                            </p>
                          </div>
                        </div>
                        <div
                          v-if="showFrpcCustomDomain"
                          class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                        >
                          <Label class="sm:text-right">{{
                            t('extensions.frpc.customDomain')
                          }}</Label>
                          <Input
                            v-model="extConfig.frpc.custom_domain"
                            class="sm:col-span-3"
                            :placeholder="t('extensions.frpc.customDomainPlaceholder')"
                            :disabled="isExtRunning(extensions?.frpc?.status)"
                          />
                        </div>
                        <div
                          v-if="showFrpcSecretKey"
                          class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                        >
                          <Label class="sm:text-right">{{ t('extensions.frpc.secretKey') }}</Label>
                          <Input
                            v-model="extConfig.frpc.secret_key"
                            class="sm:col-span-3"
                            type="password"
                            autocomplete="off"
                            :disabled="isExtRunning(extensions?.frpc?.status)"
                          />
                        </div>
                        <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                          <Label class="sm:text-right">{{ t('extensions.frpc.tls') }}</Label>
                          <div class="settings-toggle sm:col-span-3">
                            <Switch
                              v-model="extConfig.frpc.tls"
                              :disabled="isExtRunning(extensions?.frpc?.status)"
                            />
                          </div>
                        </div>
                      </template>
                    </ExtensionConfigModeEditor>
                  </div>
                  <div class="space-y-2">
                    <Collapsible
                      v-model:open="showLogs.frpc"
                      @update:open="(open) => open && refreshExtensionLogs('frpc')"
                    >
                      <CollapsibleTrigger as-child
                        ><Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          class="gap-2 text-muted-foreground"
                          ><ChevronRight
                            :class="[
                              'size-4 transition-transform',
                              showLogs.frpc ? 'rotate-90' : '',
                            ]"
                          />{{ t('extensions.viewLogs') }}</Button
                        ></CollapsibleTrigger
                      >
                      <CollapsibleContent class="space-y-2 pt-2">
                        <pre
                          class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono"
                          >{{
                            (extensionLogs.frpc || []).join('\n') || t('extensions.noLogs')
                          }}</pre
                        >
                        <Button variant="ghost" size="sm" @click="refreshExtensionLogs('frpc')">
                          <RefreshCw class="size-3 mr-1" />
                          {{ t('common.refresh') }}
                        </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
            </Card>
          </div>

          <!-- RTSP Section -->
          <div v-show="activeSection === 'third-party-access'" class="settings-section-services">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('extensions.rtsp.desc')">{{
                  t('extensions.rtsp.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    :aria-label="t('common.refresh')"
                    @click="loadRtspConfig"
                    :disabled="rtspLoading"
                  >
                    <RefreshCw :class="['size-4', rtspLoading ? 'animate-spin' : '']" />
                  </Button>
                  <div class="settings-service-actions">
                    <Button
                      variant="outline"
                      :aria-label="t('extensions.start') + ' ' + t('extensions.rtsp.title')"
                      :disabled="
                        rtspLoading ||
                        rtspStatus?.service_status === 'running' ||
                        rtspStatus?.service_status === 'starting'
                      "
                      size="sm"
                      @click="startRtsp"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      :aria-label="t('extensions.stop') + ' ' + t('extensions.rtsp.title')"
                      :disabled="
                        rtspLoading ||
                        !(
                          rtspStatus?.service_status === 'running' ||
                          rtspStatus?.service_status === 'starting'
                        )
                      "
                      size="sm"
                      variant="outline"
                      @click="stopRtsp"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                  <div class="settings-save-action">
                    <Button
                      class="shrink-0"
                      :disabled="loading || rtspLoading"
                      @click="saveRtspConfig"
                    >
                      <Loader2 v-if="serviceSaving === 'rtsp'" class="size-4 mr-2 animate-spin" /><Save v-else class="size-4 mr-2" />{{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-field-list grid gap-4">
                  <div class="settings-toggle flex items-center justify-between">
                    <Label>{{ t('extensions.autoStart') }}</Label>
                    <Switch v-model="rtspLocalConfig.enabled" />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.bind') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rtspLocalConfig.bind"
                        placeholder="0.0.0.0 / ::"
                        :disabled="rtspStatus?.service_status === 'running'"
                      />
                    </div>
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.port') }}</Label>
                    <Input
                      v-model.number="rtspLocalConfig.port"
                      class="sm:col-span-3"
                      type="number"
                      min="1"
                      max="65535"
                      :disabled="rtspStatus?.service_status === 'running'"
                    />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.path') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rtspLocalConfig.path"
                        :placeholder="t('extensions.rtsp.pathPlaceholder')"
                        :disabled="rtspStatus?.service_status === 'running'"
                      />
                    </div>
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.codec') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Select
                        v-model="rtspLocalConfig.codec"
                        :disabled="rtspStatus?.service_status === 'running'"
                      >
                        <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                        <SelectContent
                          ><SelectItem value="h264">H.264</SelectItem
                          ><SelectItem value="h265">H.265</SelectItem></SelectContent
                        >
                      </Select>
                    </div>
                  </div>
                  <div class="settings-toggle flex items-center justify-between">
                    <Label>{{ t('extensions.rtsp.allowOneClient') }}</Label>
                    <Switch
                      v-model="rtspLocalConfig.allow_one_client"
                      :disabled="rtspStatus?.service_status === 'running'"
                    />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.username') }}</Label>
                    <Input
                      v-model="rtspLocalConfig.username"
                      class="sm:col-span-3"
                      :placeholder="t('extensions.rtsp.usernamePlaceholder')"
                      :disabled="rtspStatus?.service_status === 'running'"
                    />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.password') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <div class="relative">
                        <Input
                          v-model="rtspLocalConfig.password"
                          :type="showPasswords ? 'text' : 'password'"
                          :placeholder="t('extensions.rtsp.passwordPlaceholder')"
                          :disabled="rtspStatus?.service_status === 'running'"
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
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <div class="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
                  <span class="shrink-0 font-medium">{{ t('extensions.rtsp.urlPreview') }}</span>
                  <code class="truncate font-mono">{{ rtspStreamUrl }}</code>
                </div>
              </CardFooter>
            </Card>
          </div>

          <!-- VNC Section -->
          <div v-show="activeSection === 'third-party-access'" class="settings-section-services">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('extensions.vnc.desc')">{{
                  t('extensions.vnc.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    :aria-label="t('common.refresh')"
                    @click="loadVncConfig"
                    :disabled="vncLoading"
                  >
                    <RefreshCw :class="['size-4', vncLoading ? 'animate-spin' : '']" />
                  </Button>
                  <div class="settings-service-actions">
                    <Button
                      variant="outline"
                      :aria-label="t('extensions.start') + ' ' + t('extensions.vnc.title')"
                      :disabled="
                        vncLoading ||
                        vncStatus?.service_status === 'running' ||
                        vncStatus?.service_status === 'starting'
                      "
                      size="sm"
                      @click="startVnc"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      :aria-label="t('extensions.stop') + ' ' + t('extensions.vnc.title')"
                      :disabled="
                        vncLoading ||
                        !(
                          vncStatus?.service_status === 'running' ||
                          vncStatus?.service_status === 'starting'
                        )
                      "
                      size="sm"
                      variant="outline"
                      @click="stopVnc"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                  <div class="settings-save-action">
                    <Button
                      class="shrink-0"
                      :disabled="loading || vncLoading"
                      @click="saveVncConfig"
                    >
                      <Loader2 v-if="serviceSaving === 'vnc'" class="size-4 mr-2 animate-spin" /><Save v-else class="size-4 mr-2" />{{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <div class="settings-field-list grid gap-4">
                  <div class="settings-toggle flex items-center justify-between">
                    <Label>{{ t('extensions.autoStart') }}</Label>
                    <Switch v-model="vncLocalConfig.enabled" />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.bind') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="vncLocalConfig.bind"
                        placeholder="0.0.0.0 / ::"
                        :disabled="vncStatus?.service_status === 'running'"
                      />
                    </div>
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.port') }}</Label>
                    <Input
                      v-model.number="vncLocalConfig.port"
                      class="sm:col-span-3"
                      type="number"
                      min="1"
                      max="65535"
                      :disabled="vncStatus?.service_status === 'running'"
                    />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.encoding') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Select
                        v-model="vncLocalConfig.encoding"
                        :disabled="vncStatus?.service_status === 'running'"
                      >
                        <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                        <SelectContent
                          ><SelectItem value="tight_jpeg">{{
                            t('extensions.vnc.encodingTightJpeg')
                          }}</SelectItem
                          ><SelectItem value="h264">{{
                            t('extensions.vnc.encodingH264')
                          }}</SelectItem></SelectContent
                        >
                      </Select>
                    </div>
                  </div>
                  <div class="settings-toggle flex items-center justify-between">
                    <Label>{{ t('extensions.vnc.allowOneClient') }}</Label>
                    <Switch
                      v-model="vncLocalConfig.allow_one_client"
                      :disabled="vncStatus?.service_status === 'running'"
                    />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.password') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <div class="relative">
                        <Input
                          v-model="vncLocalConfig.password"
                          :type="showPasswords ? 'text' : 'password'"
                          maxlength="8"
                          autocomplete="off"
                          :placeholder="
                            vncStatus?.config.has_password
                              ? t('extensions.vnc.passwordPlaceholder')
                              : t('extensions.vnc.passwordRequiredPlaceholder')
                          "
                          :disabled="vncStatus?.service_status === 'running'"
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
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <div class="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
                  <span class="shrink-0 font-medium">{{ t('extensions.vnc.urlPreview') }}</span>
                  <code class="truncate font-mono">{{ vncStreamUrl }}</code>
                </div>
              </CardFooter>
            </Card>
          </div>

          <!-- RustDesk Section -->
          <div v-show="activeSection === 'third-party-access'" class="settings-section-services">
            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('extensions.rustdesk.desc')">{{
                  t('extensions.rustdesk.title')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    :aria-label="t('common.refresh')"
                    @click="loadRustdeskConfig"
                    :disabled="rustdeskLoading"
                  >
                    <RefreshCw :class="['size-4', rustdeskLoading ? 'animate-spin' : '']" />
                  </Button>
                  <div class="settings-service-actions">
                    <Button
                      variant="outline"
                      :aria-label="t('extensions.start') + ' ' + t('extensions.rustdesk.title')"
                      :disabled="
                        rustdeskLoading ||
                        rustdeskStatus?.service_status === 'running' ||
                        rustdeskStatus?.service_status === 'starting'
                      "
                      size="sm"
                      @click="startRustdesk"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      :aria-label="t('extensions.stop') + ' ' + t('extensions.rustdesk.title')"
                      :disabled="
                        rustdeskLoading ||
                        !(
                          rustdeskStatus?.service_status === 'running' ||
                          rustdeskStatus?.service_status === 'starting'
                        )
                      "
                      size="sm"
                      variant="outline"
                      @click="stopRustdesk"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                  <div class="settings-save-action">
                    <Button :disabled="loading || rustdeskLoading" @click="saveRustdeskConfig">
                      <Loader2 v-if="serviceSaving === 'rustdesk'" class="size-4 mr-2 animate-spin" /><Save v-else class="size-4 mr-2" />{{ t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-4">
                <!-- Status and controls -->

                <!-- Config -->
                <div class="settings-field-list grid gap-4">
                  <div class="settings-toggle flex items-center justify-between">
                    <Label>{{ t('extensions.autoStart') }}</Label>
                    <Switch v-model="rustdeskLocalConfig.enabled" />
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.mode') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Select
                        v-model="rustdeskLocalConfig.mode"
                        :disabled="rustdeskStatus?.service_status === 'running'"
                      >
                        <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                        <SelectContent>
                          <SelectItem value="id">{{ t('extensions.rustdesk.modeId') }}</SelectItem>
                          <SelectItem value="direct_ip">{{
                            t('extensions.rustdesk.modeDirectIp')
                          }}</SelectItem>
                        </SelectContent>
                      </Select>
                      <p class="text-xs text-muted-foreground">
                        {{
                          rustdeskLocalConfig.mode === 'id'
                            ? t('extensions.rustdesk.modeIdDesc')
                            : t('extensions.rustdesk.modeDirectIpDesc')
                        }}
                      </p>
                    </div>
                  </div>
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.codec') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Select
                        v-model="rustdeskLocalConfig.codec"
                        :disabled="rustdeskStatus?.service_status === 'running'"
                      >
                        <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                        <SelectContent
                          ><SelectItem value="h264">H.264</SelectItem
                          ><SelectItem value="h265">H.265</SelectItem></SelectContent
                        >
                      </Select>
                    </div>
                  </div>
                  <div
                    v-if="rustdeskLocalConfig.mode === 'id'"
                    class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                  >
                    <Label class="sm:text-right">{{
                      t('extensions.rustdesk.rendezvousServer')
                    }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rustdeskLocalConfig.rendezvous_server"
                        :placeholder="t('extensions.rustdesk.rendezvousServerPlaceholder')"
                        :disabled="rustdeskStatus?.service_status === 'running'"
                      />
                      <p
                        v-if="rustdeskLocalConfig.enabled && rustdeskValidationMessage"
                        class="text-xs text-destructive"
                      >
                        {{ rustdeskValidationMessage }}
                      </p>
                    </div>
                  </div>
                  <div
                    v-if="rustdeskLocalConfig.mode === 'id'"
                    class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                  >
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.relayServer') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rustdeskLocalConfig.relay_server"
                        :placeholder="t('extensions.rustdesk.relayServerPlaceholder')"
                        :disabled="
                          !rustdeskLocalConfig.rendezvous_server ||
                          rustdeskStatus?.service_status === 'running'
                        "
                      />
                    </div>
                  </div>
                  <div
                    v-if="rustdeskLocalConfig.mode === 'id'"
                    class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                  >
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.relayKey') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <div class="relative">
                        <Input
                          v-model="rustdeskLocalConfig.relay_key"
                          :type="showPasswords ? 'text' : 'password'"
                          :disabled="
                            !rustdeskLocalConfig.rendezvous_server ||
                            rustdeskStatus?.service_status === 'running'
                          "
                          maxlength="44"
                          autocomplete="off"
                          spellcheck="false"
                          class="font-mono"
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
                  <div
                    v-if="rustdeskLocalConfig.mode === 'direct_ip'"
                    class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                  >
                    <Label class="sm:text-right">{{
                      t('extensions.rustdesk.directAccessPort')
                    }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model.number="rustdeskLocalConfig.direct_access_port"
                        type="number"
                        min="1"
                        max="65535"
                        :disabled="rustdeskStatus?.service_status === 'running'"
                      />
                      <p v-if="rustdeskValidationMessage" class="text-xs text-destructive">
                        {{ rustdeskValidationMessage }}
                      </p>
                    </div>
                  </div>
                </div>
                <Separator />

                <!-- Device Info -->
                <div class="settings-field-list space-y-3">
                  <!-- Device ID -->
                  <div
                    v-if="rustdeskLocalConfig.mode === 'id'"
                    class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center"
                  >
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.deviceId') }}</Label>
                    <div class="sm:col-span-3 flex items-center gap-2">
                      <code class="font-mono text-lg bg-muted px-3 py-1 rounded">{{
                        rustdeskConfig?.device_id || '-'
                      }}</code>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="size-8"
                        :aria-label="t('extensions.rustdesk.copyId')"
                        @click="copyToClipboard(rustdeskConfig?.device_id || '', 'id')"
                        :disabled="!rustdeskConfig?.device_id"
                      >
                        <Check v-if="rustdeskCopied === 'id'" class="size-4 text-success" />
                        <Copy v-else class="size-4" />
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        @click="regenerateRustdeskId"
                        :disabled="rustdeskLoading"
                      >
                        <RefreshCw class="size-4 mr-1" />
                        {{ t('extensions.rustdesk.regenerateId') }}
                      </Button>
                    </div>
                  </div>

                  <!-- Device Password (shown directly) -->
                  <div class="settings-field grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{
                      t('extensions.rustdesk.devicePassword')
                    }}</Label>
                    <div class="sm:col-span-3 flex items-center gap-2">
                      <code class="font-mono text-lg bg-muted px-3 py-1 rounded">{{
                        rustdeskPassword?.device_password || '-'
                      }}</code>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="size-8"
                        :aria-label="t('extensions.rustdesk.copyPassword')"
                        @click="
                          copyToClipboard(rustdeskPassword?.device_password || '', 'password')
                        "
                        :disabled="!rustdeskPassword?.device_password"
                      >
                        <Check v-if="rustdeskCopied === 'password'" class="size-4 text-success" />
                        <Copy v-else class="size-4" />
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        @click="regenerateRustdeskPassword"
                        :disabled="rustdeskLoading"
                      >
                        <RefreshCw class="size-4 mr-1" />
                        {{ t('extensions.rustdesk.regeneratePassword') }}
                      </Button>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- Redfish Section -->
          <div v-show="activeSection === 'third-party-access'" class="settings-section-services">
            <!-- Auto-restart: restarting progress -->
            <div
              v-if="autoRestarting"
              class="flex items-center gap-3 rounded-lg border bg-card px-4 py-3 text-sm shadow-sm"
            >
              <RefreshCw class="size-4 animate-spin text-primary shrink-0" />
              <div class="flex-1 min-w-0">
                <p class="font-medium">{{ t('settings.autoRestarting') }}</p>
                <p class="text-xs text-muted-foreground">
                  {{
                    webServerConfig.https_enabled
                      ? t('settings.autoRestartingHttpsDesc', { sec: autoRestartCountdown })
                      : t('settings.autoRestartingDesc')
                  }}
                </p>
              </div>
              <span
                v-if="webServerConfig.https_enabled && autoRestartCountdown > 0"
                class="tabular-nums text-lg font-bold text-primary shrink-0"
              >
                {{ autoRestartCountdown }}
              </span>
            </div>

            <!-- Auto-restart: HTTPS manual redirect (cert must be accepted by user) -->
            <Alert v-if="autoRestartManualUrl" variant="warning">
              <Lock />
              <AlertTitle>{{ t('settings.httpsManualRedirectTitle') }}</AlertTitle>
              <AlertDescription>{{ t('settings.httpsManualRedirectDesc') }}</AlertDescription>
              <Button
                as-child
                class="col-start-2 mt-2 w-full bg-warning text-warning-foreground hover:bg-warning/90"
              >
                <a :href="autoRestartManualUrl"><ExternalLink />{{ autoRestartManualUrl }}</a>
              </Button>
            </Alert>

            <!-- Auto-restart: failure / timeout -->
            <Alert v-if="autoRestartFailed" variant="destructive">
              <AlertTriangle />
              <AlertDescription class="flex items-center justify-between gap-3">
                <span>{{ t('settings.autoRestartFailed') }}</span>
                <Button
                  variant="outline"
                  size="sm"
                  class="text-foreground"
                  @click="triggerAutoRestart"
                >
                  <RefreshCw class="size-3 mr-1" />
                  {{ t('common.retry') }}
                </Button>
              </AlertDescription>
            </Alert>

            <Card>
              <CardHeader class="settings-card-heading"
                ><CardTitle :help="t('settings.redfishDesc')">{{
                  t('settings.redfishTitle')
                }}</CardTitle>
                <div class="settings-card-actions">
                  <div class="settings-save-action">
                    <Button @click="saveRedfishConfig" :disabled="redfishSaving || autoRestarting">
                      <RefreshCw
                        v-if="autoRestarting || redfishSaving"
                        class="size-4 mr-2 animate-spin"
                      />
                      <Save v-else class="size-4 mr-2" />
                      {{ autoRestarting ? t('settings.restarting') : t('common.save') }}
                    </Button>
                  </div>
                </div></CardHeader
              >
              <CardContent class="space-y-5">
                <div class="settings-toggle flex items-start justify-between gap-4">
                  <Label>{{ t('settings.redfishEnabled') }}</Label>
                  <Switch v-model="redfishEnabled" />
                </div>
                <div class="settings-field">
                  <Label>
                    {{ t('settings.redfishEndpoint') }}
                  </Label>
                  <code class="block py-2 font-mono text-sm break-all">{{
                    redfishAccessUrl
                  }}</code>
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <p class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                  <AlertTriangle class="size-3.5 shrink-0 text-warning" />
                  <span class="truncate">{{ t('settings.restartRequiredHint') }}</span>
                </p>
              </CardFooter>
            </Card>
          </div>

          <!-- About: usage, host identity and one upgrade card. -->
          <div v-show="activeSection === 'about'" class="space-y-4">
            <AboutSystemCards :active="activeSection === 'about'" />
            <Card class="update-card">
              <CardHeader
                ><CardTitle :help="t('settings.onlineUpgradeDesc')">{{
                  t('settings.onlineUpgrade')
                }}</CardTitle></CardHeader
              >
              <CardContent class="space-y-5">
                <div class="settings-field">
                  <Label>{{ t('settings.updateChannel') }}</Label
                  ><Select v-model="updateChannel" :disabled="updateRunning"
                    ><SelectTrigger class="w-full"><SelectValue /></SelectTrigger
                    ><SelectContent
                      ><SelectItem value="stable">Stable</SelectItem
                      ><SelectItem value="beta">Beta</SelectItem></SelectContent
                    ></Select
                  >
                </div>
                <div class="settings-field">
                  <Label>{{ t('settings.latestVersion') }}</Label>
                  <div class="flex min-w-0 items-center justify-between gap-4">
                    <span class="text-sm text-muted-foreground">{{
                      updateOverview?.latest_version || t('common.unknown')
                    }}</span
                    ><Button
                      variant="outline"
                      :disabled="updateRunning || updateLoading"
                      @click="loadUpdateOverview"
                      ><RefreshCw class="size-4" :class="{ 'animate-spin': updateLoading }" />{{
                        t('settingsUi.checkUpdates')
                      }}</Button
                    >
                  </div>
                </div>
                <div
                  v-if="
                    updateRunning ||
                    updateStatus?.phase === 'failed' ||
                    updateStatus?.phase === 'success'
                  "
                  class="space-y-2"
                  role="status"
                >
                  <p class="text-sm">{{ updateStatusBadgeText() }}</p>
                  <div class="h-2 overflow-hidden rounded bg-muted">
                    <div
                      class="h-full bg-primary transition-all"
                      :style="{
                        width: `${Math.max(0, Math.min(100, updateStatus?.progress || 0))}%`,
                      }"
                    />
                  </div>
                  <p v-if="updateStatus?.last_error" class="text-sm text-destructive">
                    {{ updateStatus.last_error }}
                  </p>
                </div>
                <section class="update-release-notes">
                  <div class="flex items-center justify-between gap-4">
                    <h3 class="text-sm font-semibold">{{ t('settings.releaseNotes') }}</h3>
                    <a
                      href="https://github.com/mofeng-git/One-KVM/releases"
                      target="_blank"
                      rel="noopener noreferrer"
                      class="inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:underline"
                      >{{ t('settingsUi.project') }}<ExternalLink class="size-3.5"
                    /></a>
                  </div>
                  <p v-if="updateLoading" class="text-sm text-muted-foreground">
                    {{ t('common.loading') }}
                  </p>
                  <p
                    v-else-if="!updateOverview?.notes_between?.length"
                    class="text-sm text-muted-foreground"
                  >
                    {{ t('settings.noUpdates') }}
                  </p>
                  <div v-else class="max-h-80 space-y-4 overflow-y-auto">
                    <article
                      v-for="item in updateOverview.notes_between"
                      :key="item.version"
                      class="space-y-2 border-b pb-4 last:border-0"
                    >
                      <div class="flex items-center justify-between gap-4">
                        <span class="text-sm font-medium">v{{ item.version }}</span
                        ><span class="text-sm text-muted-foreground">{{ item.published_at }}</span>
                      </div>
                      <ul class="list-disc space-y-1 pl-5 text-sm">
                        <li v-for="(note, idx) in item.notes" :key="idx">{{ note }}</li>
                      </ul>
                    </article>
                  </div>
                </section>
                <p v-if="!onlineUpgradeSupported" class="text-sm text-muted-foreground">
                  {{ t('settings.onlineUpgradeUnsupported') }}
                </p>
              </CardContent>
              <CardFooter v-if="onlineUpgradeSupported" class="justify-end border-t"
                ><Button
                  :disabled="updateRunning || !updateOverview?.upgrade_available"
                  @click="startOnlineUpgrade"
                  ><RefreshCw v-if="updateRunning" class="size-4 animate-spin" />{{
                    t('settings.startUpgrade')
                  }}</Button
                ></CardFooter
              >
            </Card>
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>

    <!-- Terminal Dialog -->
    <TerminalDialog v-model:open="showTerminalDialog" />

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
            <RefreshCw v-if="restarting" class="size-4 mr-2 animate-spin" />
            {{ restarting ? t('settings.restarting') : t('common.restartNow') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </AppLayout>
</template>
