<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSystemStore } from '@/stores/system'
import {
  configApi,
  streamApi,
  userApi,
  videoConfigApi,
  streamConfigApi,
  hidConfigApi,
  msdConfigApi,
  atxConfigApi,
  extensionsApi,
  type EncoderBackendInfo,
  type User as UserType,
} from '@/api'
import type {
  ExtensionsStatus,
  ExtensionStatus,
  AtxDriverType,
  ActiveLevel,
  AtxDevices,
} from '@/types/generated'
import { setLanguage } from '@/i18n'
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
  Network,
  HardDrive,
  Power,
  UserPlus,
  User,
  Pencil,
  Trash2,
  Menu,
  Users,
  Globe,
  RefreshCw,
  Terminal,
  Play,
  Square,
  ChevronRight,
  Plus,
  ExternalLink,
} from 'lucide-vue-next'

const { t, locale } = useI18n()
const systemStore = useSystemStore()

// Settings state
const activeSection = ref('appearance')
const mobileMenuOpen = ref(false)
const loading = ref(false)
const saved = ref(false)

// Navigation structure
const navGroups = computed(() => [
  {
    title: t('settings.general'),
    items: [
      { id: 'appearance', label: t('settings.appearance'), icon: Sun },
    ]
  },
  {
    title: t('settings.hardware'),
    items: [
      { id: 'video', label: t('settings.video'), icon: Monitor, status: config.value.video_device ? t('settings.configured') : null },
      { id: 'hid', label: t('settings.hid'), icon: Keyboard, status: config.value.hid_backend.toUpperCase() },
      { id: 'msd', label: t('settings.msd'), icon: HardDrive },
      { id: 'atx', label: t('settings.atx'), icon: Power },
    ]
  },
  {
    title: t('settings.extensions'),
    items: [
      { id: 'ext-ttyd', label: t('extensions.ttyd.title'), icon: Terminal },
      { id: 'ext-gostc', label: t('extensions.gostc.title'), icon: Globe },
      { id: 'ext-easytier', label: t('extensions.easytier.title'), icon: Network },
    ]
  },
  {
    title: t('settings.system'),
    items: [
      { id: 'users', label: t('settings.users'), icon: Users },
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

// Password change
const showPasswordDialog = ref(false)
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const showPasswords = ref(false)
const passwordError = ref('')

// User management
const users = ref<UserType[]>([])
const usersLoading = ref(false)
const showAddUserDialog = ref(false)
const showEditUserDialog = ref(false)
const editingUser = ref<UserType | null>(null)
const newUser = ref({ username: '', password: '', role: 'user' as 'admin' | 'user' })
const editUserData = ref({ username: '', role: 'user' as 'admin' | 'user' })

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
  ttyd: { enabled: false, shell: '/bin/bash', credential: '' },
  gostc: { enabled: false, addr: 'gostc.mofeng.run', key: '', tls: true },
  easytier: { enabled: false, network_name: '', network_secret: '', peer_urls: [] as string[], virtual_ip: '' },
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
}

const devices = ref<DeviceConfig>({
  video: [],
  serial: [],
  audio: [],
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
  msd_enabled: false,
  network_port: 8080,
  encoder_backend: 'auto',
  // STUN/TURN settings
  stun_server: '',
  turn_server: '',
  turn_username: '',
  turn_password: '',
})

// 跟踪服务器是否已配置 TURN 密码
const hasTurnPassword = ref(false)

// ATX config state
const atxConfig = ref({
  enabled: false,
  power: {
    driver: 'none' as AtxDriverType,
    device: '',
    pin: 0,
    active_level: 'high' as ActiveLevel,
  },
  reset: {
    driver: 'none' as AtxDriverType,
    device: '',
    pin: 0,
    active_level: 'high' as ActiveLevel,
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
})

// Encoder backend
const availableBackends = ref<EncoderBackendInfo[]>([])

const selectedBackendFormats = computed(() => {
  if (config.value.encoder_backend === 'auto') return []
  const backend = availableBackends.value.find(b => b.id === config.value.encoder_backend)
  return backend?.supported_formats || []
})

// Video selection computed properties
import { computed, watch } from 'vue'

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

// Password change
async function changePassword() {
  passwordError.value = ''

  if (newPassword.value.length < 4) {
    passwordError.value = t('setup.passwordHint')
    return
  }

  if (newPassword.value !== confirmPassword.value) {
    passwordError.value = t('setup.passwordMismatch')
    return
  }

  try {
    await configApi.update({
      current_password: currentPassword.value,
      new_password: newPassword.value,
    })
    showPasswordDialog.value = false
    currentPassword.value = ''
    newPassword.value = ''
    confirmPassword.value = ''
  } catch (e) {
    passwordError.value = t('auth.invalidPassword')
  }
}

// MSD 开关变更处理
function onMsdEnabledChange(val: boolean) {
  config.value.msd_enabled = val
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
        videoConfigApi.update({
          device: config.value.video_device || undefined,
          format: config.value.video_format || undefined,
          width: config.value.video_width,
          height: config.value.video_height,
          fps: config.value.video_fps,
        })
      )
      // 同时保存 Stream/Encoder 和 STUN/TURN 配置
      savePromises.push(
        streamConfigApi.update({
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
      savePromises.push(
        hidConfigApi.update({
          backend: config.value.hid_backend as any,
          ch9329_port: config.value.hid_serial_device || undefined,
          ch9329_baudrate: config.value.hid_serial_baudrate,
        })
      )
    }

    // MSD 配置
    if (activeSection.value === 'msd') {
      savePromises.push(
        msdConfigApi.update({
          enabled: config.value.msd_enabled,
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
      videoConfigApi.get(),
      streamConfigApi.get(),
      hidConfigApi.get(),
      msdConfigApi.get(),
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
      msd_enabled: msd.enabled || false,
      network_port: 8080, // 从旧 API 加载
      encoder_backend: stream.encoder || 'auto',
      // STUN/TURN settings
      stun_server: stream.stun_server || '',
      turn_server: stream.turn_server || '',
      turn_username: stream.turn_username || '',
      turn_password: '', // 密码不从服务器返回，仅用于设置
    }

    // 设置是否已配置 TURN 密码
    hasTurnPassword.value = stream.has_turn_password || false

    // 加载 web config（仍使用旧 API）
    try {
      const fullConfig = await configApi.get()
      const web = fullConfig.web as any || {}
      config.value.network_port = web.http_port || 8080
    } catch (e) {
      console.warn('Failed to load web config:', e)
    }
  } catch (e) {
    console.error('Failed to load config:', e)
  }
}

async function loadDevices() {
  try {
    devices.value = await configApi.listDevices()
  } catch (e) {
    console.error('Failed to load devices:', e)
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

// User management functions
async function loadUsers() {
  usersLoading.value = true
  try {
    const result = await userApi.list()
    users.value = result.users || []
  } catch (e) {
    console.error('Failed to load users:', e)
  } finally {
    usersLoading.value = false
  }
}

async function createUser() {
  if (!newUser.value.username || !newUser.value.password) return
  try {
    await userApi.create(newUser.value.username, newUser.value.password, newUser.value.role)
    showAddUserDialog.value = false
    newUser.value = { username: '', password: '', role: 'user' }
    await loadUsers()
  } catch (e) {
    console.error('Failed to create user:', e)
  }
}

function openEditUserDialog(user: UserType) {
  editingUser.value = user
  editUserData.value = { username: user.username, role: user.role }
  showEditUserDialog.value = true
}

async function updateUser() {
  if (!editingUser.value) return
  try {
    await userApi.update(editingUser.value.id, editUserData.value)
    showEditUserDialog.value = false
    editingUser.value = null
    await loadUsers()
  } catch (e) {
    console.error('Failed to update user:', e)
  }
}

async function confirmDeleteUser(user: UserType) {
  if (!confirm(`Delete user "${user.username}"?`)) return
  try {
    await userApi.delete(user.id)
    await loadUsers()
  } catch (e) {
    console.error('Failed to delete user:', e)
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
        credential: ttyd.credential || '',
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
    const config = await atxConfigApi.get()
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
    await atxConfigApi.update({
      enabled: atxConfig.value.enabled,
      power: {
        driver: atxConfig.value.power.driver,
        device: atxConfig.value.power.device || undefined,
        pin: atxConfig.value.power.pin,
        active_level: atxConfig.value.power.active_level,
      },
      reset: {
        driver: atxConfig.value.reset.driver,
        device: atxConfig.value.reset.device || undefined,
        pin: atxConfig.value.reset.pin,
        active_level: atxConfig.value.reset.active_level,
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
  } else if (driver === 'usbrelay') {
    return atxDevices.value.usb_relays
  }
  return []
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
    loadUsers(),
    loadExtensions(),
    loadAtxConfig(),
    loadAtxDevices(),
  ])
})
</script>

<template>
  <AppLayout>
    <div class="flex h-[calc(100vh-6rem)]">
      <!-- Mobile Header -->
      <div class="lg:hidden fixed top-16 left-0 right-0 z-20 flex items-center justify-between px-4 py-3 border-b bg-background">
        <h1 class="text-lg font-semibold">{{ t('settings.title') }}</h1>
        <Sheet v-model:open="mobileMenuOpen">
          <SheetTrigger as-child>
            <Button variant="outline" size="sm">
              <Menu class="h-4 w-4 mr-2" />
              {{ t('common.menu') }}
            </Button>
          </SheetTrigger>
          <SheetContent side="left" class="w-72 p-0">
            <div class="p-6">
              <h2 class="text-lg font-semibold mb-4">{{ t('settings.title') }}</h2>
              <nav class="space-y-6">
                <div v-for="group in navGroups" :key="group.title" class="space-y-1">
                  <h3 class="px-3 text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">{{ group.title }}</h3>
                  <button
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
      </div>

      <!-- Desktop Sidebar -->
      <aside class="hidden lg:block w-64 shrink-0 border-r bg-muted/30">
        <div class="sticky top-0 p-6 space-y-6">
          <h1 class="text-xl font-semibold">{{ t('settings.title') }}</h1>
          <nav class="space-y-6">
            <div v-for="group in navGroups" :key="group.title" class="space-y-1">
              <h3 class="px-3 text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">{{ group.title }}</h3>
              <button
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

          <!-- Video Section -->
          <div v-show="activeSection === 'video'" class="space-y-6">
            <!-- Video Device Settings -->
            <Card>
              <CardHeader class="flex flex-row items-start justify-between space-y-0">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.videoSettings') }}</CardTitle>
                  <CardDescription>{{ t('settings.videoSettingsDesc') }}</CardDescription>
                </div>
                <Button variant="ghost" size="icon" class="h-8 w-8" @click="loadDevices">
                  <RefreshCw class="h-4 w-4" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="video-device">{{ t('settings.videoDevice') }}</Label>
                  <select id="video-device" v-model="config.video_device" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option value="">{{ t('settings.selectDevice') }}</option>
                    <option v-for="dev in devices.video" :key="dev.path" :value="dev.path">{{ dev.name }}</option>
                  </select>
                </div>
                <div class="space-y-2">
                  <Label for="video-format">{{ t('settings.videoFormat') }}</Label>
                  <select id="video-format" v-model="config.video_format" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm" :disabled="!config.video_device">
                    <option value="">{{ t('settings.selectFormat') }}</option>
                    <option v-for="fmt in availableFormats" :key="fmt.format" :value="fmt.format">{{ fmt.format }} - {{ fmt.description }}</option>
                  </select>
                </div>
                <div class="grid grid-cols-2 gap-4">
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
                <div class="grid grid-cols-2 gap-4">
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
                <Button variant="ghost" size="icon" class="h-8 w-8" @click="loadDevices">
                  <RefreshCw class="h-4 w-4" />
                </Button>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="space-y-2">
                  <Label for="hid-backend">{{ t('settings.hidBackend') }}</Label>
                  <select id="hid-backend" v-model="config.hid_backend" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                    <option value="ch9329">CH9329 (Serial)</option>
                    <option value="otg">USB OTG</option>
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
              </CardContent>
            </Card>
          </div>

          <!-- Users Section -->
          <div v-show="activeSection === 'users'" class="space-y-6">
            <Card>
              <CardHeader class="flex flex-row items-center justify-between space-y-0 pb-4">
                <div class="space-y-1.5">
                  <CardTitle>{{ t('settings.userManagement') }}</CardTitle>
                  <CardDescription>{{ t('settings.userManagementDesc') }}</CardDescription>
                </div>
                <Button size="sm" @click="showAddUserDialog = true">
                  <UserPlus class="h-4 w-4 mr-2" />{{ t('settings.addUser') }}
                </Button>
              </CardHeader>
              <CardContent>
                <div v-if="usersLoading" class="text-center py-8">
                  <p class="text-sm text-muted-foreground">{{ t('settings.loadingUsers') }}</p>
                </div>
                <div v-else-if="users.length === 0" class="text-center py-8">
                  <User class="h-8 w-8 mx-auto mb-2 text-muted-foreground" />
                  <p class="text-sm text-muted-foreground">{{ t('settings.noUsers') }}</p>
                </div>
                <div v-else class="divide-y">
                  <div v-for="user in users" :key="user.id" class="flex items-center justify-between py-3">
                    <div class="flex items-center gap-3">
                      <div class="h-8 w-8 rounded-full bg-muted flex items-center justify-center">
                        <User class="h-4 w-4" />
                      </div>
                      <div>
                        <p class="text-sm font-medium">{{ user.username }}</p>
                        <Badge variant="outline" class="text-xs">{{ user.role === 'admin' ? t('settings.roleAdmin') : t('settings.roleUser') }}</Badge>
                      </div>
                    </div>
                    <div class="flex gap-1">
                      <Button size="icon" variant="ghost" class="h-8 w-8" @click="openEditUserDialog(user)"><Pencil class="h-4 w-4" /></Button>
                      <Button size="icon" variant="ghost" class="h-8 w-8 text-destructive" :disabled="user.role === 'admin' && users.filter(u => u.role === 'admin').length === 1" @click="confirmDeleteUser(user)"><Trash2 class="h-4 w-4" /></Button>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          <!-- MSD Section -->
          <div v-show="activeSection === 'msd'" class="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>{{ t('settings.msdSettings') }}</CardTitle>
                <CardDescription>{{ t('settings.msdDesc') }}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <Label for="msd-enabled">{{ t('settings.msdEnable') }}</Label>
                    <p class="text-xs text-muted-foreground">{{ t('settings.msdEnableDesc') }}</p>
                  </div>
                  <Switch
                    id="msd-enabled"
                    :model-value="config.msd_enabled"
                    @update:model-value="onMsdEnabledChange"
                  />
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
                <Button variant="ghost" size="icon" class="h-8 w-8" @click="loadAtxDevices">
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
                <div class="grid grid-cols-2 gap-4">
                  <div class="space-y-2">
                    <Label for="power-driver">{{ t('settings.atxDriver') }}</Label>
                    <select id="power-driver" v-model="atxConfig.power.driver" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="none">{{ t('settings.atxDriverNone') }}</option>
                      <option value="gpio">{{ t('settings.atxDriverGpio') }}</option>
                      <option value="usbrelay">{{ t('settings.atxDriverUsbRelay') }}</option>
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
                <div class="grid grid-cols-2 gap-4">
                  <div class="space-y-2">
                    <Label for="power-pin">{{ atxConfig.power.driver === 'usbrelay' ? t('settings.atxChannel') : t('settings.atxPin') }}</Label>
                    <Input id="power-pin" type="number" v-model.number="atxConfig.power.pin" min="0" :disabled="atxConfig.power.driver === 'none'" />
                  </div>
                  <div v-if="atxConfig.power.driver === 'gpio'" class="space-y-2">
                    <Label for="power-level">{{ t('settings.atxActiveLevel') }}</Label>
                    <select id="power-level" v-model="atxConfig.power.active_level" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="high">{{ t('settings.atxLevelHigh') }}</option>
                      <option value="low">{{ t('settings.atxLevelLow') }}</option>
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
                <div class="grid grid-cols-2 gap-4">
                  <div class="space-y-2">
                    <Label for="reset-driver">{{ t('settings.atxDriver') }}</Label>
                    <select id="reset-driver" v-model="atxConfig.reset.driver" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="none">{{ t('settings.atxDriverNone') }}</option>
                      <option value="gpio">{{ t('settings.atxDriverGpio') }}</option>
                      <option value="usbrelay">{{ t('settings.atxDriverUsbRelay') }}</option>
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
                <div class="grid grid-cols-2 gap-4">
                  <div class="space-y-2">
                    <Label for="reset-pin">{{ atxConfig.reset.driver === 'usbrelay' ? t('settings.atxChannel') : t('settings.atxPin') }}</Label>
                    <Input id="reset-pin" type="number" v-model.number="atxConfig.reset.pin" min="0" :disabled="atxConfig.reset.driver === 'none'" />
                  </div>
                  <div v-if="atxConfig.reset.driver === 'gpio'" class="space-y-2">
                    <Label for="reset-level">{{ t('settings.atxActiveLevel') }}</Label>
                    <select id="reset-level" v-model="atxConfig.reset.active_level" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
                      <option value="high">{{ t('settings.atxLevelHigh') }}</option>
                      <option value="low">{{ t('settings.atxLevelLow') }}</option>
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
                  <div class="grid grid-cols-2 gap-4">
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
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.ttyd.shell') }}</Label>
                      <Input v-model="extConfig.ttyd.shell" class="col-span-3" placeholder="/bin/bash" :disabled="isExtRunning(extensions?.ttyd?.status)" />
                    </div>
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.ttyd.credential') }}</Label>
                      <Input v-model="extConfig.ttyd.credential" class="col-span-3" placeholder="user:password" :disabled="isExtRunning(extensions?.ttyd?.status)" />
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <button @click="showLogs.ttyd = !showLogs.ttyd; if (showLogs.ttyd) refreshExtensionLogs('ttyd')" class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
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

          <!-- gostc Section -->
          <div v-show="activeSection === 'ext-gostc'" class="space-y-6">
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
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.gostc.addr') }}</Label>
                      <Input v-model="extConfig.gostc.addr" class="col-span-3" placeholder="gostc.mofeng.run" :disabled="isExtRunning(extensions?.gostc?.status)" />
                    </div>
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.gostc.key') }}</Label>
                      <Input v-model="extConfig.gostc.key" type="password" class="col-span-3" :disabled="isExtRunning(extensions?.gostc?.status)" />
                    </div>
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.gostc.tls') }}</Label>
                      <div class="col-span-3">
                        <Switch v-model="extConfig.gostc.tls" :disabled="isExtRunning(extensions?.gostc?.status)" />
                      </div>
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <button @click="showLogs.gostc = !showLogs.gostc; if (showLogs.gostc) refreshExtensionLogs('gostc')" class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
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
          </div>

          <!-- easytier Section -->
          <div v-show="activeSection === 'ext-easytier'" class="space-y-6">
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
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.easytier.networkName') }}</Label>
                      <Input v-model="extConfig.easytier.network_name" class="col-span-3" :disabled="isExtRunning(extensions?.easytier?.status)" />
                    </div>
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.easytier.networkSecret') }}</Label>
                      <Input v-model="extConfig.easytier.network_secret" type="password" class="col-span-3" :disabled="isExtRunning(extensions?.easytier?.status)" />
                    </div>
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.easytier.peers') }}</Label>
                      <div class="col-span-3 space-y-2">
                        <div v-for="(_, i) in extConfig.easytier.peer_urls" :key="i" class="flex gap-2">
                          <Input v-model="extConfig.easytier.peer_urls[i]" placeholder="tcp://1.2.3.4:11010" :disabled="isExtRunning(extensions?.easytier?.status)" />
                          <Button variant="ghost" size="icon" @click="removeEasytierPeer(i)" :disabled="isExtRunning(extensions?.easytier?.status)">
                            <Trash2 class="h-4 w-4" />
                          </Button>
                        </div>
                        <Button variant="outline" size="sm" @click="addEasytierPeer" :disabled="isExtRunning(extensions?.easytier?.status)">
                          <Plus class="h-4 w-4 mr-1" />
                          {{ t('extensions.easytier.addPeer') }}
                        </Button>
                      </div>
                    </div>
                    <div class="grid grid-cols-4 items-center gap-4">
                      <Label class="text-right">{{ t('extensions.easytier.virtualIp') }}</Label>
                      <div class="col-span-3 space-y-1">
                        <Input v-model="extConfig.easytier.virtual_ip" placeholder="10.0.0.1/24" :disabled="isExtRunning(extensions?.easytier?.status)" />
                        <p class="text-xs text-muted-foreground">{{ t('extensions.easytier.virtualIpHint') }}</p>
                      </div>
                    </div>
                  </div>
                  <!-- Logs -->
                  <div class="space-y-2">
                    <button @click="showLogs.easytier = !showLogs.easytier; if (showLogs.easytier) refreshExtensionLogs('easytier')" class="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
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

          <!-- About Section -->
          <div v-show="activeSection === 'about'" class="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>One-KVM</CardTitle>
                <CardDescription>{{ t('settings.aboutDesc') }}</CardDescription>
              </CardHeader>
              <CardContent>
                <div class="flex justify-between items-center py-2">
                  <span class="text-sm text-muted-foreground">{{ t('settings.version') }}</span>
                  <Badge>{{ systemStore.version || t('common.unknown') }} ({{ systemStore.buildDate || t('common.unknown') }})</Badge>
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
              <Button :disabled="loading" @click="saveConfig">
                <Check v-if="saved" class="h-4 w-4 mr-2" /><Save v-else class="h-4 w-4 mr-2" />{{ saved ? t('common.success') : t('common.save') }}
              </Button>
            </div>
          </div>

        </div>
      </main>
    </div>

    <!-- Password Change Dialog -->
    <Dialog v-model:open="showPasswordDialog">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('settings.changePassword') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-4">
          <div class="space-y-2">
            <Label for="current-password">{{ t('settings.currentPassword') }}</Label>
            <div class="relative">
              <Input id="current-password" v-model="currentPassword" :type="showPasswords ? 'text' : 'password'" />
              <button type="button" class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground" @click="showPasswords = !showPasswords">
                <Eye v-if="!showPasswords" class="h-4 w-4" /><EyeOff v-else class="h-4 w-4" />
              </button>
            </div>
          </div>
          <div class="space-y-2">
            <Label for="new-password">{{ t('settings.newPassword') }}</Label>
            <Input id="new-password" v-model="newPassword" :type="showPasswords ? 'text' : 'password'" />
          </div>
          <div class="space-y-2">
            <Label for="confirm-password">{{ t('setup.confirmPassword') }}</Label>
            <Input id="confirm-password" v-model="confirmPassword" :type="showPasswords ? 'text' : 'password'" />
          </div>
          <p v-if="passwordError" class="text-sm text-destructive">{{ passwordError }}</p>
        </div>
        <DialogFooter>
          <Button variant="outline" size="sm" @click="showPasswordDialog = false">{{ t('common.cancel') }}</Button>
          <Button size="sm" @click="changePassword">{{ t('common.save') }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Add User Dialog -->
    <Dialog v-model:open="showAddUserDialog">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('settings.addUser') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-4">
          <div class="space-y-2">
            <Label for="new-username">{{ t('settings.username') }}</Label>
            <Input id="new-username" v-model="newUser.username" />
          </div>
          <div class="space-y-2">
            <Label for="new-user-password">{{ t('settings.password') }}</Label>
            <Input id="new-user-password" v-model="newUser.password" type="password" />
          </div>
          <div class="space-y-2">
            <Label for="new-user-role">{{ t('settings.role') }}</Label>
            <select id="new-user-role" v-model="newUser.role" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
              <option value="user">{{ t('settings.roleUser') }}</option>
              <option value="admin">{{ t('settings.roleAdmin') }}</option>
            </select>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" size="sm" @click="showAddUserDialog = false">{{ t('common.cancel') }}</Button>
          <Button size="sm" @click="createUser">{{ t('settings.create') }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Edit User Dialog -->
    <Dialog v-model:open="showEditUserDialog">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('settings.editUser') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-4">
          <div class="space-y-2">
            <Label for="edit-username">{{ t('settings.username') }}</Label>
            <Input id="edit-username" v-model="editUserData.username" />
          </div>
          <div class="space-y-2">
            <Label for="edit-user-role">{{ t('settings.role') }}</Label>
            <select id="edit-user-role" v-model="editUserData.role" class="w-full h-9 px-3 rounded-md border border-input bg-background text-sm">
              <option value="user">{{ t('settings.roleUser') }}</option>
              <option value="admin">{{ t('settings.roleAdmin') }}</option>
            </select>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" size="sm" @click="showEditUserDialog = false">{{ t('common.cancel') }}</Button>
          <Button size="sm" @click="updateUser">{{ t('common.save') }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Terminal Dialog -->
    <Dialog v-model:open="showTerminalDialog">
      <DialogContent class="max-w-[95vw] w-[1200px] h-[600px] p-0 flex flex-col overflow-hidden">
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
  </AppLayout>
</template>
