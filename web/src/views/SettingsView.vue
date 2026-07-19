<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
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
  hidApi,
  streamApi,
  atxConfigApi,
  extensionsApi,
  redfishConfigApi,
  rtspConfigApi,
  rustdeskConfigApi,
  systemApi,
  updateApi,
  usbApi,
  vncConfigApi,
  watchdogConfigApi,
  type EncoderBackendInfo,
  type AuthConfig,
  type RustDeskConfigResponse,
  type RustDeskStatusResponse,
  type RustDeskPasswordResponse,
  type RtspStatusResponse,
  type RtspConfigUpdate,
  type VncConfigUpdate,
  type VncStatusResponse,
  type WebConfig,
  type UpdateOverviewResponse,
  type UpdateStatusResponse,
  type UpdateChannel,
  type VideoEncoderSelfCheckResponse,
} from '@/api'
import type {
  ExtensionsStatus,
  ExtensionStatus,
  AtxDriverType,
  ActiveLevel,
  AtxDevices,
  OtgHidProfile,
  OtgHidFunctions,
  Ch9329DescriptorConfig,
  Ch9329DescriptorState,
  NetworkInterfaceInfo,
  OtgNetworkStatus,
  WatchdogConfigResponse,
} from '@/types/generated'
import { FrpProxyType, FrpcConfigMode } from '@/types/generated'
import { formatFpsLabel, toConfigFps } from '@/lib/fps'
import { useClipboard } from '@/composables/useClipboard'
import { useFeatureVisibility } from '@/composables/useFeatureVisibility'
import { useTheme } from '@/composables/useTheme'
import { getVideoFormatState } from '@/lib/video-format-support'
import { formatVideoDeviceLabel } from '@/lib/video-device-label'
import AppLayout from '@/components/AppLayout.vue'
import LanguageToggleButton from '@/components/LanguageToggleButton.vue'
import TerminalDialog from '@/components/TerminalDialog.vue'
import TotpSettingsCard from '@/components/TotpSettingsCard.vue'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Separator } from '@/components/ui/separator'
import { Badge } from '@/components/ui/badge'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { ButtonGroup } from '@/components/ui/button-group'
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
  SidebarMenuButton,
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
} from 'lucide-vue-next'

const { t, te } = useI18n()
const route = useRoute()
const router = useRouter()
const systemStore = useSystemStore()
const configStore = useConfigStore()
const authStore = useAuthStore()
const featureVisibility = useFeatureVisibility()
const { theme, setTheme } = useTheme()

const isWindows = computed(() => systemStore.platform?.mode === 'windows')

const activeSection = ref<SettingsSectionId>('appearance')
const loading = ref(false)
const saved = ref(false)
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
  'ext-remote-access',
  'about',
] as const
type SettingsSectionId = typeof SETTINGS_SECTION_IDS[number]
const SETTINGS_SECTION_ID_SET = new Set<string>(SETTINGS_SECTION_IDS)

const navGroups = computed(() => [
  {
    title: t('settings.software'),
    items: [
      { id: 'appearance', label: t('settings.appearance'), icon: Sun },
      { id: 'account', label: t('settings.account'), icon: User },
      { id: 'network', label: t('settings.network'), icon: Globe },
      { id: 'about', label: t('settings.about'), icon: Info },
    ]
  },
  {
    title: t('settings.system'),
    items: [
      { id: 'video', label: t('settings.video'), icon: Monitor },
      { id: 'hid', label: t('settings.hid'), icon: Keyboard },
      { id: 'atx', label: t('settings.atx'), icon: Power },
      { id: 'environment', label: t('settings.environment'), icon: Server },
      { id: 'other', label: t('settings.other'), icon: Wrench },
    ]
  },
  {
    title: t('settings.extensions'),
    items: [
      { id: 'ext-ttyd', label: t('extensions.ttyd.title'), icon: Terminal },
      { id: 'third-party-access', label: t('extensions.thirdPartyAccess.title'), icon: ScreenShare },
      { id: 'ext-remote-access', label: t('extensions.remoteAccess.title'), icon: ExternalLink },
    ]
  }
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
    case 'ext-ttyd': return 'extTtydSubtitle'
    case 'third-party-access': return 'thirdPartyAccessSubtitle'
    case 'ext-remote-access': return 'extRemoteAccessSubtitle'
    default: return `${id}Subtitle`
  }
}

function isSettingsSectionId(value: string): value is SettingsSectionId {
  return SETTINGS_SECTION_ID_SET.has(value)
}

function selectSection(id: string) {
  if (!isSettingsSectionId(id)) return
  activeSection.value = id
  void loadSectionData(id)
}

function normalizeSettingsSection(value: unknown): SettingsSectionId | null {
  if (typeof value !== 'string') return null
  if (value === 'access-control') return 'account'
  if (value === 'ext-frpc') return 'ext-remote-access'
  if (value === 'redfish') return 'third-party-access'
  if (value === 'msd') return 'hid'
  if (value === 'ext-rustdesk' || value === 'ext-vnc' || value === 'ext-rtsp') return 'third-party-access'
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
      await Promise.all([
        loadConfig(),
        loadDevices(),
        loadBackends(),
      ])
      return
    case 'hid':
      await Promise.all([
        loadConfig(),
        loadDevices(),
      ])
      return
    case 'atx':
      await Promise.all([
        loadConfig(),
        loadAtxConfig(),
        loadAtxDevices(),
      ])
      return
    case 'environment':
      return
    case 'other':
      await Promise.all([
        loadWatchdogConfig(),
        fetchUsbDevices(),
      ])
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
      await Promise.all([
        loadUpdateOverview(),
        refreshUpdateStatus(),
      ])
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
  if (reason.includes('No hardware watchdog device found')) return t('settings.watchdog.unsupportedReason')
  if (reason.includes('discover')) return t('settings.watchdog.discoveryFailed')
  if (reason.includes('open a hardware watchdog')) return t('settings.watchdog.openFailed')
  if (reason.includes('keepalive')) return t('settings.watchdog.feedFailed')
  if (reason.includes('safely disabled') || reason.includes('nowayout')) return t('settings.watchdog.disableFailed')
  return t('settings.watchdog.abnormalReason')
})

function watchdogRequestError(error: unknown, action: 'enable' | 'disable'): string {
  const message = error instanceof Error ? error.message : ''
  if (message.includes('No hardware watchdog device found')) return t('settings.watchdog.unsupportedReason')
  if (message.includes('cannot be safely disabled') || message.includes('nowayout')) {
    return t('settings.watchdog.disableFailed')
  }
  return t(action === 'enable' ? 'settings.watchdog.enableFailed' : 'settings.watchdog.toggleFailed')
}

const watchdogStatusClass = computed(() => {
  switch (watchdogStatusKey.value) {
    case 'running': return 'bg-success'
    case 'error': return 'bg-destructive'
    default: return 'bg-muted-foreground'
  }
})

async function loadWatchdogConfig() {
  watchdogLoading.value = true
  watchdogError.value = ''
  try {
    watchdogStatus.value = await watchdogConfigApi.get()
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
  gostc: [],
  easytier: [],
  frpc: [],
})
const showLogs = ref<Record<string, boolean>>({
  ttyd: false,
  gostc: false,
  easytier: false,
  frpc: false,
})

const showTerminalDialog = ref(false)

const extConfig = ref({
  ttyd: { enabled: false, shell: '/bin/bash' },
  gostc: { enabled: false, addr: '', key: '', tls: true },
  easytier: { enabled: false, network_name: '', network_secret: '', peer_urls: [] as string[], virtual_ip: '' },
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

const gostcValidationMessage = computed(() => {
  if (!extConfig.value.gostc.addr?.trim()) return t('extensions.gostc.addrRequired')
  if (!extConfig.value.gostc.key) return t('extensions.gostc.keyRequired')
  return ''
})

const easytierValidationMessage = computed(() => {
  if (!extConfig.value.easytier.network_name?.trim()) return t('extensions.easytier.networkNameRequired')
  return ''
})

const frpcRemotePortRequired = computed(() => ['tcp', 'udp'].includes(extConfig.value.frpc.proxy_type))
const showFrpcRemotePort = computed(() => ['tcp', 'udp', 'stcp', 'sudp', 'xtcp'].includes(extConfig.value.frpc.proxy_type))
const showFrpcCustomDomain = computed(() => ['http', 'https'].includes(extConfig.value.frpc.proxy_type))
const showFrpcSecretKey = computed(() => ['stcp', 'sudp', 'xtcp'].includes(extConfig.value.frpc.proxy_type))
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
  if (frpcRemotePortRequired.value && !extConfig.value.frpc.remote_port) return t('extensions.frpc.remotePortRequired')
  return ''
})

const rustdeskConfig = ref<RustDeskConfigResponse | null>(null)
const rustdeskStatus = ref<RustDeskStatusResponse | null>(null)
const rustdeskPassword = ref<RustDeskPasswordResponse | null>(null)
const rustdeskLoading = ref(false)
const rustdeskCopied = ref<'id' | 'password' | null>(null)
const { copy: clipboardCopy } = useClipboard()
const rustdeskLocalConfig = ref({
  enabled: false,
  codec: 'h264' as 'h264' | 'h265',
  rendezvous_server: '',
  relay_server: '',
  relay_key: '',
})

const rustdeskValidationMessage = computed(() => {
  if (!rustdeskLocalConfig.value.rendezvous_server?.trim()) {
    return t('extensions.rustdesk.rendezvousServerRequired')
  }
  return ''
})

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

const vncStatus = ref<VncStatusResponse | null>(null)
const vncLoading = ref(false)
const vncLocalConfig = ref<VncConfigUpdate & { password?: string }>({
  enabled: false,
  bind: '0.0.0.0',
  port: 5900,
  encoding: 'tight_jpeg',
  allow_one_client: true,
  password: '',
})

function formatHostForUrl(hostname: string): string {
  if (!hostname) return '127.0.0.1'
  return hostname.includes(':') && !hostname.startsWith('[')
    ? `[${hostname}]`
    : hostname
}

const rtspStreamUrl = computed(() => {
  const host = formatHostForUrl(window.location.hostname || '127.0.0.1')
  const path = (rtspLocalConfig.value.path || 'live').trim().replace(/^\/+|\/+$/g, '') || 'live'
  const port = Number(rtspLocalConfig.value.port) || 8554
  return `rtsp://${host}:${port}/${path}`
})

const vncStreamUrl = computed(() => {
  const host = formatHostForUrl(window.location.hostname || '127.0.0.1')
  const port = Number(vncLocalConfig.value.port) || 5900
  return `${host}:${port}`
})

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

/** 预览当前配置生效后的访问 URL（取第一个非通配地址显示） */
const previewAccessUrl = computed(() => {
  const https = webServerConfig.value.https_enabled
  const port = https ? webServerConfig.value.https_port : webServerConfig.value.http_port
  const scheme = https ? 'https' : 'http'
  // 对通配地址，用当前浏览器 hostname 替代
  const addrs = effectiveBindAddresses.value
  const firstAddr = addrs.find(a => a !== '0.0.0.0' && a !== '::') ?? window.location.hostname
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
  hid_otg_profile: 'custom' as OtgHidProfile,
  hid_otg_functions: {
    keyboard: true,
    mouse_relative: true,
    mouse_absolute: true,
    consumer: true,
  } as OtgHidFunctions,
  hid_otg_keyboard_leds: false,
  hid_ch9329_hybrid_mouse: false,
  msd_enabled: false,
  msd_dir: '',
  otg_network_enabled: false,
  otg_network_driver: 'ncm' as 'ncm' | 'ecm' | 'rndis',
  otg_network_interface: '',
  encoder_backend: 'auto',
  stun_server: '',
  turn_server: '',
  turn_username: '',
  turn_password: '',
})

const otgNetworkInterfaces = ref<NetworkInterfaceInfo[]>([])
const otgNetworkInterfacesLoaded = ref(false)
const otgNetworkStatus = ref<OtgNetworkStatus | null>(null)

function syncOtgNetworkInterface() {
  const firstInterface = otgNetworkInterfaces.value[0]?.name || ''
  const selectedInterfaceExists = otgNetworkInterfaces.value.some(
    item => item.name === config.value.otg_network_interface,
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
  if (status === 'ok') return 'bg-success'
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

function videoEncoderCell(row: VideoEncoderSelfCheckRow, codecId: string): VideoEncoderSelfCheckCell | undefined {
  return row.cells.find(cell => cell.codec_id === codecId)
}

const currentHardwareEncoderText = computed(() =>
  videoEncoderSelfCheckResult.value?.current_hardware_encoder === 'None'
    ? t('settings.encoderSelfCheck.none')
    : (videoEncoderSelfCheckResult.value?.current_hardware_encoder || t('settings.encoderSelfCheck.none'))
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
  if (!usbResetTarget.value) return
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

const isKeyboardLedToggleDisabled = computed(() =>
  config.value.hid_backend !== 'otg' || !effectiveOtgFunctions.value.keyboard
)

const isHidFunctionSelectionValid = computed(() => {
  if (config.value.hid_backend !== 'otg') return true
  const f = config.value.hid_otg_functions
  return !!(f.keyboard || f.mouse_relative || f.mouse_absolute || f.consumer)
})

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
  if (config.value.hid_backend !== 'ch9329') return false
  const source = ch9329DescriptorSource.value
  if (!source) return false
  const current = currentCh9329DescriptorSource()
  return source.port === current.port && source.baudrate === current.baudrate
})

async function loadCh9329Descriptor() {
  if (config.value.hid_backend !== 'ch9329') return
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
    ch9329DescriptorError.value = e instanceof Error ? e.message : t('settings.ch9329DescriptorLoadFailed')
  } finally {
    ch9329DescriptorLoading.value = false
  }
}

const isCh9329DescriptorValid = computed(() => {
  if (config.value.hid_backend !== 'ch9329') return true
  return utf8ByteLength(ch9329Manufacturer.value) <= 23
    && utf8ByteLength(ch9329Product.value) <= 23
    && utf8ByteLength(ch9329SerialNumber.value) <= 23
})

const canEditCh9329Descriptor = computed(() =>
  config.value.hid_backend === 'ch9329'
  && ch9329DescriptorLoaded.value
  && isCh9329DescriptorSourceCurrent.value
  && !ch9329DescriptorLoading.value
)

const isCh9329DescriptorDirty = computed(() => {
  if (!canEditCh9329Descriptor.value || !ch9329DescriptorBaseline.value) return false
  const current = currentCh9329DescriptorForm()
  const baseline = ch9329DescriptorBaseline.value
  return current.vendorId !== baseline.vendorId
    || current.productId !== baseline.productId
    || current.manufacturer !== baseline.manufacturer
    || current.product !== baseline.product
    || current.serialNumber !== baseline.serialNumber
})

const isHidSettingsValid = computed(() =>
  isHidFunctionSelectionValid.value
  && isCh9329DescriptorValid.value
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
  if (config.value.hid_backend !== 'ch9329') return ''
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
    ? options.filter(option => ['none', 'serial'].includes(option.value))
    : options
})

const availableBackends = ref<EncoderBackendInfo[]>([])

const selectedBackendFormats = computed(() => {
  if (config.value.encoder_backend === 'auto') return []
  const backend = availableBackends.value.find(b => b.id === config.value.encoder_backend)
  return backend?.supported_formats || []
})

const selectedDevice = computed(() => {
  return devices.value.video.find(d => d.path === config.value.video_device)
})

const availableFormats = computed(() => {
  if (!selectedDevice.value) return []
  return selectedDevice.value.formats
})

const availableFormatOptions = computed(() => {
  return availableFormats.value.map(format => {
    const state = getVideoFormatState(format.format, 'config', config.value.encoder_backend)
    return {
      ...format,
      state,
      disabled: state === 'unsupported',
    }
  })
})

const selectableFormats = computed(() => {
  return availableFormatOptions.value.filter(format => !format.disabled)
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

watch(
  selectableFormats,
  () => {
    if (selectableFormats.value.length === 0) {
      config.value.video_format = ''
      return
    }

    const isValid = selectableFormats.value.some(f => f.format === config.value.video_format)
    if (!isValid) {
      config.value.video_format = selectableFormats.value[0]?.format || ''
    }
  },
  { deep: true },
)

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


function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`
}

const hasDeviceCpuUsage = computed(() => {
  return !!systemStore.deviceInfo
})

const hasDeviceMemoryUsage = computed(() => {
  const info = systemStore.deviceInfo
  return !!info && info.memory_total > 0
})

const hasDeviceNetworkAddresses = computed(() => {
  return (systemStore.deviceInfo?.network_addresses.length ?? 0) > 0
})

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

async function saveConfig() {
  loading.value = true
  saved.value = false
  saveError.value = ''

  try {
    if (activeSection.value === 'video') {
      await configStore.updateStream({
        encoder: config.value.encoder_backend as any,
        stun_server: config.value.stun_server.trim(),
        turn_server: config.value.turn_server.trim(),
        turn_username: config.value.turn_username.trim(),
        turn_password: config.value.turn_password.trim(),
      })
      await configStore.updateVideo({
        device: config.value.video_device || undefined,
        format: config.value.video_format || undefined,
        width: config.value.video_width,
        height: config.value.video_height,
        fps: toConfigFps(config.value.video_fps),
      })
    }

    if (activeSection.value === 'hid') {
      if (!isHidSettingsValid.value) {
        return
      }
      const hidUpdate: any = {
        backend: config.value.hid_backend as any,
        ch9329_port: config.value.hid_serial_device || undefined,
        ch9329_baudrate: config.value.hid_serial_baudrate,
        ch9329_hybrid_mouse: config.value.hid_ch9329_hybrid_mouse,
        otg_udc: config.value.hid_otg_udc,
      }
      if (config.value.hid_backend === 'ch9329' && isCh9329DescriptorDirty.value) {
        hidUpdate.ch9329_descriptor = {
          vendor_id: parseInt(ch9329VendorIdHex.value, 16) || 0x1a86,
          product_id: parseInt(ch9329ProductIdHex.value, 16) || 0xe129,
          manufacturer: ch9329Manufacturer.value,
          product: ch9329Product.value,
          serial_number: ch9329SerialNumber.value || '',
        }
      }
      if (config.value.hid_backend === 'otg') {
        hidUpdate.otg_descriptor = {
          vendor_id: parseInt(otgVendorIdHex.value, 16) || 0x1d6b,
          product_id: parseInt(otgProductIdHex.value, 16) || 0x0104,
          manufacturer: otgManufacturer.value || 'One-KVM',
          product: otgProduct.value || 'One-KVM USB Device',
          serial_number: otgSerialNumber.value || undefined,
        }
        hidUpdate.otg_profile = 'custom'
        hidUpdate.otg_functions = { ...config.value.hid_otg_functions }
        hidUpdate.otg_keyboard_leds = config.value.hid_otg_keyboard_leds
      }
      const otgEnabled = config.value.hid_backend === 'otg'
      const response = await configStore.updateOtg({
        hid: hidUpdate,
        msd: {
          enabled: otgEnabled && config.value.msd_enabled,
          msd_dir: config.value.msd_dir || undefined,
        },
        network: {
          enabled: otgEnabled && config.value.otg_network_enabled,
          driver_mode: config.value.otg_network_driver as any,
          bridge_interface: config.value.otg_network_interface,
        },
      })
      otgNetworkStatus.value = response.status
    }

    if (activeSection.value !== 'hid') {
      await loadSectionData(activeSection.value)
    }
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch (error) {
    saveError.value = error instanceof Error ? error.message : t('api.operationFailedDesc')
    if (activeSection.value === 'hid') {
      await loadConfig().catch(() => undefined)
      otgNetworkStatus.value = await otgNetworkApi.status().catch(() => null)
    }
  } finally {
    loading.value = false
  }
}

async function loadConfig() {
  try {
    const [video, stream, hid, msd, otgNetwork] = await Promise.all([
      configStore.refreshVideo(),
      configStore.refreshStream(),
      configStore.refreshHid(),
      configStore.refreshMsd(),
      otgNetworkApi.get().catch(() => ({
        enabled: false,
        driver_mode: 'ncm' as const,
        bridge_interface: '',
        host_mac: '',
        device_mac: '',
      })),
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
      hid_ch9329_hybrid_mouse: hid.ch9329_hybrid_mouse ?? false,
      msd_enabled: msd.enabled || false,
      msd_dir: msd.msd_dir || '',
      otg_network_enabled: otgNetwork.enabled,
      otg_network_driver: otgNetwork.driver_mode,
      otg_network_interface: otgNetwork.bridge_interface,
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
      if (hid.backend !== 'ch9329') {
        applyCh9329DescriptorForm(hid.ch9329_descriptor, true)
      }
    }
    if (hid.backend === 'ch9329') {
      await loadCh9329Descriptor()
    } else {
      clearCh9329DescriptorState()
    }
    otgNetworkStatus.value = await otgNetworkApi.status().catch(() => null)
  } catch {
  }
}

async function loadDevices() {
  try {
    const [deviceConfig, networkInterfaces] = await Promise.all([
      configApi.listDevices(),
      otgNetworkApi.interfaces().catch(() => []),
    ])
    devices.value = deviceConfig
    otgNetworkInterfaces.value = networkInterfaces
    otgNetworkInterfacesLoaded.value = true
    syncOtgNetworkInterface()
  } catch {
  }
}

async function loadBackends() {
  try {
    const result = await streamApi.getCodecs()
    availableBackends.value = result.backends || []
  } catch {
  }
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

async function loadExtensions() {
  extensionsLoading.value = true
  try {
    extensions.value = await extensionsApi.getAll()
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
      const frpc = extensions.value.frpc.config
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

type ExtensionConfigId = 'ttyd' | 'gostc' | 'easytier' | 'frpc'
type ValidatedExtensionConfigId = Exclude<ExtensionConfigId, 'ttyd'>

async function startExtension(id: ExtensionConfigId) {
  if (id !== 'ttyd' && !validateExtensionConfig(id)) return

  try {
    await extensionsApi.start(id)
    await loadExtensions()
  } catch {
  }
}

async function stopExtension(id: ExtensionConfigId) {
  try {
    await extensionsApi.stop(id)
    await loadExtensions()
  } catch {
  }
}

async function refreshExtensionLogs(id: ExtensionConfigId) {
  try {
    const result = await extensionsApi.logs(id, 100)
    extensionLogs.value[id] = result.logs
  } catch {
  }
}

async function saveExtensionConfig(id: ExtensionConfigId) {
  if (id !== 'ttyd') {
    const shouldValidate = extConfig.value[id].enabled
      || (id === 'frpc' && extConfig.value.frpc.config_mode === FrpcConfigMode.Full)
    if (shouldValidate && !validateExtensionConfig(id)) return
  }

  loading.value = true
  try {
    if (id === 'ttyd') {
      await extensionsApi.updateTtyd(extConfig.value.ttyd)
    } else if (id === 'gostc') {
      await extensionsApi.updateGostc(extConfig.value.gostc)
    } else if (id === 'easytier') {
      await extensionsApi.updateEasytier(extConfig.value.easytier)
    } else if (id === 'frpc') {
      const frpc = extConfig.value.frpc
      await extensionsApi.updateFrpc({
        ...frpc,
        remote_port: frpcQuickMode.value && showFrpcRemotePort.value ? frpc.remote_port : undefined,
        custom_domain: frpcQuickMode.value && showFrpcCustomDomain.value ? frpc.custom_domain || undefined : undefined,
        secret_key: frpcQuickMode.value && showFrpcSecretKey.value ? frpc.secret_key : '',
      })
    }
    await loadExtensions()
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch {
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

function getExtStatusText(status: ExtensionStatus | undefined): string {
  if (!status) return t('extensions.stopped')
  switch (status.state) {
    case 'unavailable': return t('extensions.unavailable')
    case 'stopped': return t('extensions.stopped')
    case 'running': return t('extensions.running')
    default: return t('extensions.stopped')
  }
}

function getExtStatusClass(status: ExtensionStatus | undefined): string {
  if (!status) return 'bg-muted-foreground'
  switch (status.state) {
    case 'unavailable': return 'bg-muted-foreground'
    case 'stopped': return 'bg-muted-foreground'
    case 'running': return 'bg-success'
    default: return 'bg-muted-foreground'
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

async function loadAtxConfig() {
  try {
    const config = await configStore.refreshAtx()
    atxConfig.value = {
      enabled: config.enabled,
      driver: config.enabled ? config.driver : 'none' as AtxDriverType,
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
  } catch {
  }
}

async function loadAtxDevices() {
  try {
    atxDevices.value = await atxConfigApi.listDevices()
  } catch {
  }
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

function applyRustdeskStatus(status: RustDeskStatusResponse) {
  const config = status.config
  rustdeskConfig.value = config
  rustdeskStatus.value = status
  rustdeskLocalConfig.value = {
    enabled: config.enabled,
    codec: config.codec || 'h264',
    rendezvous_server: config.rendezvous_server,
    relay_server: config.relay_server || '',
    relay_key: config.relay_key || '',
  }
}

async function loadRustdeskConfig() {
  rustdeskLoading.value = true
  try {
    const status = await configStore.refreshRustdeskStatus()
    applyRustdeskStatus(status)
  } catch {
  } finally {
    rustdeskLoading.value = false
  }
}

async function loadRustdeskPassword() {
  try {
    rustdeskPassword.value = await configStore.refreshRustdeskPassword()
  } catch {
  }
}

function normalizeRustdeskServer(value: string, defaultPort: number): string {
  const trimmed = value.trim()
  if (!trimmed) return ''
  if (trimmed.includes(':')) return trimmed
  return `${trimmed}:${defaultPort}`
}

/** Strip line breaks from pasted keys. */
function normalizeRustdeskRelayKey(value: string): string {
  const cleaned = value.replace(/\r?\n/g, '').trim()
  return cleaned
}

function showValidationError(message: string): boolean {
  toast.error(t('api.operationFailed'), {
    description: message,
    duration: 4000,
  })
  return false
}

function validateExtensionConfig(id: ValidatedExtensionConfigId): boolean {
  let message = ''
  if (id === 'gostc') {
    message = gostcValidationMessage.value
  } else if (id === 'easytier') {
    message = easytierValidationMessage.value
  } else {
    message = frpcValidationMessage.value
  }

  return !message || showValidationError(message)
}

function validateRustdeskConfig(): boolean {
  return !rustdeskValidationMessage.value || showValidationError(rustdeskValidationMessage.value)
}

function validateVncConfig(enabled = vncLocalConfig.value.enabled): boolean {
  const password = (vncLocalConfig.value.password || '').trim()
  if (enabled && !vncStatus.value?.config.has_password && !password) {
    return showValidationError(t('extensions.vnc.passwordRequired'))
  }
  if (password.length > 8) {
    return showValidationError(t('extensions.vnc.passwordMaxLength'))
  }
  return true
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

async function loadWebServerConfig() {
  try {
    const config = await configStore.refreshWeb()
    webServerConfig.value = config
    applyBindStateFromConfig(config)
  } catch {
  }
}

async function loadRedfishConfig() {
  try {
    const data = await redfishConfigApi.get()
    redfishEnabled.value = data.enabled
  } catch {
  }
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

async function saveWebServerConfig() {
  if (bindAddressError.value) return
  webServerLoading.value = true
  try {
    const updated = await configStore.updateWeb({
      http_port: webServerConfig.value.http_port,
      https_port: webServerConfig.value.https_port,
      https_enabled: webServerConfig.value.https_enabled,
      bind_addresses: effectiveBindAddresses.value,
    })
    webServerConfig.value = updated
    applyBindStateFromConfig(updated)
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
    await new Promise(r => setTimeout(r, 800))
    try {
      const ctrl = new AbortController()
      const tid = setTimeout(() => ctrl.abort(), 1500)
      const res = await fetch(healthUrl, { signal: ctrl.signal })
      clearTimeout(tid)
      if (res.ok) return true
    } catch {
    }
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
        await new Promise(r => setTimeout(r, 1000))
        autoRestartCountdown.value = i
      }
      autoRestartManualUrl.value = targetOrigin
      autoRestarting.value = false
    } else {
      // HTTP：可以安全轮询，服务恢复后自动跳转。
      await new Promise(r => setTimeout(r, 1200))
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
  try {
    updateSawRestarting.value = false
    updateSawRequestFailure.value = false
    updateAutoReloadTriggered.value = false
    await updateApi.upgrade({ channel: updateChannel.value })
    await refreshUpdateStatus()
    startUpdatePolling()
  } catch {
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

function rustdeskUpdatePayload(enabled = rustdeskLocalConfig.value.enabled) {
  return {
    enabled,
    codec: rustdeskLocalConfig.value.codec,
    rendezvous_server: normalizeRustdeskServer(
      rustdeskLocalConfig.value.rendezvous_server,
      21116,
    ),
    relay_server: normalizeRustdeskServer(rustdeskLocalConfig.value.relay_server, 21117),
    relay_key: normalizeRustdeskRelayKey(rustdeskLocalConfig.value.relay_key),
  }
}

async function saveRustdeskConfig() {
  if (rustdeskLocalConfig.value.enabled && !validateRustdeskConfig()) return

  loading.value = true
  saved.value = false
  try {
    await configStore.updateRustdesk(rustdeskUpdatePayload())
    await loadRustdeskConfig()
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch {
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
  } catch {
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
  } catch {
  } finally {
    rustdeskLoading.value = false
  }
}

async function startRustdesk() {
  if (!validateRustdeskConfig()) return

  rustdeskLoading.value = true
  try {
    await configStore.updateRustdesk(rustdeskUpdatePayload())
    const status = await rustdeskConfigApi.start()
    applyRustdeskStatus(status)
  } catch {
  } finally {
    rustdeskLoading.value = false
  }
}

async function stopRustdesk() {
  rustdeskLoading.value = true
  try {
    const status = await rustdeskConfigApi.stop()
    applyRustdeskStatus(status)
  } catch {
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
      if (status.startsWith('error:')) return t('extensions.failed')
      return status
  }
}

function getRustdeskStatusClass(status: string | null | undefined): string {
  switch (status) {
    case 'running':
    case 'registered':
    case 'connected': return 'bg-success'
    case 'starting':
    case 'connecting': return 'bg-warning'
    case 'stopped':
    case 'not_initialized':
    case 'disconnected': return 'bg-muted-foreground'
    default:
      if (status?.startsWith('error:')) return 'bg-destructive'
      return 'bg-muted-foreground'
  }
}

function applyRtspStatus(status: RtspStatusResponse) {
  rtspStatus.value = status
  rtspLocalConfig.value = {
    enabled: status.config.enabled,
    bind: status.config.bind,
    port: status.config.port,
    path: status.config.path,
    allow_one_client: status.config.allow_one_client,
    codec: status.config.codec,
    username: status.config.username || '',
    password: status.config.password || '',
  }
}

async function loadRtspConfig() {
  rtspLoading.value = true
  try {
    const status = await configStore.refreshRtspStatus()
    applyRtspStatus(status)
  } catch {
  } finally {
    rtspLoading.value = false
  }
}

function rtspUpdatePayload(enabled = !!rtspLocalConfig.value.enabled): RtspConfigUpdate {
  return {
    enabled,
    bind: rtspLocalConfig.value.bind?.trim() || '0.0.0.0',
    port: Number(rtspLocalConfig.value.port) || 8554,
    path: normalizeRtspPath(rtspLocalConfig.value.path || 'live'),
    allow_one_client: !!rtspLocalConfig.value.allow_one_client,
    codec: rtspLocalConfig.value.codec || 'h264',
    username: (rtspLocalConfig.value.username || '').trim(),
    password: (rtspLocalConfig.value.password || '').trim(),
  }
}

async function saveRtspConfig() {
  loading.value = true
  saved.value = false
  try {
    await configStore.updateRtsp(rtspUpdatePayload())
    await loadRtspConfig()
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch {
  } finally {
    loading.value = false
  }
}

async function startRtsp() {
  rtspLoading.value = true
  try {
    await configStore.updateRtsp(rtspUpdatePayload())
    const status = await rtspConfigApi.start()
    applyRtspStatus(status)
  } catch {
  } finally {
    rtspLoading.value = false
  }
}

async function stopRtsp() {
  rtspLoading.value = true
  try {
    const status = await rtspConfigApi.stop()
    applyRtspStatus(status)
  } catch {
  } finally {
    rtspLoading.value = false
  }
}

function applyVncStatus(status: VncStatusResponse) {
  vncStatus.value = status
  vncLocalConfig.value = {
    enabled: status.config.enabled,
    bind: status.config.bind,
    port: status.config.port,
    encoding: status.config.encoding,
    allow_one_client: status.config.allow_one_client,
    password: '',
  }
}

async function loadVncConfig() {
  vncLoading.value = true
  try {
    const status = await configStore.refreshVncStatus()
    applyVncStatus(status)
  } catch {
  } finally {
    vncLoading.value = false
  }
}

function vncUpdatePayload(enabled = !!vncLocalConfig.value.enabled): VncConfigUpdate {
  const update: VncConfigUpdate = {
    enabled,
    bind: vncLocalConfig.value.bind?.trim() || '0.0.0.0',
    port: Number(vncLocalConfig.value.port) || 5900,
    encoding: vncLocalConfig.value.encoding || 'tight_jpeg',
    allow_one_client: !!vncLocalConfig.value.allow_one_client,
  }
  const password = (vncLocalConfig.value.password || '').trim()
  if (password) update.password = password
  return update
}

async function saveVncConfig() {
  if (!validateVncConfig()) return

  loading.value = true
  saved.value = false
  try {
    await configStore.updateVnc(vncUpdatePayload())
    await loadVncConfig()
    saved.value = true
    setTimeout(() => (saved.value = false), 2000)
  } catch {
  } finally {
    loading.value = false
  }
}

async function startVnc() {
  if (!validateVncConfig(true)) return

  vncLoading.value = true
  try {
    await configStore.updateVnc(vncUpdatePayload())
    const status = await vncConfigApi.start()
    applyVncStatus(status)
  } catch {
  } finally {
    vncLoading.value = false
  }
}

async function stopVnc() {
  vncLoading.value = true
  try {
    const status = await vncConfigApi.stop()
    applyVncStatus(status)
  } catch {
  } finally {
    vncLoading.value = false
  }
}

function getVncServiceStatusText(status: string | undefined): string {
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

function getVncStatusClass(status: string | undefined): string {
  switch (status) {
    case 'running': return 'bg-success'
    case 'starting': return 'bg-warning'
    case 'stopped': return 'bg-muted-foreground'
    default:
      if (status?.startsWith('error:')) return 'bg-destructive'
      return 'bg-muted-foreground'
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
    case 'running': return 'bg-success'
    case 'starting': return 'bg-warning'
    case 'stopped': return 'bg-muted-foreground'
    default:
      if (status?.startsWith('error:')) return 'bg-destructive'
      return 'bg-muted-foreground'
  }
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

watch(() => config.value.hid_backend, () => {
  otgSelfCheckResult.value = null
  otgSelfCheckError.value = ''
  if (config.value.hid_backend === 'ch9329') {
    void loadCh9329Descriptor()
  } else {
    clearCh9329DescriptorState()
  }
})

watch(
  () => [config.value.hid_serial_device, config.value.hid_serial_baudrate],
  () => {
    if (config.value.hid_backend === 'ch9329' && !isCh9329DescriptorSourceCurrent.value) {
      clearCh9329DescriptorState()
    }
  },
)

watch(() => route.query.tab, (tab) => {
  const section = normalizeSettingsSection(tab)
  if (section && activeSection.value !== section) {
    selectSection(section)
  }
})

watch(isWindows, () => {
  ensureVisibleSection()
})
</script>

<template>
  <AppLayout>
    <SidebarProvider class="h-full min-h-0 overflow-hidden">
      <Sidebar class="top-10 h-[calc(100dvh-2.5rem)] sm:top-14 sm:h-[calc(100dvh-3.5rem)]" collapsible="offcanvas">
        <SidebarHeader class="gap-1 px-6 pt-4 pb-2 text-foreground md:pt-6">
          <h1 class="text-xl font-semibold">{{ t('settings.title') }}</h1>
          <p class="text-xs text-muted-foreground">{{ t('settings.sidebarSubtitle') }}</p>
        </SidebarHeader>
        <SidebarContent class="px-3 pb-6 md:pb-10">
          <SidebarGroup v-for="group in navGroups" :key="group.title" class="px-0 py-1">
            <SidebarGroupLabel class="uppercase">{{ group.title }}</SidebarGroupLabel>
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
            </SidebarMenu>
          </SidebarGroup>
        </SidebarContent>
      </Sidebar>

      <SidebarInset class="min-w-0 overflow-y-auto">
      <!-- Mobile Header -->
      <div class="md:hidden sticky top-0 z-20 flex items-center px-3 sm:px-4 py-2 sm:py-3 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/70">
        <SidebarTrigger class="mr-1.5 size-8 sm:mr-2 sm:size-9" />
        <div class="flex items-center gap-2 min-w-0">
          <component :is="sectionMeta.icon" class="size-4 text-muted-foreground shrink-0" />
          <h1 class="text-sm sm:text-base font-semibold truncate">{{ sectionMeta.title }}</h1>
        </div>
      </div>
        <div class="mx-auto w-full max-w-4xl px-3 sm:px-6 lg:px-8 pt-6 pb-10 space-y-4 settings-dense">

          <!-- Section Header -->
          <header class="space-y-1.5 pb-2 border-b">
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
              <CardHeader>
                <CardTitle>{{ t('settings.theme') }}</CardTitle>
                <CardDescription>{{ t('settings.themeDesc') }}</CardDescription>
              </CardHeader>
              <CardContent>
                <div class="grid grid-cols-3 gap-2 sm:max-w-md">
                  <Button :variant="theme === 'light' ? 'default' : 'outline'" size="sm" class="justify-center" @click="setTheme('light')">
                    <Sun class="size-4 mr-1.5" />{{ t('settings.lightMode') }}
                  </Button>
                  <Button :variant="theme === 'dark' ? 'default' : 'outline'" size="sm" class="justify-center" @click="setTheme('dark')">
                    <Moon class="size-4 mr-1.5" />{{ t('settings.darkMode') }}
                  </Button>
                  <Button :variant="theme === 'system' ? 'default' : 'outline'" size="sm" class="justify-center" @click="setTheme('system')">
                    <Monitor class="size-4 mr-1.5" />{{ t('settings.systemMode') }}
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
                <LanguageToggleButton variant="outline" size="sm" label-mode="current" />
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.featureVisibility') }}</CardTitle>
                <CardDescription>{{ t('settings.featureVisibilityDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-1">
                <div class="flex items-center justify-between gap-4 px-3 py-3">
                  <Label for="feature-web-terminal" class="flex min-w-0 items-center gap-2 font-normal">
                    <Terminal class="size-4 shrink-0 text-muted-foreground" />
                    <span class="truncate">{{ t('actionbar.webTerminal') }}</span>
                  </Label>
                  <Switch id="feature-web-terminal" v-model="featureVisibility.webTerminal" />
                </div>
                <div class="flex items-center justify-between gap-4 px-3 py-3">
                  <Label for="feature-computer-use" class="flex min-w-0 items-center gap-2 font-normal">
                    <Bot class="size-4 shrink-0 text-muted-foreground" />
                    <span class="truncate">{{ t('settings.computerUseAgent') }}</span>
                  </Label>
                  <Switch id="feature-computer-use" v-model="featureVisibility.computerUse" />
                </div>
                <div class="flex items-center justify-between gap-4 px-3 py-3">
                  <Label for="feature-paste-text" class="flex min-w-0 items-center gap-2 font-normal">
                    <ClipboardPaste class="size-4 shrink-0 text-muted-foreground" />
                    <span class="truncate">{{ t('settings.pasteText') }}</span>
                  </Label>
                  <Switch id="feature-paste-text" v-model="featureVisibility.pasteText" />
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- Account Section -->
          <div v-show="activeSection === 'account'" class="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.username') }}</CardTitle>
                <CardDescription>{{ t('settings.usernameDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="account-username">{{ t('settings.username') }}</Label>
                  <Input id="account-username" v-model="usernameInput" autocomplete="username" />
                </div>
                <div class="space-y-2">
                  <Label for="account-username-password">{{ t('settings.currentPassword') }}</Label>
                  <Input id="account-username-password" v-model="usernamePassword" type="password" autocomplete="current-password" />
                </div>
                <p v-if="usernameError" class="text-xs text-destructive">{{ usernameError }}</p>
                <p v-else-if="usernameSaved" class="flex items-center gap-1.5 text-xs text-success"><Check class="size-3.5" />{{ t('common.success') }}</p>
              </CardContent>
              <CardFooter class="border-t pt-4 justify-end">
                <Button @click="changeUsername" :disabled="usernameSaving">
                  <Loader2 v-if="usernameSaving" class="size-4 mr-2 animate-spin" />
                  <Save v-else class="size-4 mr-2" />
                  {{ t('common.save') }}
                </Button>
              </CardFooter>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.changePassword') }}</CardTitle>
                <CardDescription>{{ t('settings.passwordDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="account-current-password">{{ t('settings.currentPassword') }}</Label>
                  <Input id="account-current-password" v-model="currentPassword" type="password" autocomplete="current-password" />
                </div>
                <div class="space-y-2">
                  <Label for="account-new-password">{{ t('settings.newPassword') }}</Label>
                  <Input id="account-new-password" v-model="newPassword" type="password" autocomplete="new-password" />
                </div>
                <div class="space-y-2">
                  <Label for="account-confirm-password">{{ t('auth.confirmPassword') }}</Label>
                  <Input id="account-confirm-password" v-model="confirmPassword" type="password" autocomplete="new-password" />
                </div>
                <p v-if="passwordError" class="text-xs text-destructive">{{ passwordError }}</p>
                <p v-else-if="passwordSaved" class="flex items-center gap-1.5 text-xs text-success"><Check class="size-3.5" />{{ t('common.success') }}</p>
              </CardContent>
              <CardFooter class="border-t pt-4 justify-end">
                <Button @click="changePassword" :disabled="passwordSaving">
                  <Loader2 v-if="passwordSaving" class="size-4 mr-2 animate-spin" />
                  <Save v-else class="size-4 mr-2" />
                  {{ t('common.save') }}
                </Button>
              </CardFooter>
            </Card>

            <TotpSettingsCard />

            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.authSettings') }}</CardTitle>
                <CardDescription>{{ t('settings.authSettingsDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-start justify-between gap-4">
                  <Label>{{ t('settings.allowMultipleSessions') }}</Label>
                  <Switch
                    v-model="authConfig.single_user_allow_multiple_sessions"
                    :disabled="authConfigLoading"
                  />
                </div>
              </CardContent>
              <CardFooter class="border-t pt-4 justify-end">
                <Button @click="saveAuthConfig" :disabled="authConfigLoading">
                  <Loader2 v-if="authConfigLoading" class="size-4 mr-2 animate-spin" />
                  <Save v-else class="size-4 mr-2" />
                  {{ t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- Video Section -->
          <div v-show="activeSection === 'video'" class="space-y-4">
            <!-- Video Device Settings -->
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.videoSettings') }}</CardTitle>
                  <CardDescription>{{ t('settings.videoSettingsDesc') }}</CardDescription>
                </div>
                <Button variant="ghost" size="icon-sm" :aria-label="t('common.refresh')" @click="loadDevices">
                  <RefreshCw class="size-4" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="video-device">{{ t('settings.videoDevice') }}</Label>
                  <NativeSelect id="video-device" v-model="config.video_device" class="w-full">
                    <NativeSelectOption value="">{{ t('settings.selectDevice') }}</NativeSelectOption>
                    <NativeSelectOption v-for="dev in devices.video" :key="dev.path" :value="dev.path">{{ formatVideoDeviceLabel(dev) }}</NativeSelectOption>
                  </NativeSelect>
                </div>
                <div class="space-y-2">
                  <Label for="video-format">{{ t('settings.videoFormat') }}</Label>
                  <NativeSelect id="video-format" v-model="config.video_format" class="w-full" :disabled="!config.video_device">
                    <NativeSelectOption value="">{{ t('settings.selectFormat') }}</NativeSelectOption>
                    <NativeSelectOption
                      v-for="fmt in availableFormatOptions"
                      :key="fmt.format"
                      :value="fmt.format"
                      :disabled="fmt.disabled"
                    >
                      {{ fmt.format }} - {{ fmt.description }}{{ fmt.disabled ? t('common.notSupportedYet') : '' }}
                    </NativeSelectOption>
                  </NativeSelect>
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="video-resolution">{{ t('settings.resolution') }}</Label>
                    <NativeSelect id="video-resolution" :model-value="`${config.video_width}x${config.video_height}`" class="w-full" :disabled="!config.video_format" @update:model-value="value => { const parts = String(value).split('x').map(Number); if (parts[0] && parts[1]) { config.video_width = parts[0]; config.video_height = parts[1]; } }">
                      <NativeSelectOption v-for="res in availableResolutions" :key="`${res.width}x${res.height}`" :value="`${res.width}x${res.height}`">{{ res.width }}x{{ res.height }}</NativeSelectOption>
                    </NativeSelect>
                  </div>
                  <div class="space-y-2">
                    <Label for="video-fps">{{ t('settings.frameRate') }}</Label>
                    <NativeSelect id="video-fps" :model-value="config.video_fps" class="w-full" :disabled="!config.video_format" @update:model-value="value => config.video_fps = Number(value)">
                      <NativeSelectOption v-for="fps in availableFps" :key="fps" :value="fps">{{ formatFpsLabel(fps) }}</NativeSelectOption>
                      <NativeSelectOption v-if="!availableFps.includes(config.video_fps)" :value="config.video_fps">{{ formatFpsLabel(config.video_fps) }}</NativeSelectOption>
                    </NativeSelect>
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
                  <Select v-model="config.encoder_backend">
                    <SelectTrigger id="encoder-backend" class="w-full"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="auto">{{ t('settings.autoRecommended') }}</SelectItem>
                      <SelectItem v-for="backend in availableBackends" :key="backend.id" :value="backend.id">{{ backend.name }} {{ backend.is_hardware ? `(${t('settings.hardware')})` : `(${t('settings.software')})` }}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div v-if="config.encoder_backend !== 'auto' && selectedBackendFormats.length > 0" class="space-y-2">
                  <Label>{{ t('settings.supportedFormats') }}</Label>
                  <div class="flex flex-wrap gap-2">
                    <Badge v-for="format in selectedBackendFormats" :key="format" variant="outline">{{ format.toUpperCase() }}</Badge>
                  </div>
                </div>
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
                </div>
                <Separator />
                <div class="space-y-2">
                  <Label for="turn-server">{{ t('settings.turnServer') }}</Label>
                  <Input
                    id="turn-server"
                    v-model="config.turn_server"
                    :placeholder="t('settings.turnServerPlaceholder')"
                  />
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="turn-username">{{ t('settings.turnUsername') }}</Label>
                    <Input
                      id="turn-username"
                      v-model="config.turn_username"
                      :disabled="!config.stun_server && !config.turn_server"
                    />
                  </div>
                  <div class="space-y-2">
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
                        :aria-label="showPasswords ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
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
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.hidSettings') }}</CardTitle>
                  <CardDescription>{{ t('settings.hidSettingsDesc') }}</CardDescription>
                </div>
                <Button variant="ghost" size="icon-sm" :aria-label="t('common.refresh')" @click="loadDevices">
                  <RefreshCw class="size-4" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="hid-backend">{{ t('settings.hidBackend') }}</Label>
                  <Select v-model="config.hid_backend">
                    <SelectTrigger id="hid-backend" class="w-full"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="ch9329">CH9329 (Serial)</SelectItem>
                      <SelectItem value="otg">USB OTG</SelectItem>
                      <SelectItem value="none">{{ t('common.disabled') }}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div v-if="config.hid_backend === 'ch9329'" class="space-y-2">
                  <Label for="serial-device">{{ t('settings.serialDevice') }}</Label>
                  <NativeSelect id="serial-device" v-model="config.hid_serial_device" class="w-full">
                    <NativeSelectOption value="">{{ t('settings.selectDevice') }}</NativeSelectOption>
                    <NativeSelectOption v-for="dev in devices.serial" :key="dev.path" :value="dev.path">{{ dev.name }} ({{ dev.path }})</NativeSelectOption>
                  </NativeSelect>
                </div>
                <div v-if="config.hid_backend === 'ch9329'" class="space-y-2">
                  <Label for="serial-baudrate">{{ t('settings.baudRate') }}</Label>
                  <Select :model-value="config.hid_serial_baudrate" @update:model-value="value => config.hid_serial_baudrate = Number(value)">
                    <SelectTrigger id="serial-baudrate" class="w-full"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem v-for="baud in [9600, 19200, 38400, 57600, 115200]" :key="baud" :value="baud">{{ baud }}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div v-if="config.hid_backend === 'otg'" class="space-y-2">
                  <Label for="otg-udc">{{ t('settings.otgUdc') }}</Label>
                  <NativeSelect id="otg-udc" v-model="config.hid_otg_udc" class="w-full">
                    <NativeSelectOption value="">{{ t('settings.autoRecommended') }}</NativeSelectOption>
                    <NativeSelectOption v-for="udc in devices.udc" :key="udc.name" :value="udc.name">{{ udc.name }}</NativeSelectOption>
                  </NativeSelect>
                </div>

                <template v-if="config.hid_backend === 'ch9329'">
                  <Separator class="my-4" />
                  <div class="space-y-4">
                    <div>
                      <div class="flex items-start justify-between gap-3">
                        <div>
                          <h4 class="text-sm font-medium">{{ t('settings.ch9329Descriptor') }}</h4>
                          <p class="text-sm text-muted-foreground">{{ t('settings.ch9329DescriptorDesc') }}</p>
                        </div>
                        <Button variant="ghost" size="icon-sm" class="shrink-0" :aria-label="t('common.refresh')" :disabled="ch9329DescriptorLoading" @click="loadCh9329Descriptor">
                          <RefreshCw class="size-4" :class="{ 'animate-spin': ch9329DescriptorLoading }" />
                        </Button>
                      </div>
                    </div>
                    <p v-if="ch9329DescriptorLoading" class="text-sm text-muted-foreground flex items-center gap-2">
                      <Loader2 class="size-4 animate-spin" />
                      {{ t('settings.ch9329DescriptorLoading') }}
                    </p>
                    <p v-else-if="ch9329DescriptorError" class="text-sm text-destructive">
                      {{ ch9329DescriptorError }}
                    </p>
                    <div class="grid gap-4 sm:grid-cols-2">
                      <div class="space-y-2">
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
                      <div class="space-y-2">
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
                    <div class="space-y-2">
                      <Label for="ch9329-manufacturer">{{ t('settings.manufacturer') }}</Label>
                      <Input
                        id="ch9329-manufacturer"
                        v-model="ch9329Manufacturer"
                        placeholder="WCH.CN"
                        maxlength="23"
                        :disabled="!canEditCh9329Descriptor"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label for="ch9329-product">{{ t('settings.productName') }}</Label>
                      <Input
                        id="ch9329-product"
                        v-model="ch9329Product"
                        placeholder="CH9329"
                        maxlength="23"
                        :disabled="!canEditCh9329Descriptor"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label for="ch9329-serial">{{ t('settings.serialNumber') }}</Label>
                      <Input
                        id="ch9329-serial"
                        v-model="ch9329SerialNumber"
                        :placeholder="t('settings.serialNumberAuto')"
                        maxlength="23"
                        :disabled="!canEditCh9329Descriptor"
                      />
                    </div>
                    <p v-if="!ch9329DescriptorLoading && !ch9329DescriptorLoaded && !ch9329DescriptorError" class="text-xs text-muted-foreground">
                      {{ t('settings.ch9329DescriptorReadRequired') }}
                    </p>
                    <p v-if="!isCh9329DescriptorValid" class="text-xs text-warning">
                      {{ t('settings.ch9329StringLengthWarning') }}
                    </p>
                    <p class="text-sm text-warning">
                      {{ t('settings.ch9329DescriptorWarning') }}
                    </p>
                  </div>
                  <Separator class="my-4" />
                  <div class="space-y-4">
                    <div>
                      <h4 class="text-sm font-medium">{{ t('settings.ch9329Options') }}</h4>
                      <p class="text-sm text-muted-foreground">{{ t('settings.ch9329OptionsDesc') }}</p>
                    </div>
                    <div class="space-y-3 rounded-md border border-border/60 p-3">
                      <div class="flex items-center justify-between gap-4">
                        <div>
                          <Label>{{ t('settings.ch9329HybridMouse') }}</Label>
                          <p class="text-xs text-muted-foreground">{{ t('settings.ch9329HybridMouseDesc') }}</p>
                        </div>
                        <Switch v-model="config.hid_ch9329_hybrid_mouse" />
                      </div>
                    </div>
                  </div>
                </template>

                <!-- OTG Descriptor Settings -->
                <template v-if="config.hid_backend === 'otg'">
                  <Separator class="my-4" />
                  <div class="space-y-4">
                    <div>
                      <h4 class="text-sm font-medium">{{ t('settings.otgDescriptor') }}</h4>
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
                    <p class="text-sm text-warning">
                      {{ t('settings.descriptorWarning') }}
                    </p>
                  </div>
                  <Separator class="my-4" />
                  <div class="space-y-4">
                    <div>
                      <h4 class="text-sm font-medium">{{ t('settings.otgHidProfile') }}</h4>
                    </div>
                    <div class="space-y-3">
                      <div class="space-y-3 rounded-md border border-border/60 p-3">
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
                      </div>
                      <div class="space-y-3 rounded-md border border-border/60 p-3">
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
                          <Switch v-model="config.hid_otg_keyboard_leds" :disabled="isKeyboardLedToggleDisabled" />
                        </div>
                      </div>
                      <div class="space-y-3 rounded-md border border-border/60 p-3">
                        <div class="flex items-center justify-between gap-4">
                          <div>
                            <Label>{{ t('settings.otgFunctionMsd') }}</Label>
                          </div>
                          <Switch v-model="config.msd_enabled" />
                        </div>
                        <template v-if="config.msd_enabled">
                          <Separator />
                          <div class="space-y-2">
                            <Label for="msd-dir">{{ t('settings.msdDir') }}</Label>
                            <Input id="msd-dir" v-model="config.msd_dir" placeholder="/etc/one-kvm/msd" />
                          </div>
                        </template>
                      </div>
                      <div class="space-y-3 rounded-md border border-border/60 p-3">
                        <div class="flex items-center justify-between gap-4">
                          <div>
                            <Label>{{ t('settings.otgNetwork') }}</Label>
                          </div>
                          <Switch v-model="config.otg_network_enabled" />
                        </div>
                        <template v-if="config.otg_network_enabled">
                          <Separator />
                          <div class="grid gap-4 sm:grid-cols-2">
                            <div class="space-y-2">
                              <Label for="otg-network-driver">{{ t('settings.otgNetworkDriver') }}</Label>
                              <Select v-model="config.otg_network_driver">
                                <SelectTrigger id="otg-network-driver" class="w-full"><SelectValue /></SelectTrigger>
                                <SelectContent>
                                  <SelectItem value="ncm">NCM</SelectItem>
                                  <SelectItem value="ecm">ECM</SelectItem>
                                  <SelectItem value="rndis">RNDIS / Windows</SelectItem>
                                </SelectContent>
                              </Select>
                            </div>
                            <div class="space-y-2">
                              <Label for="otg-network-interface">{{ t('settings.otgNetworkInterface') }}</Label>
                              <NativeSelect id="otg-network-interface" v-model="config.otg_network_interface" class="w-full" :disabled="otgNetworkInterfaces.length === 0">
                                <NativeSelectOption v-if="otgNetworkInterfaces.length === 0" value="">{{ t('settings.otgNetworkNone') }}</NativeSelectOption>
                                <NativeSelectOption
                                  v-for="item in otgNetworkInterfaces"
                                  :key="item.name"
                                  :value="item.name"
                                >
                                  {{ item.name }} · {{ item.interface_type }}
                                </NativeSelectOption>
                              </NativeSelect>
                            </div>
                          </div>
                        </template>
                        <p v-if="otgNetworkStatus?.health === 'degraded'" class="text-xs text-destructive">
                          {{ t('settings.otgRuntimeDegraded') }}: {{ otgNetworkStatus.error || t('common.error') }}
                        </p>
                      </div>
                    </div>
                    <p class="text-xs text-warning">
                      {{ t('settings.otgProfileWarning') }}
                    </p>
                  </div>
                </template>

              </CardContent>
            </Card>

          </div>

          <!-- Environment Section -->
          <div v-show="activeSection === 'environment'" class="space-y-4">
            <Card v-if="systemStore.deviceInfo">
              <CardHeader>
                <CardTitle>{{ t('settings.deviceInfo') }}</CardTitle>
                <CardDescription>{{ t('settings.deviceInfoDesc') }}</CardDescription>
              </CardHeader>
              <CardContent>
                <div class="space-y-3">
                  <div class="flex justify-between items-center py-2 border-b gap-2">
                    <span class="text-sm text-muted-foreground shrink-0">{{ t('settings.hostname') }}</span>
                    <span class="text-sm font-medium truncate">{{ systemStore.deviceInfo.hostname }}</span>
                  </div>
                  <div class="flex justify-between items-center py-2 border-b gap-2">
                    <span class="text-sm text-muted-foreground shrink-0">{{ t('settings.cpuModel') }}</span>
                    <span class="text-sm font-medium truncate max-w-[60%] text-right">{{ systemStore.deviceInfo.cpu_model }}</span>
                  </div>
                  <div v-if="hasDeviceCpuUsage" class="flex justify-between items-center py-2 border-b">
                    <span class="text-sm text-muted-foreground">{{ t('settings.cpuUsage') }}</span>
                    <span class="text-sm font-medium">{{ systemStore.deviceInfo.cpu_usage.toFixed(1) }}%</span>
                  </div>
                  <div v-if="hasDeviceMemoryUsage" class="flex justify-between items-center py-2 border-b">
                    <span class="text-sm text-muted-foreground">{{ t('settings.memoryUsage') }}</span>
                    <span class="text-sm font-medium">{{ formatBytes(systemStore.deviceInfo.memory_used) }} / {{ formatBytes(systemStore.deviceInfo.memory_total) }}</span>
                  </div>
                  <div v-if="hasDeviceNetworkAddresses" class="py-2">
                    <span class="text-sm text-muted-foreground">{{ t('settings.networkAddresses') }}</span>
                    <div class="mt-2 space-y-1">
                      <div v-for="addr in systemStore.deviceInfo.network_addresses" :key="addr.interface" class="flex justify-between items-center text-sm">
                        <span class="text-muted-foreground">{{ addr.interface }}</span>
                        <code class="font-mono bg-muted px-2 py-0.5 rounded">{{ addr.ip }}</code>
                      </div>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>

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
                  <RefreshCw class="size-4 mr-2" :class="{ 'animate-spin': otgSelfCheckLoading }" />
                  {{ t('settings.otgSelfCheck.run') }}
                </Button>
              </CardHeader>
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
                          <span class="inline-block size-2 rounded-full shrink-0" :class="otgGroupStatusClass(group.status)" />
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
                            <span class="inline-block size-2 rounded-full mt-1.5 shrink-0" :class="otgCheckLevelClass(item.level)" />
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

            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.encoderSelfCheck.title') }}</CardTitle>
                  <CardDescription>{{ t('settings.encoderSelfCheck.desc') }}</CardDescription>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="videoEncoderSelfCheckLoading"
                  :class="[
                    'transition-all duration-150 active:scale-95 active:brightness-95',
                    videoEncoderRunButtonPressed ? 'scale-95 brightness-95' : ''
                  ]"
                  @click="onRunVideoEncoderSelfCheckClick"
                >
                  <RefreshCw class="size-4 mr-2" :class="{ 'animate-spin': videoEncoderSelfCheckLoading }" />
                  {{ t('settings.encoderSelfCheck.run') }}
                </Button>
              </CardHeader>
              <CardContent class="space-y-3">
                <p v-if="videoEncoderSelfCheckError" class="text-xs text-destructive">
                  {{ videoEncoderSelfCheckError }}
                </p>

                <template v-if="videoEncoderSelfCheckResult">
                  <div class="text-sm">
                    {{ t('settings.encoderSelfCheck.currentHardwareEncoder') }}：{{ currentHardwareEncoderText }}
                  </div>

                  <div class="rounded-md border bg-card">
                    <Table class="table-fixed">
                      <TableHeader>
                        <TableRow>
                          <TableHead class="w-[18%] px-2">{{ t('settings.encoderSelfCheck.resolution') }}</TableHead>
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
            <Card>
              <CardHeader>
                <div class="flex items-start justify-between gap-4">
                  <div class="min-w-0 space-y-1">
                    <CardTitle>{{ t('settings.watchdog.title') }}</CardTitle>
                    <CardDescription>{{ t('settings.watchdog.description') }}</CardDescription>
                  </div>
                  <Switch
                    :aria-label="t('settings.watchdog.title')"
                    :model-value="watchdogStatus?.enabled ?? false"
                    :disabled="watchdogLoading || !watchdogStatus || (!watchdogStatus.supported && !watchdogStatus.enabled)"
                    @update:model-value="updateWatchdog"
                  />
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center gap-2 text-sm">
                  <Loader2 v-if="watchdogLoading" class="size-4 animate-spin text-muted-foreground" />
                  <span v-else class="size-2.5 rounded-full" :class="watchdogStatusClass" />
                  <span class="font-medium">
                    {{ watchdogLoading
                      ? t('common.loading')
                      : t(`settings.watchdog.status.${watchdogStatusKey}`) }}
                  </span>
                </div>
                <p
                  v-if="watchdogError || watchdogDisplayReason"
                  class="text-sm"
                  :class="watchdogStatusKey === 'error' || watchdogError ? 'text-destructive' : 'text-muted-foreground'"
                >
                  {{ watchdogError || watchdogDisplayReason }}
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.usbDevices.title') }}</CardTitle>
                  <CardDescription>{{ t('settings.usbDevices.desc') }}</CardDescription>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="usbDevicesLoading"
                  @click="fetchUsbDevices()"
                >
                  <RefreshCw class="size-4 mr-2" :class="{ 'animate-spin': usbDevicesLoading }" />
                  {{ t('settings.usbDevices.refresh') }}
                </Button>
              </CardHeader>
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
                          <TableHead class="text-right">{{ t('settings.usbDevices.colAction') }}</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        <TableRow
                          v-for="dev in usbDevices"
                          :key="`${dev.bus_num}-${dev.dev_num}`"
                        >
                          <TableCell>
                            <div class="font-medium truncate max-w-[180px]" :title="dev.product || dev.manufacturer || `${dev.id_vendor}:${dev.id_product}`">{{ dev.product || dev.manufacturer || `${dev.id_vendor}:${dev.id_product}` }}</div>
                          </TableCell>
                          <TableCell class="font-mono text-xs">{{ dev.id_vendor }}:{{ dev.id_product }}</TableCell>
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
            <AlertDialog :open="usbResetTarget != null">
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>{{ t('settings.usbDevices.resetConfirmTitle') }}</AlertDialogTitle>
                  <AlertDialogDescription>
                    {{ t('settings.usbDevices.resetConfirmDesc', { device: usbResetTarget?.product || `${usbResetTarget?.id_vendor}:${usbResetTarget?.id_product}` }) }}
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel @click="usbResetTarget = null">{{ t('common.cancel') }}</AlertDialogCancel>
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
                  {{ webServerConfig.https_enabled
                    ? t('settings.autoRestartingHttpsDesc', { sec: autoRestartCountdown })
                    : t('settings.autoRestartingDesc') }}
                </p>
              </div>
              <span v-if="webServerConfig.https_enabled && autoRestartCountdown > 0"
                class="tabular-nums text-lg font-bold text-primary shrink-0">
                {{ autoRestartCountdown }}
              </span>
            </div>

            <!-- Auto-restart: HTTPS manual redirect (cert must be accepted by user) -->
            <Alert v-if="autoRestartManualUrl" variant="warning">
              <Lock />
              <AlertTitle>{{ t('settings.httpsManualRedirectTitle') }}</AlertTitle>
              <AlertDescription>{{ t('settings.httpsManualRedirectDesc') }}</AlertDescription>
              <Button as-child class="col-start-2 mt-2 w-full bg-warning text-warning-foreground hover:bg-warning/90">
                <a :href="autoRestartManualUrl"><ExternalLink />{{ autoRestartManualUrl }}</a>
              </Button>
            </Alert>

            <!-- Auto-restart: failure / timeout -->
            <Alert v-if="autoRestartFailed" variant="destructive">
              <AlertTriangle />
              <AlertDescription class="flex items-center justify-between gap-3">
              <span>{{ t('settings.autoRestartFailed') }}</span>
              <Button variant="outline" size="sm" class="text-foreground" @click="triggerAutoRestart">
                <RefreshCw class="size-3 mr-1" />
                {{ t('common.retry') }}
              </Button>
              </AlertDescription>
            </Alert>

            <!-- Port Configuration Card -->
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.portConfig') }}</CardTitle>
                <CardDescription>{{ t('settings.portConfigDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-5">
                <!-- HTTPS toggle -->
                <div class="flex items-start justify-between gap-4">
                  <Label>{{ t('settings.httpsEnabled') }}</Label>
                  <Switch v-model="webServerConfig.https_enabled" />
                </div>

                <Separator />

                <!-- Active port (primary) -->
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <div class="flex items-center gap-2">
                      <Label class="text-sm font-medium">
                        {{ webServerConfig.https_enabled ? t('settings.httpsPort') : t('settings.httpPort') }}
                      </Label>
                      <Badge variant="default" class="h-4 text-[10px] px-1.5">{{ t('settings.portActive') }}</Badge>
                    </div>
                    <Input
                      v-if="webServerConfig.https_enabled"
                      v-model.number="webServerConfig.https_port"
                      type="number" min="1" max="65535"
                    />
                    <Input
                      v-else
                      v-model.number="webServerConfig.http_port"
                      type="number" min="1" max="65535"
                    />
                  </div>
                  <div class="space-y-2">
                    <div class="flex items-center gap-2">
                      <Label class="text-sm text-muted-foreground">
                        {{ webServerConfig.https_enabled ? t('settings.httpPort') : t('settings.httpsPort') }}
                      </Label>
                      <Badge variant="secondary" class="h-4 text-[10px] px-1.5 font-normal">{{ t('settings.portReserved') }}</Badge>
                    </div>
                    <Input
                      v-if="webServerConfig.https_enabled"
                      v-model.number="webServerConfig.http_port"
                      type="number" min="1" max="65535"
                      class="opacity-60"
                    />
                    <Input
                      v-else
                      v-model.number="webServerConfig.https_port"
                      type="number" min="1" max="65535"
                      class="opacity-60"
                    />
                  </div>
                </div>
                <!-- Preview URL -->
                <div class="rounded-md border bg-muted/40 p-3 space-y-1.5">
                  <p class="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">{{ t('settings.previewUrl') }}</p>
                  <div class="flex items-center gap-2">
                    <code class="font-mono text-xs sm:text-sm break-all flex-1 min-w-0">{{ previewAccessUrl }}</code>
                    <Button
                      variant="ghost" size="icon-sm" class="shrink-0"
                      :title="t('settings.copyUrl')"
                      :aria-label="t('settings.copyUrl')"
                      @click="copyPreviewUrl"
                    >
                      <Check v-if="previewUrlCopied" class="size-3.5 text-success" />
                      <Copy v-else class="size-3.5" />
                    </Button>
                    <Button
                      variant="ghost" size="icon-sm" class="shrink-0"
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
                <Button @click="saveWebServerConfig" :disabled="webServerLoading || autoRestarting">
                  <RefreshCw v-if="autoRestarting" class="size-4 mr-2 animate-spin" />
                  <Save v-else class="size-4 mr-2" />
                  {{ autoRestarting ? t('settings.restarting') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>

            <!-- Listen Address Card -->
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.listenAddress') }}</CardTitle>
                <CardDescription>{{ t('settings.listenAddressDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <RadioGroup v-model="bindMode" class="space-y-3">
                  <!-- All addresses -->
                  <div class="space-y-2">
                    <div class="flex items-start gap-3">
                      <RadioGroupItem value="all" id="bind-all" class="mt-0.5" />
                      <div class="flex-1">
                        <Label for="bind-all" class="cursor-pointer">{{ t('settings.bindModeAll') }}</Label>
                      </div>
                    </div>
                    <div v-if="bindMode === 'all'" class="ml-7 flex items-center justify-between rounded-md border border-dashed px-3 py-2">
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
                        <Label for="bind-loopback" class="cursor-pointer">{{ t('settings.bindModeLocal') }}</Label>
                      </div>
                    </div>
                    <div v-if="bindMode === 'loopback'" class="ml-7 flex items-center justify-between rounded-md border border-dashed px-3 py-2">
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
                        <Label for="bind-custom" class="cursor-pointer">{{ t('settings.bindModeCustom') }}</Label>
                      </div>
                    </div>
                    <div v-if="bindMode === 'custom'" class="ml-7 space-y-2">
                      <div v-for="(_, i) in bindAddressList" :key="`bind-${i}`" class="flex gap-2">
                        <Input v-model="bindAddressList[i]" placeholder="192.168.1.10" />
                        <Button variant="ghost" size="icon" :aria-label="t('common.delete')" @click="removeBindAddress(i)">
                          <Trash2 class="size-4" />
                        </Button>
                      </div>
                      <Button variant="outline" size="sm" @click="addBindAddress">
                        <Plus class="size-4 mr-1" />
                        {{ t('settings.addBindAddress') }}
                      </Button>
                      <p v-if="bindAddressError" class="text-xs text-destructive">{{ bindAddressError }}</p>
                    </div>
                  </div>
                </RadioGroup>

                <!-- Effective addresses preview -->
                <div v-if="effectiveBindAddresses.length > 0" class="rounded-md border bg-muted/40 p-3 space-y-1.5">
                  <p class="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">{{ t('settings.effectiveAddresses') }}</p>
                  <code class="font-mono text-xs sm:text-sm break-all block">{{ effectiveBindAddresses.join(', ') }}</code>
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <p class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                  <AlertTriangle class="size-3.5 shrink-0 text-warning" />
                  <span class="truncate">{{ t('settings.restartRequiredHint') }}</span>
                </p>
                <Button @click="saveWebServerConfig" :disabled="webServerLoading || !!bindAddressError || autoRestarting">
                  <RefreshCw v-if="autoRestarting" class="size-4 mr-2 animate-spin" />
                  <Save v-else class="size-4 mr-2" />
                  {{ autoRestarting ? t('settings.restarting') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>

            <!-- SSL Certificate Card -->
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0 pb-3">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.sslCertificate') }}</CardTitle>
                  <CardDescription>{{ t('settings.sslCertificateDesc') }}</CardDescription>
                </div>
                <Badge :variant="webServerConfig.has_custom_cert ? 'default' : 'secondary'" class="mt-1 shrink-0">
                  {{ webServerConfig.has_custom_cert ? t('settings.sslCertCustom') : t('settings.sslCertSelfSigned') }}
                </Badge>
              </CardHeader>
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
                    <RefreshCw v-if="certClearing || autoRestarting" class="size-3 mr-1 animate-spin" />
                    <Trash2 v-else class="size-3 mr-1" />
                    {{ autoRestarting ? t('settings.restarting') : t('settings.sslCertClear') }}
                  </Button>
                  </AlertDescription>
                </Alert>

                <!-- Certificate textarea -->
                <div class="space-y-2">
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
                <div class="space-y-2">
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
                <Button
                  :disabled="certSaving || autoRestarting || !sslCertPem.trim() || !sslKeyPem.trim()"
                  @click="saveCertificate"
                >
                  <RefreshCw v-if="certSaving || autoRestarting" class="size-4 mr-2 animate-spin" />
                  <Save v-else class="size-4 mr-2" />
                  {{ autoRestarting ? t('settings.restarting') : t('settings.sslCertSave') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- ATX Section -->
          <div v-show="activeSection === 'atx'" class="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.atxPowerManagement') }}</CardTitle>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between gap-3">
                  <div class="flex min-w-0 flex-1 items-center gap-3">
                    <Label for="atx-driver" class="shrink-0">{{ t('settings.atxDriver') }}</Label>
                    <Select v-model="atxConfig.driver">
                      <SelectTrigger id="atx-driver" class="w-full max-w-xs"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem v-for="option in atxDriverOptions" :key="option.value" :value="option.value">{{ option.label }}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <Button variant="ghost" size="icon-sm" class="shrink-0" :aria-label="t('common.refresh')" @click="loadAtxDevices">
                    <RefreshCw class="size-4" />
                  </Button>
                </div>

                <div v-if="['usbrelay', 'serial'].includes(atxConfig.driver)" class="grid gap-4 sm:grid-cols-2">
                  <div v-if="['usbrelay', 'serial'].includes(atxConfig.driver)" class="space-y-2">
                    <Label for="atx-device">{{ t('settings.atxDevice') }}</Label>
                    <NativeSelect id="atx-device" v-model="atxConfig.device" class="w-full">
                      <NativeSelectOption value="">{{ t('settings.selectDevice') }}</NativeSelectOption>
                      <NativeSelectOption
                        v-for="dev in getAtxDevicesForDriver(atxConfig.driver)"
                        :key="dev"
                        :value="dev"
                        :disabled="atxConfig.driver === 'serial' && isAtxSerialDeviceReserved(dev)"
                      >
                        {{ formatAtxDeviceLabel(atxConfig.driver, dev) }}
                      </NativeSelectOption>
                    </NativeSelect>
                  </div>
                  <div v-if="atxConfig.driver === 'serial'" class="space-y-2">
                    <Label for="atx-baudrate">{{ t('settings.baudRate') }}</Label>
                    <Select :model-value="atxConfig.baud_rate" @update:model-value="value => atxConfig.baud_rate = Number(value)">
                      <SelectTrigger id="atx-baudrate" class="w-full"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem v-for="baud in [9600, 19200, 38400, 57600, 115200]" :key="baud" :value="baud">{{ baud }}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <template v-if="atxConfig.driver === 'gpio'">
                  <Separator />
                  <div class="space-y-4">
                    <div class="space-y-3 rounded-md border p-3">
                      <Label>{{ t('settings.atxPowerButton') }}</Label>
                      <div class="grid gap-3 sm:grid-cols-3">
                        <div class="space-y-2">
                          <Label for="power-device">{{ t('settings.atxGpioChip') }}</Label>
                          <NativeSelect id="power-device" v-model="atxConfig.power.device" class="w-full">
                            <NativeSelectOption value="">{{ t('settings.atxDriverNone') }}</NativeSelectOption>
                            <NativeSelectOption v-for="dev in atxDevices.gpio_chips" :key="dev" :value="dev">{{ dev }}</NativeSelectOption>
                          </NativeSelect>
                        </div>
                        <div class="space-y-2">
                          <Label for="power-pin">{{ t('settings.atxPin') }}</Label>
                          <Input id="power-pin" type="number" v-model.number="atxConfig.power.pin" min="0" :disabled="!atxConfig.power.device" />
                        </div>
                        <div class="space-y-2">
                          <Label for="power-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select v-model="atxConfig.power.active_level" :disabled="!atxConfig.power.device">
                            <SelectTrigger id="power-active-level" class="w-full"><SelectValue /></SelectTrigger>
                            <SelectContent><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem><SelectItem value="low">{{ t('settings.atxLevelLow') }}</SelectItem></SelectContent>
                          </Select>
                        </div>
                      </div>
                    </div>

                    <div class="space-y-3 rounded-md border p-3">
                      <Label>{{ t('settings.atxResetButton') }}</Label>
                      <div class="grid gap-3 sm:grid-cols-3">
                        <div class="space-y-2">
                          <Label for="reset-device">{{ t('settings.atxGpioChip') }}</Label>
                          <NativeSelect id="reset-device" v-model="atxConfig.reset.device" class="w-full">
                            <NativeSelectOption value="">{{ t('settings.atxDriverNone') }}</NativeSelectOption>
                            <NativeSelectOption v-for="dev in atxDevices.gpio_chips" :key="dev" :value="dev">{{ dev }}</NativeSelectOption>
                          </NativeSelect>
                        </div>
                        <div class="space-y-2">
                          <Label for="reset-pin">{{ t('settings.atxPin') }}</Label>
                          <Input id="reset-pin" type="number" v-model.number="atxConfig.reset.pin" min="0" :disabled="!atxConfig.reset.device" />
                        </div>
                        <div class="space-y-2">
                          <Label for="reset-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select v-model="atxConfig.reset.active_level" :disabled="!atxConfig.reset.device">
                            <SelectTrigger id="reset-active-level" class="w-full"><SelectValue /></SelectTrigger>
                            <SelectContent><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem><SelectItem value="low">{{ t('settings.atxLevelLow') }}</SelectItem></SelectContent>
                          </Select>
                        </div>
                      </div>
                    </div>

                    <div class="space-y-3 rounded-md border p-3">
                      <Label>{{ t('settings.atxLedSensing') }}</Label>
                      <div class="grid gap-3 sm:grid-cols-3">
                        <div class="space-y-2">
                          <Label for="led-device">{{ t('settings.atxGpioChip') }}</Label>
                          <NativeSelect id="led-device" v-model="atxConfig.led.device" class="w-full">
                            <NativeSelectOption value="">{{ t('settings.atxDriverNone') }}</NativeSelectOption>
                            <NativeSelectOption v-for="dev in atxDevices.gpio_chips" :key="dev" :value="dev">{{ dev }}</NativeSelectOption>
                          </NativeSelect>
                        </div>
                        <div class="space-y-2">
                          <Label for="led-pin">{{ t('settings.atxPin') }}</Label>
                          <Input id="led-pin" type="number" v-model.number="atxConfig.led.pin" min="0" :disabled="!atxConfig.led.device" />
                        </div>
                        <div class="space-y-2">
                          <Label for="led-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select v-model="atxConfig.led.active_level" :disabled="!atxConfig.led.device">
                            <SelectTrigger id="led-active-level" class="w-full"><SelectValue /></SelectTrigger>
                            <SelectContent><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem><SelectItem value="low">{{ t('settings.atxLevelLow') }}</SelectItem></SelectContent>
                          </Select>
                        </div>
                      </div>
                    </div>

                    <div class="space-y-3 rounded-md border p-3">
                      <Label>{{ t('settings.atxHddSensing') }}</Label>
                      <div class="grid gap-3 sm:grid-cols-3">
                        <div class="space-y-2">
                          <Label for="hdd-device">{{ t('settings.atxGpioChip') }}</Label>
                          <NativeSelect id="hdd-device" v-model="atxConfig.hdd.device" class="w-full">
                            <NativeSelectOption value="">{{ t('settings.atxDriverNone') }}</NativeSelectOption>
                            <NativeSelectOption v-for="dev in atxDevices.gpio_chips" :key="dev" :value="dev">{{ dev }}</NativeSelectOption>
                          </NativeSelect>
                        </div>
                        <div class="space-y-2">
                          <Label for="hdd-pin">{{ t('settings.atxPin') }}</Label>
                          <Input id="hdd-pin" type="number" v-model.number="atxConfig.hdd.pin" min="0" :disabled="!atxConfig.hdd.device" />
                        </div>
                        <div class="space-y-2">
                          <Label for="hdd-active-level">{{ t('settings.atxActiveLevel') }}</Label>
                          <Select v-model="atxConfig.hdd.active_level" :disabled="!atxConfig.hdd.device">
                            <SelectTrigger id="hdd-active-level" class="w-full"><SelectValue /></SelectTrigger>
                            <SelectContent><SelectItem value="high">{{ t('settings.atxLevelHigh') }}</SelectItem><SelectItem value="low">{{ t('settings.atxLevelLow') }}</SelectItem></SelectContent>
                          </Select>
                        </div>
                      </div>
                    </div>
                  </div>
                </template>

                <template v-else-if="['usbrelay', 'serial'].includes(atxConfig.driver)">
                  <Separator />
                  <div class="space-y-4">
                    <div class="space-y-3 rounded-md border p-3">
                      <Label>{{ t('settings.atxPowerButton') }}</Label>
                      <div class="space-y-2">
                        <Label for="power-channel">{{ t('settings.atxChannel') }}</Label>
                        <Input id="power-channel" type="number" v-model.number="atxConfig.power.pin" min="1" />
                      </div>
                    </div>

                    <div class="space-y-3 rounded-md border p-3">
                      <Label>{{ t('settings.atxResetButton') }}</Label>
                      <div class="space-y-2">
                        <Label for="reset-channel">{{ t('settings.atxChannel') }}</Label>
                        <Input id="reset-channel" type="number" v-model.number="atxConfig.reset.pin" min="1" />
                      </div>
                    </div>
                  </div>
                </template>
              </CardContent>
              <CardFooter class="border-t pt-4 justify-end">
                <Button :disabled="atxSaving" @click="saveAtxSettings">
                  <Loader2 v-if="atxSaving" class="size-4 mr-2 animate-spin" /><Check v-else-if="atxSaved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ atxSaving ? t('actionbar.applying') : atxSaved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>

            <!-- WOL Config -->
            <Card>
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
                </div>
              </CardContent>
              <CardFooter class="border-t pt-4 justify-end">
                <Button :disabled="wolSaving" @click="saveWolSettings">
                  <Loader2 v-if="wolSaving" class="size-4 mr-2 animate-spin" /><Check v-else-if="wolSaved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ wolSaving ? t('actionbar.applying') : wolSaved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- ttyd Section -->
          <div v-show="activeSection === 'ext-ttyd'" class="space-y-4">
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
                  {{ t('extensions.binaryNotFound', { path: isWindows ? 'ttyd.win32.exe' : '/usr/bin/ttyd' }) }}
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
                        <Terminal class="size-4 mr-1" />
                        {{ t('extensions.ttyd.open') }}
                      </Button>
                      <Button
                        v-if="!isExtRunning(extensions?.ttyd?.status)"
                        size="sm"
                        @click="startExtension('ttyd')"
                        :disabled="extensionsLoading"
                      >
                        <Play class="size-4 mr-1" />
                        {{ t('extensions.start') }}
                      </Button>
                      <Button
                        v-else
                        size="sm"
                        variant="outline"
                        @click="stopExtension('ttyd')"
                        :disabled="extensionsLoading"
                      >
                        <Square class="size-4 mr-1" />
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
                      <Input v-model="extConfig.ttyd.shell" class="sm:col-span-3" :placeholder="isWindows ? 'cmd' : '/bin/bash'" :disabled="isExtRunning(extensions?.ttyd?.status)" />
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <Collapsible v-model:open="showLogs.ttyd" @update:open="open => open && refreshExtensionLogs('ttyd')">
                      <CollapsibleTrigger as-child><Button type="button" variant="ghost" size="sm" class="gap-2 text-muted-foreground"><ChevronRight :class="['size-4 transition-transform', showLogs.ttyd ? 'rotate-90' : '']" />{{ t('extensions.viewLogs') }}</Button></CollapsibleTrigger>
                      <CollapsibleContent class="space-y-2 pt-2">
                      <pre class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono">{{ (extensionLogs.ttyd || []).join('\n') || t('extensions.noLogs') }}</pre>
                      <Button variant="ghost" size="sm" @click="refreshExtensionLogs('ttyd')">
                        <RefreshCw class="size-3 mr-1" />
                        {{ t('common.refresh') }}
                      </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
              <CardFooter v-if="extensions?.ttyd?.available" class="border-t pt-4 justify-end">
                <Button :disabled="loading || isExtRunning(extensions?.ttyd?.status)" @click="saveExtensionConfig('ttyd')">
                  <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- Remote Access Section -->
          <div v-show="activeSection === 'ext-remote-access'" class="space-y-4">
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
                  {{ t('extensions.binaryNotFound', { path: isWindows ? 'gostc.exe' : '/usr/bin/gostc' }) }}
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
                        :disabled="extensionsLoading || !!gostcValidationMessage"
                      >
                        <Play class="size-4 mr-1" />
                        {{ t('extensions.start') }}
                      </Button>
                      <Button
                        v-else
                        size="sm"
                        variant="outline"
                        @click="stopExtension('gostc')"
                        :disabled="extensionsLoading"
                      >
                        <Square class="size-4 mr-1" />
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
                      <div class="sm:col-span-3 space-y-1">
                        <Input v-model="extConfig.gostc.addr" :placeholder="t('extensions.gostc.addrPlaceholder')" :disabled="isExtRunning(extensions?.gostc?.status)" />
                        <p v-if="extConfig.gostc.enabled && !extConfig.gostc.addr?.trim()" class="text-xs text-destructive">{{ t('extensions.gostc.addrRequired') }}</p>
                      </div>
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.gostc.key') }}</Label>
                      <div class="sm:col-span-3 space-y-1">
                        <Input v-model="extConfig.gostc.key" type="password" autocomplete="off" :disabled="isExtRunning(extensions?.gostc?.status)" />
                        <p v-if="extConfig.gostc.enabled && !extConfig.gostc.key" class="text-xs text-destructive">{{ t('extensions.gostc.keyRequired') }}</p>
                      </div>
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
                    <Collapsible v-model:open="showLogs.gostc" @update:open="open => open && refreshExtensionLogs('gostc')">
                      <CollapsibleTrigger as-child><Button type="button" variant="ghost" size="sm" class="gap-2 text-muted-foreground"><ChevronRight :class="['size-4 transition-transform', showLogs.gostc ? 'rotate-90' : '']" />{{ t('extensions.viewLogs') }}</Button></CollapsibleTrigger>
                      <CollapsibleContent class="space-y-2 pt-2">
                      <pre class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono">{{ (extensionLogs.gostc || []).join('\n') || t('extensions.noLogs') }}</pre>
                      <Button variant="ghost" size="sm" @click="refreshExtensionLogs('gostc')">
                        <RefreshCw class="size-3 mr-1" />
                        {{ t('common.refresh') }}
                      </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
              <CardFooter v-if="extensions?.gostc?.available" class="border-t pt-4 justify-end">
                <Button :disabled="loading || isExtRunning(extensions?.gostc?.status)" @click="saveExtensionConfig('gostc')">
                  <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>

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
                  {{ t('extensions.binaryNotFound', { path: isWindows ? 'easytier-core.exe' : '/usr/bin/easytier-core' }) }}
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
                        :disabled="extensionsLoading || !!easytierValidationMessage"
                      >
                        <Play class="size-4 mr-1" />
                        {{ t('extensions.start') }}
                      </Button>
                      <Button
                        v-else
                        size="sm"
                        variant="outline"
                        @click="stopExtension('easytier')"
                        :disabled="extensionsLoading"
                      >
                        <Square class="size-4 mr-1" />
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
                      <div class="sm:col-span-3 space-y-1">
                        <Input v-model="extConfig.easytier.network_name" :disabled="isExtRunning(extensions?.easytier?.status)" />
                        <p v-if="extConfig.easytier.enabled && !extConfig.easytier.network_name?.trim()" class="text-xs text-destructive">{{ t('extensions.easytier.networkNameRequired') }}</p>
                      </div>
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easytier.networkSecret') }}</Label>
                      <Input v-model="extConfig.easytier.network_secret" type="password" autocomplete="off" class="sm:col-span-3" :disabled="isExtRunning(extensions?.easytier?.status)" />
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easytier.peers') }}</Label>
                      <div class="sm:col-span-3 space-y-2">
                        <div v-for="(_, i) in extConfig.easytier.peer_urls" :key="i" class="flex gap-2">
                          <Input v-model="extConfig.easytier.peer_urls[i]" placeholder="tcp://1.2.3.4:11010" :disabled="isExtRunning(extensions?.easytier?.status)" />
                          <Button variant="ghost" size="icon" :aria-label="t('common.delete')" @click="removeEasytierPeer(i)" :disabled="isExtRunning(extensions?.easytier?.status)">
                            <Trash2 class="size-4" />
                          </Button>
                        </div>
                        <Button variant="outline" size="sm" @click="addEasytierPeer" :disabled="isExtRunning(extensions?.easytier?.status)">
                          <Plus class="size-4 mr-1" />
                          {{ t('extensions.easytier.addPeer') }}
                        </Button>
                      </div>
                    </div>
                    <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                      <Label class="sm:text-right">{{ t('extensions.easytier.virtualIp') }}</Label>
                      <div class="sm:col-span-3 space-y-1">
                        <Input v-model="extConfig.easytier.virtual_ip" placeholder="10.0.0.1/24" :disabled="isExtRunning(extensions?.easytier?.status)" />
                      </div>
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <Collapsible v-model:open="showLogs.easytier" @update:open="open => open && refreshExtensionLogs('easytier')">
                      <CollapsibleTrigger as-child><Button type="button" variant="ghost" size="sm" class="gap-2 text-muted-foreground"><ChevronRight :class="['size-4 transition-transform', showLogs.easytier ? 'rotate-90' : '']" />{{ t('extensions.viewLogs') }}</Button></CollapsibleTrigger>
                      <CollapsibleContent class="space-y-2 pt-2">
                      <pre class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono">{{ (extensionLogs.easytier || []).join('\n') || t('extensions.noLogs') }}</pre>
                      <Button variant="ghost" size="sm" @click="refreshExtensionLogs('easytier')">
                        <RefreshCw class="size-3 mr-1" />
                        {{ t('common.refresh') }}
                      </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
              <CardFooter v-if="extensions?.easytier?.available" class="border-t pt-4 justify-end">
                <Button :disabled="loading || isExtRunning(extensions?.easytier?.status)" @click="saveExtensionConfig('easytier')">
                  <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
            <!-- FRPC -->
            <Card>
              <CardHeader>
                <div class="flex items-center justify-between gap-4">
                  <div class="space-y-1.5">
                    <CardTitle>{{ t('extensions.frpc.title') }}</CardTitle>
                    <CardDescription>{{ t('extensions.frpc.desc') }}</CardDescription>
                  </div>
                  <Badge :variant="extensions?.frpc?.available ? 'default' : 'destructive'">
                    {{ extensions?.frpc?.available ? t('extensions.available') : t('extensions.unavailable') }}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div v-if="!extensions?.frpc?.available" class="text-sm text-muted-foreground bg-muted p-3 rounded-md">
                  {{ t('extensions.binaryNotFound', { path: isWindows ? 'frpc.exe' : '/usr/bin/frpc' }) }}
                </div>
                <template v-else>
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <div :class="['w-2 h-2 rounded-full', getExtStatusClass(extensions?.frpc?.status)]" />
                      <span class="text-sm">{{ getExtStatusText(extensions?.frpc?.status) }}</span>
                    </div>
                    <div class="flex gap-2">
                      <Button
                        v-if="!isExtRunning(extensions?.frpc?.status)"
                        size="sm"
                        @click="startExtension('frpc')"
                        :disabled="extensionsLoading || !!frpcValidationMessage"
                      >
                        <Play class="size-4 mr-1" />
                        {{ t('extensions.start') }}
                      </Button>
                      <Button
                        v-else
                        size="sm"
                        variant="outline"
                        @click="stopExtension('frpc')"
                        :disabled="extensionsLoading"
                      >
                        <Square class="size-4 mr-1" />
                        {{ t('extensions.stop') }}
                      </Button>
                    </div>
                  </div>
                  <Separator />
                  <div class="grid gap-4">
                    <div class="flex items-center justify-between">
                      <Label>{{ t('extensions.autoStart') }}</Label>
                      <Switch v-model="extConfig.frpc.enabled" :disabled="isExtRunning(extensions?.frpc?.status)" />
                    </div>
                    <ButtonGroup class="grid w-full grid-cols-2">
                      <Button
                        type="button"
                        :variant="frpcQuickMode ? 'default' : 'outline'"
                        :disabled="isExtRunning(extensions?.frpc?.status)"
                        @click="extConfig.frpc.config_mode = FrpcConfigMode.Quick"
                      >
                        {{ t('extensions.frpc.quickConfig') }}
                      </Button>
                      <Button
                        type="button"
                        :variant="!frpcQuickMode ? 'default' : 'outline'"
                        :disabled="isExtRunning(extensions?.frpc?.status)"
                        @click="extConfig.frpc.config_mode = FrpcConfigMode.Full"
                      >
                        {{ t('extensions.frpc.fullConfig') }}
                      </Button>
                    </ButtonGroup>
                    <template v-if="frpcQuickMode">
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.proxyType') }}</Label>
                        <div class="sm:col-span-3">
                          <RadioGroup v-model="extConfig.frpc.proxy_type" class="flex flex-wrap gap-4" :disabled="isExtRunning(extensions?.frpc?.status)">
                            <div v-for="type in ['tcp', 'udp', 'http', 'https', 'stcp', 'sudp', 'xtcp']" :key="type" class="flex items-center space-x-2">
                              <RadioGroupItem :value="type" :id="`frpc-${type}`" />
                              <Label :for="`frpc-${type}`" class="cursor-pointer uppercase">{{ type }}</Label>
                            </div>
                          </RadioGroup>
                        </div>
                      </div>
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.proxyName') }}</Label>
                        <div class="sm:col-span-3 space-y-1">
                          <Input v-model="extConfig.frpc.proxy_name" :placeholder="t('extensions.frpc.proxyNamePlaceholder')" :disabled="isExtRunning(extensions?.frpc?.status)" />
                          <p v-if="extConfig.frpc.enabled && !extConfig.frpc.proxy_name?.trim()" class="text-xs text-destructive">{{ t('extensions.frpc.proxyNameRequired') }}</p>
                        </div>
                      </div>
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.serverAddr') }}</Label>
                        <div class="sm:col-span-3 space-y-1">
                          <Input v-model="extConfig.frpc.server_addr" :placeholder="t('extensions.frpc.serverAddrPlaceholder')" :disabled="isExtRunning(extensions?.frpc?.status)" />
                          <p v-if="extConfig.frpc.enabled && !extConfig.frpc.server_addr?.trim()" class="text-xs text-destructive">{{ t('extensions.frpc.serverAddrRequired') }}</p>
                        </div>
                      </div>
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.serverPort') }}</Label>
                        <Input v-model.number="extConfig.frpc.server_port" class="sm:col-span-3" type="number" min="1" max="65535" :disabled="isExtRunning(extensions?.frpc?.status)" />
                      </div>
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.token') }}</Label>
                        <div class="sm:col-span-3 space-y-1">
                          <Input v-model="extConfig.frpc.token" type="password" autocomplete="off" :disabled="isExtRunning(extensions?.frpc?.status)" />
                          <p v-if="extConfig.frpc.enabled && !extConfig.frpc.token" class="text-xs text-destructive">{{ t('extensions.frpc.tokenRequired') }}</p>
                        </div>
                      </div>
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.localIp') }}</Label>
                        <div class="sm:col-span-3 space-y-1">
                          <Input v-model="extConfig.frpc.local_ip" placeholder="127.0.0.1" :disabled="isExtRunning(extensions?.frpc?.status)" />
                          <p v-if="extConfig.frpc.enabled && !extConfig.frpc.local_ip?.trim()" class="text-xs text-destructive">{{ t('extensions.frpc.localIpRequired') }}</p>
                        </div>
                      </div>
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.localPort') }}</Label>
                        <Input v-model.number="extConfig.frpc.local_port" class="sm:col-span-3" type="number" min="1" max="65535" :disabled="isExtRunning(extensions?.frpc?.status)" />
                      </div>
                      <div v-if="showFrpcRemotePort" class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.remotePort') }}</Label>
                        <div class="sm:col-span-3 space-y-1">
                          <Input v-model.number="extConfig.frpc.remote_port" type="number" min="1" max="65535" :disabled="isExtRunning(extensions?.frpc?.status)" />
                          <p v-if="extConfig.frpc.enabled && frpcRemotePortRequired && !extConfig.frpc.remote_port" class="text-xs text-destructive">{{ t('extensions.frpc.remotePortRequired') }}</p>
                        </div>
                      </div>
                      <div v-if="showFrpcCustomDomain" class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.customDomain') }}</Label>
                        <Input v-model="extConfig.frpc.custom_domain" class="sm:col-span-3" :placeholder="t('extensions.frpc.customDomainPlaceholder')" :disabled="isExtRunning(extensions?.frpc?.status)" />
                      </div>
                      <div v-if="showFrpcSecretKey" class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.secretKey') }}</Label>
                        <Input v-model="extConfig.frpc.secret_key" class="sm:col-span-3" type="password" autocomplete="off" :disabled="isExtRunning(extensions?.frpc?.status)" />
                      </div>
                      <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                        <Label class="sm:text-right">{{ t('extensions.frpc.tls') }}</Label>
                        <div class="sm:col-span-3">
                          <Switch v-model="extConfig.frpc.tls" :disabled="isExtRunning(extensions?.frpc?.status)" />
                        </div>
                      </div>
                    </template>
                    <div v-else class="space-y-1">
                      <Textarea
                        v-model="extConfig.frpc.custom_toml"
                        class="min-h-[300px] font-mono text-xs"
                        spellcheck="false"
                        :disabled="isExtRunning(extensions?.frpc?.status)"
                      />
                      <p class="text-xs text-muted-foreground">{{ t('extensions.frpc.fullConfigHint') }}</p>
                      <p v-if="!extConfig.frpc.custom_toml?.trim()" class="text-xs text-destructive">{{ t('extensions.frpc.fullConfigRequired') }}</p>
                    </div>
                  </div>
                  <div class="space-y-2">
                    <Collapsible v-model:open="showLogs.frpc" @update:open="open => open && refreshExtensionLogs('frpc')">
                      <CollapsibleTrigger as-child><Button type="button" variant="ghost" size="sm" class="gap-2 text-muted-foreground"><ChevronRight :class="['size-4 transition-transform', showLogs.frpc ? 'rotate-90' : '']" />{{ t('extensions.viewLogs') }}</Button></CollapsibleTrigger>
                      <CollapsibleContent class="space-y-2 pt-2">
                      <pre class="p-3 bg-muted rounded-md text-xs max-h-48 overflow-auto font-mono">{{ (extensionLogs.frpc || []).join('\n') || t('extensions.noLogs') }}</pre>
                      <Button variant="ghost" size="sm" @click="refreshExtensionLogs('frpc')">
                        <RefreshCw class="size-3 mr-1" />
                        {{ t('common.refresh') }}
                      </Button>
                      </CollapsibleContent>
                    </Collapsible>
                  </div>
                </template>
              </CardContent>
              <CardFooter v-if="extensions?.frpc?.available" class="border-t pt-4 justify-end">
                <Button :disabled="loading || isExtRunning(extensions?.frpc?.status)" @click="saveExtensionConfig('frpc')">
                  <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- RTSP Section -->
          <div v-show="activeSection === 'third-party-access'" class="space-y-4">
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
                    <Button variant="ghost" size="icon-sm" :aria-label="t('common.refresh')" @click="loadRtspConfig" :disabled="rtspLoading">
                      <RefreshCw :class="['size-4', rtspLoading ? 'animate-spin' : '']" />
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
                      v-if="rtspStatus?.service_status !== 'running' && rtspStatus?.service_status !== 'starting'"
                      size="sm"
                      @click="startRtsp"
                      :disabled="rtspLoading || rtspStatus?.service_status === 'starting'"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      v-else
                      size="sm"
                      variant="outline"
                      @click="stopRtsp"
                      :disabled="rtspLoading"
                    >
                      <Square class="size-4 mr-1" />
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
                    <div class="sm:col-span-3 space-y-1">
                      <Input v-model="rtspLocalConfig.bind" placeholder="0.0.0.0 / ::" :disabled="rtspStatus?.service_status === 'running'" />
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.port') }}</Label>
                    <Input v-model.number="rtspLocalConfig.port" class="sm:col-span-3" type="number" min="1" max="65535" :disabled="rtspStatus?.service_status === 'running'" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.path') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input v-model="rtspLocalConfig.path" :placeholder="t('extensions.rtsp.pathPlaceholder')" :disabled="rtspStatus?.service_status === 'running'" />
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.codec') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Select v-model="rtspLocalConfig.codec" :disabled="rtspStatus?.service_status === 'running'">
                        <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                        <SelectContent><SelectItem value="h264">H.264</SelectItem><SelectItem value="h265">H.265</SelectItem></SelectContent>
                      </Select>
                    </div>
                  </div>
                  <div class="flex items-center justify-between">
                    <Label>{{ t('extensions.rtsp.allowOneClient') }}</Label>
                    <Switch v-model="rtspLocalConfig.allow_one_client" :disabled="rtspStatus?.service_status === 'running'" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rtsp.username') }}</Label>
                    <Input v-model="rtspLocalConfig.username" class="sm:col-span-3" :placeholder="t('extensions.rtsp.usernamePlaceholder')" :disabled="rtspStatus?.service_status === 'running'" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
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
                          :aria-label="showPasswords ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
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
                <Button class="shrink-0" :disabled="loading || rtspLoading" @click="saveRtspConfig">
                  <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- VNC Section -->
          <div v-show="activeSection === 'third-party-access'" class="space-y-4">
            <Card>
              <CardHeader>
                <div class="flex items-center justify-between">
                  <div class="space-y-1.5">
                    <CardTitle>{{ t('extensions.vnc.title') }}</CardTitle>
                    <CardDescription>{{ t('extensions.vnc.desc') }}</CardDescription>
                  </div>
                  <div class="flex items-center gap-2">
                    <Badge :variant="vncStatus?.service_status === 'running' ? 'default' : 'secondary'">
                      {{ getVncServiceStatusText(vncStatus?.service_status) }}
                    </Badge>
                    <Button variant="ghost" size="icon-sm" :aria-label="t('common.refresh')" @click="loadVncConfig" :disabled="vncLoading">
                      <RefreshCw :class="['size-4', vncLoading ? 'animate-spin' : '']" />
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-2">
                    <div :class="['w-2 h-2 rounded-full', getVncStatusClass(vncStatus?.service_status)]" />
                    <span class="text-sm">{{ getVncServiceStatusText(vncStatus?.service_status) }}</span>
                    <template v-if="vncStatus?.connection_count">
                      <span class="text-muted-foreground">|</span>
                      <span class="text-sm text-muted-foreground">{{ t('extensions.vnc.clients', { count: vncStatus.connection_count }) }}</span>
                    </template>
                  </div>
                  <div class="flex items-center gap-2">
                    <Button
                      v-if="vncStatus?.service_status !== 'running' && vncStatus?.service_status !== 'starting'"
                      size="sm"
                      @click="startVnc"
                      :disabled="vncLoading || vncStatus?.service_status === 'starting'"
                    >
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      v-else
                      size="sm"
                      variant="outline"
                      @click="stopVnc"
                      :disabled="vncLoading"
                    >
                      <Square class="size-4 mr-1" />
                      {{ t('extensions.stop') }}
                    </Button>
                  </div>
                </div>
                <Separator />

                <div class="grid gap-4">
                  <div class="flex items-center justify-between">
                    <Label>{{ t('extensions.autoStart') }}</Label>
                    <Switch v-model="vncLocalConfig.enabled" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.bind') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input v-model="vncLocalConfig.bind" placeholder="0.0.0.0 / ::" :disabled="vncStatus?.service_status === 'running'" />
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.port') }}</Label>
                    <Input v-model.number="vncLocalConfig.port" class="sm:col-span-3" type="number" min="1" max="65535" :disabled="vncStatus?.service_status === 'running'" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.encoding') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Select v-model="vncLocalConfig.encoding" :disabled="vncStatus?.service_status === 'running'">
                        <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                        <SelectContent><SelectItem value="tight_jpeg">{{ t('extensions.vnc.encodingTightJpeg') }}</SelectItem><SelectItem value="h264">{{ t('extensions.vnc.encodingH264') }}</SelectItem></SelectContent>
                      </Select>
                    </div>
                  </div>
                  <div class="flex items-center justify-between">
                    <Label>{{ t('extensions.vnc.allowOneClient') }}</Label>
                    <Switch v-model="vncLocalConfig.allow_one_client" :disabled="vncStatus?.service_status === 'running'" />
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.vnc.password') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <div class="relative">
                        <Input
                          v-model="vncLocalConfig.password"
                          :type="showPasswords ? 'text' : 'password'"
                          maxlength="8"
                          autocomplete="off"
                          :placeholder="vncStatus?.config.has_password ? t('extensions.vnc.passwordPlaceholder') : t('extensions.vnc.passwordRequiredPlaceholder')"
                          :disabled="vncStatus?.service_status === 'running'"
                        />
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon-sm"
                          class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground"
                          :aria-label="showPasswords ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
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
                <Button class="shrink-0" :disabled="loading || vncLoading" @click="saveVncConfig">
                  <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- RustDesk Section -->
          <div v-show="activeSection === 'third-party-access'" class="space-y-4">
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
                    <Button variant="ghost" size="icon-sm" :aria-label="t('common.refresh')" @click="loadRustdeskConfig" :disabled="rustdeskLoading">
                      <RefreshCw :class="['size-4', rustdeskLoading ? 'animate-spin' : '']" />
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
                      <Play class="size-4 mr-1" />
                      {{ t('extensions.start') }}
                    </Button>
                    <Button
                      v-else
                      size="sm"
                      variant="outline"
                      @click="stopRustdesk"
                      :disabled="rustdeskLoading"
                    >
                      <Square class="size-4 mr-1" />
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
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.codec') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Select v-model="rustdeskLocalConfig.codec" :disabled="rustdeskStatus?.service_status === 'running'">
                        <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                        <SelectContent><SelectItem value="h264">H.264</SelectItem><SelectItem value="h265">H.265</SelectItem></SelectContent>
                      </Select>
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.rendezvousServer') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rustdeskLocalConfig.rendezvous_server"
                        :placeholder="t('extensions.rustdesk.rendezvousServerPlaceholder')"
                        :disabled="rustdeskStatus?.service_status === 'running'"
                      />
                      <p v-if="rustdeskLocalConfig.enabled && rustdeskValidationMessage" class="text-xs text-destructive">{{ rustdeskValidationMessage }}</p>
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.relayServer') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <Input
                        v-model="rustdeskLocalConfig.relay_server"
                        :placeholder="t('extensions.rustdesk.relayServerPlaceholder')"
                        :disabled="!rustdeskLocalConfig.rendezvous_server || rustdeskStatus?.service_status === 'running'"
                      />
                    </div>
                  </div>
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.relayKey') }}</Label>
                    <div class="sm:col-span-3 space-y-1">
                      <div class="relative">
                        <Input
                          v-model="rustdeskLocalConfig.relay_key"
                          :type="showPasswords ? 'text' : 'password'"
                          :disabled="!rustdeskLocalConfig.rendezvous_server || rustdeskStatus?.service_status === 'running'"
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
                          :aria-label="showPasswords ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
                          @click="showPasswords = !showPasswords"
                        >
                          <Eye v-if="!showPasswords" class="size-4" />
                          <EyeOff v-else class="size-4" />
                        </Button>
                      </div>
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
                        class="size-8"
                        :aria-label="t('extensions.rustdesk.copyId')"
                        @click="copyToClipboard(rustdeskConfig?.device_id || '', 'id')"
                        :disabled="!rustdeskConfig?.device_id"
                      >
                        <Check v-if="rustdeskCopied === 'id'" class="size-4 text-success" />
                        <Copy v-else class="size-4" />
                      </Button>
                      <Button variant="outline" size="sm" @click="regenerateRustdeskId" :disabled="rustdeskLoading">
                        <RefreshCw class="size-4 mr-1" />
                        {{ t('extensions.rustdesk.regenerateId') }}
                      </Button>
                    </div>
                  </div>

                  <!-- Device Password (shown directly) -->
                  <div class="grid gap-2 sm:grid-cols-4 sm:items-center">
                    <Label class="sm:text-right">{{ t('extensions.rustdesk.devicePassword') }}</Label>
                    <div class="sm:col-span-3 flex items-center gap-2">
                      <code class="font-mono text-lg bg-muted px-3 py-1 rounded">{{ rustdeskPassword?.device_password || '-' }}</code>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="size-8"
                        :aria-label="t('extensions.rustdesk.copyPassword')"
                        @click="copyToClipboard(rustdeskPassword?.device_password || '', 'password')"
                        :disabled="!rustdeskPassword?.device_password"
                      >
                        <Check v-if="rustdeskCopied === 'password'" class="size-4 text-success" />
                        <Copy v-else class="size-4" />
                      </Button>
                      <Button variant="outline" size="sm" @click="regenerateRustdeskPassword" :disabled="rustdeskLoading">
                        <RefreshCw class="size-4 mr-1" />
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
              <CardFooter class="border-t pt-4 justify-end">
                <Button :disabled="loading || rustdeskLoading" @click="saveRustdeskConfig">
                  <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- Redfish Section -->
          <div v-show="activeSection === 'third-party-access'" class="space-y-4">
            <!-- Auto-restart: restarting progress -->
            <div
              v-if="autoRestarting"
              class="flex items-center gap-3 rounded-lg border bg-card px-4 py-3 text-sm shadow-sm"
            >
              <RefreshCw class="size-4 animate-spin text-primary shrink-0" />
              <div class="flex-1 min-w-0">
                <p class="font-medium">{{ t('settings.autoRestarting') }}</p>
                <p class="text-xs text-muted-foreground">
                  {{ webServerConfig.https_enabled
                    ? t('settings.autoRestartingHttpsDesc', { sec: autoRestartCountdown })
                    : t('settings.autoRestartingDesc') }}
                </p>
              </div>
              <span v-if="webServerConfig.https_enabled && autoRestartCountdown > 0"
                class="tabular-nums text-lg font-bold text-primary shrink-0">
                {{ autoRestartCountdown }}
              </span>
            </div>

            <!-- Auto-restart: HTTPS manual redirect (cert must be accepted by user) -->
            <Alert v-if="autoRestartManualUrl" variant="warning">
              <Lock />
              <AlertTitle>{{ t('settings.httpsManualRedirectTitle') }}</AlertTitle>
              <AlertDescription>{{ t('settings.httpsManualRedirectDesc') }}</AlertDescription>
              <Button as-child class="col-start-2 mt-2 w-full bg-warning text-warning-foreground hover:bg-warning/90">
                <a :href="autoRestartManualUrl"><ExternalLink />{{ autoRestartManualUrl }}</a>
              </Button>
            </Alert>

            <!-- Auto-restart: failure / timeout -->
            <Alert v-if="autoRestartFailed" variant="destructive">
              <AlertTriangle />
              <AlertDescription class="flex items-center justify-between gap-3">
              <span>{{ t('settings.autoRestartFailed') }}</span>
              <Button variant="outline" size="sm" class="text-foreground" @click="triggerAutoRestart">
                <RefreshCw class="size-3 mr-1" />
                {{ t('common.retry') }}
              </Button>
              </AlertDescription>
            </Alert>

            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.redfishTitle') }}</CardTitle>
                <CardDescription>{{ t('settings.redfishDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-5">
                <div class="flex items-start justify-between gap-4">
                  <Label>{{ t('settings.redfishEnabled') }}</Label>
                  <Switch v-model="redfishEnabled" />
                </div>
                <div class="rounded-md border bg-muted/40 p-3 space-y-1.5">
                  <p class="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">{{ t('settings.redfishEndpoint') }}</p>
                  <code class="block font-mono text-xs sm:text-sm break-all">{{ redfishAccessUrl }}</code>
                </div>
              </CardContent>
              <CardFooter class="flex items-center justify-between gap-3 border-t pt-4">
                <p class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                  <AlertTriangle class="size-3.5 shrink-0 text-warning" />
                  <span class="truncate">{{ t('settings.restartRequiredHint') }}</span>
                </p>
                <Button @click="saveRedfishConfig" :disabled="redfishSaving || autoRestarting">
                  <RefreshCw v-if="autoRestarting || redfishSaving" class="size-4 mr-2 animate-spin" />
                  <Save v-else class="size-4 mr-2" />
                  {{ autoRestarting ? t('settings.restarting') : t('common.save') }}
                </Button>
              </CardFooter>
            </Card>
          </div>

          <!-- About Section -->
          <div v-show="activeSection === 'about'" class="space-y-4">
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.onlineUpgrade') }}</CardTitle>
                  <CardDescription>{{ t('settings.onlineUpgradeDesc') }}</CardDescription>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  class="size-8"
                  :aria-label="t('common.refresh')"
                  :disabled="updateRunning || updateLoading"
                  @click="loadUpdateOverview"
                >
                  <RefreshCw :class="['size-4', (updateLoading || updateRunning) ? 'animate-spin' : '']" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label>{{ t('settings.currentVersion') }}</Label>
                    <Badge variant="outline">
                      {{ updateOverview?.current_version || systemStore.version || t('common.unknown') }}
                    </Badge>
                  </div>
                  <div class="space-y-2">
                    <Label>{{ t('settings.latestVersion') }}</Label>
                    <Badge variant="outline">{{ updateOverview?.latest_version || t('common.unknown') }}</Badge>
                  </div>
                </div>

                <div class="space-y-2">
                  <Label>{{ t('settings.updateChannel') }}</Label>
                  <Select v-model="updateChannel" :disabled="updateRunning">
                    <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                    <SelectContent><SelectItem value="stable">Stable</SelectItem><SelectItem value="beta">Beta</SelectItem></SelectContent>
                  </Select>
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
                    <RefreshCw class="size-4 mr-2" :class="updateRunning ? 'animate-spin' : ''" />
                    {{ t('settings.startUpgrade') }}
                  </Button>
                </div>
              </CardContent>
            </Card>

            <p class="text-xs text-muted-foreground text-center">@2025-2026 SilentWind</p>
          </div>

          <!-- Save Button (sticky) -->
          <div v-if="['video', 'hid'].includes(activeSection)" class="sticky bottom-0 pt-3 sm:pt-4 pb-3 bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80 border-t -mx-3 px-3 sm:-mx-6 sm:px-6 lg:-mx-8 lg:px-8">
            <div class="flex items-center justify-between gap-2 sm:gap-3">
              <p v-if="activeSection === 'hid' && !isHidFunctionSelectionValid" class="flex min-w-0 items-center gap-1.5 text-xs text-warning">
                <AlertTriangle class="size-3.5 shrink-0" />
                <span class="truncate">{{ t('settings.otgFunctionMinWarning') }}</span>
              </p>
              <p v-else-if="activeSection === 'hid' && !isCh9329DescriptorValid" class="flex min-w-0 items-center gap-1.5 text-xs text-warning">
                <AlertTriangle class="size-3.5 shrink-0" />
                <span class="truncate">{{ t('settings.ch9329StringLengthWarning') }}</span>
              </p>
              <p v-else-if="activeSection === 'hid' && config.hid_backend === 'ch9329' && ch9329DescriptorLoading" class="flex min-w-0 items-center gap-1.5 text-xs text-warning">
                <AlertTriangle class="size-3.5 shrink-0" />
                <span class="truncate">{{ t('settings.ch9329DescriptorLoading') }}</span>
              </p>
              <p v-if="saveError" class="text-xs text-destructive">{{ saveError }}</p>
              <p v-else class="text-xs text-muted-foreground hidden sm:block">{{ t('settings.unsavedChangesHint') }}</p>
              <Button class="shrink-0 ml-auto" :disabled="loading || (activeSection === 'hid' && !isHidSettingsValid)" @click="saveConfig">
                <Loader2 v-if="loading" class="size-4 mr-2 animate-spin" /><Check v-else-if="saved" class="size-4 mr-2" /><Save v-else class="size-4 mr-2" />{{ loading ? t('actionbar.applying') : saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>
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
