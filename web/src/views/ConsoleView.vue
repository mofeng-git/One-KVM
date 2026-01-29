<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { useAuthStore } from '@/stores/auth'
import { useWebSocket } from '@/composables/useWebSocket'
import { useConsoleEvents } from '@/composables/useConsoleEvents'
import { useHidWebSocket } from '@/composables/useHidWebSocket'
import { useWebRTC } from '@/composables/useWebRTC'
import { useVideoSession } from '@/composables/useVideoSession'
import { getUnifiedAudio } from '@/composables/useUnifiedAudio'
import { streamApi, hidApi, atxApi, extensionsApi, atxConfigApi, authApi } from '@/api'
import type { HidKeyboardEvent, HidMouseEvent } from '@/types/hid'
import { toast } from 'vue-sonner'
import { generateUUID } from '@/lib/utils'
import type { VideoMode } from '@/components/VideoConfigPopover.vue'

// Components
import StatusCard, { type StatusDetail } from '@/components/StatusCard.vue'
import ActionBar from '@/components/ActionBar.vue'
import InfoBar from '@/components/InfoBar.vue'
import VirtualKeyboard from '@/components/VirtualKeyboard.vue'
import StatsSheet from '@/components/StatsSheet.vue'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Monitor,
  MonitorOff,
  RefreshCw,
  LogOut,
  Sun,
  Moon,
  Languages,
  ChevronDown,
  Terminal,
  ExternalLink,
  KeyRound,
  Loader2,
} from 'lucide-vue-next'
import { setLanguage } from '@/i18n'

const { t, locale } = useI18n()
const router = useRouter()
const systemStore = useSystemStore()
const authStore = useAuthStore()
const { connected: wsConnected, networkError: wsNetworkError } = useWebSocket()
const hidWs = useHidWebSocket()
const webrtc = useWebRTC()
const unifiedAudio = getUnifiedAudio()
const videoSession = useVideoSession()

const consoleEvents = useConsoleEvents({
  onStreamConfigChanging: handleStreamConfigChanging,
  onStreamConfigApplied: handleStreamConfigApplied,
  onStreamStatsUpdate: handleStreamStatsUpdate,
  onStreamModeChanged: handleStreamModeChanged,
  onStreamModeSwitching: handleStreamModeSwitching,
  onStreamModeReady: handleStreamModeReady,
  onWebRTCReady: handleWebRTCReady,
  onStreamStateChanged: handleStreamStateChanged,
  onStreamDeviceLost: handleStreamDeviceLost,
  onStreamRecovered: handleStreamRecovered,
  onDeviceInfo: handleDeviceInfo,
  onAudioStateChanged: handleAudioStateChanged,
})

// Video mode state
const videoMode = ref<VideoMode>('mjpeg')

// Video state
const videoRef = ref<HTMLImageElement | null>(null)
const webrtcVideoRef = ref<HTMLVideoElement | null>(null)
const videoContainerRef = ref<HTMLDivElement | null>(null)
const isFullscreen = ref(false)
const videoLoading = ref(true)
const videoError = ref(false)
const videoErrorMessage = ref('')
const videoRestarting = ref(false) // Track if video is restarting due to config change

// Video aspect ratio (dynamically updated from actual video dimensions)
// Using string format "width/height" to let browser handle the ratio calculation
const videoAspectRatio = ref<string | null>(null)

// Backend-provided FPS (received from WebSocket stream.stats_update events)
const backendFps = ref(0)

// Per-client statistics from backend
interface ClientStat {
  id: string
  fps: number  // Integer: frames sent in last second
  connected_secs: number
}
const clientsStats = ref<Record<string, ClientStat>>({})

// Generate a unique client ID for this browser session
// This allows us to identify our own stats in the clients_stat map
const myClientId = generateUUID()

// HID state
const mouseMode = ref<'absolute' | 'relative'>('absolute')
const pressedKeys = ref<string[]>([])
const keyboardLed = ref({
  capsLock: false,
})
const mousePosition = ref({ x: 0, y: 0 })
const lastMousePosition = ref({ x: 0, y: 0 }) // Track last position for relative mode
const isPointerLocked = ref(false) // Track pointer lock state

// Mouse move throttling (60 Hz = ~16.67ms interval)
const DEFAULT_MOUSE_MOVE_SEND_INTERVAL_MS = 16
let mouseMoveSendIntervalMs = DEFAULT_MOUSE_MOVE_SEND_INTERVAL_MS
let mouseFlushTimer: ReturnType<typeof setTimeout> | null = null
let lastMouseMoveSendTime = 0
let pendingMouseMove: { type: 'move' | 'move_abs'; x: number; y: number } | null = null
let accumulatedDelta = { x: 0, y: 0 } // For relative mode: accumulate deltas between sends

// Cursor visibility (from localStorage, updated via storage event)
const cursorVisible = ref(localStorage.getItem('hidShowCursor') !== 'false')

// Virtual keyboard state
const virtualKeyboardVisible = ref(false)
const virtualKeyboardAttached = ref(true)
const statsSheetOpen = ref(false)

// Change password dialog state
const changePasswordDialogOpen = ref(false)
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const changingPassword = ref(false)

// ttyd (web terminal) state
const ttydStatus = ref<{ available: boolean; running: boolean } | null>(null)
const showTerminalDialog = ref(false)
let ttydPollInterval: ReturnType<typeof setInterval> | null = null

// Theme
const isDark = ref(document.documentElement.classList.contains('dark'))

// Status computed (Device status removed - now only Video, Audio, HID, MSD)
const videoStatus = computed<'connected' | 'connecting' | 'disconnected' | 'error'>(() => {
  // If WebSocket has network error, video is also affected (same network dependency)
  if (wsNetworkError.value) return 'connecting'

  if (videoError.value) return 'error'
  if (videoLoading.value) return 'connecting'
  if (systemStore.stream?.online) return 'connected'
  return 'disconnected'
})

// Convert resolution to short format (e.g., 720p, 1080p, 2K, 4K)
function getResolutionShortName(width: number, height: number): string {
  // Common resolution mappings based on height
  if (height === 2160 || (height === 2160 && width === 4096)) return '4K'
  if (height === 1440) return '2K'
  if (height === 1080) return '1080p'
  if (height === 720) return '720p'
  if (height === 768) return '768p'
  if (height === 600) return '600p'
  if (height === 1024 && width === 1280) return '1024p'
  if (height === 960) return '960p'
  // Fallback: use height + 'p'
  return `${height}p`
}

// Quick info for status card trigger
const videoQuickInfo = computed(() => {
  const stream = systemStore.stream
  if (!stream?.resolution) return ''
  const resShort = getResolutionShortName(stream.resolution[0], stream.resolution[1])
  return `${resShort} ${backendFps.value}fps`
})

const videoDetails = computed<StatusDetail[]>(() => {
  const stream = systemStore.stream
  if (!stream) return []
  // Use backend-provided FPS from WebSocket
  const receivedFps = backendFps.value
  // Display mode: use local videoMode which is synced with server
  const modeDisplay = videoMode.value === 'mjpeg' ? 'MJPEG' : `${videoMode.value.toUpperCase()} (WebRTC)`
  const details: StatusDetail[] = [
    { label: t('statusCard.device'), value: stream.device || '-' },
    { label: t('statusCard.mode'), value: modeDisplay, status: 'ok' },
    { label: t('statusCard.format'), value: stream.format || 'MJPEG' },
    { label: t('statusCard.resolution'), value: stream.resolution ? `${stream.resolution[0]}x${stream.resolution[1]}` : '-' },
    { label: t('statusCard.targetFps'), value: String(stream.targetFps ?? 0) },
    { label: t('statusCard.fps'), value: String(receivedFps), status: receivedFps > 5 ? 'ok' : receivedFps > 0 ? 'warning' : undefined },
  ]

  // Show network error if WebSocket has network issue
  if (wsNetworkError.value) {
    details.push({ label: t('statusCard.connection'), value: t('statusCard.networkError'), status: 'warning' })
  }

  return details
})

const hidStatus = computed<'connected' | 'connecting' | 'disconnected' | 'error'>(() => {
  // In WebRTC mode, check DataChannel status first
  if (videoMode.value !== 'mjpeg') {
    // DataChannel is ready - HID is connected via WebRTC
    if (webrtc.dataChannelReady.value) return 'connected'
    // WebRTC is connecting - HID is also connecting
    if (webrtc.isConnecting.value) return 'connecting'
    // WebRTC is connected but DataChannel not ready - still connecting
    if (webrtc.isConnected.value) return 'connecting'
    // WebRTC not connected - fall through to WebSocket check as fallback
  }

  // MJPEG mode or WebRTC fallback: check WebSocket HID status
  // If HID WebSocket has network error, show connecting (yellow)
  if (hidWs.networkError.value) return 'connecting'

  // If HID WebSocket is not connected (disconnected without error), show disconnected
  if (!hidWs.connected.value) return 'disconnected'

  // If HID backend is unavailable (business error), show disconnected (gray)
  if (hidWs.hidUnavailable.value) return 'disconnected'

  // Normal status based on system state
  if (systemStore.hid?.available && systemStore.hid?.initialized) return 'connected'
  if (systemStore.hid?.available && !systemStore.hid?.initialized) return 'connecting'
  return 'disconnected'
})

// Quick info for HID status card trigger
const hidQuickInfo = computed(() => {
  const hid = systemStore.hid
  if (!hid?.available) return ''
  // Show current mode, not hardware capability
  return mouseMode.value === 'absolute' ? t('statusCard.absolute') : t('statusCard.relative')
})

const hidDetails = computed<StatusDetail[]>(() => {
  const hid = systemStore.hid
  if (!hid) return []

  const details: StatusDetail[] = [
    { label: t('statusCard.device'), value: hid.device || '-' },
    { label: t('statusCard.backend'), value: hid.backend || t('common.unknown') },
    { label: t('statusCard.initialized'), value: hid.initialized ? t('statusCard.yes') : t('statusCard.no'), status: hid.initialized ? 'ok' : 'warning' },
    { label: t('statusCard.mouseSupport'), value: hid.supportsAbsoluteMouse ? t('statusCard.absolute') : t('statusCard.relative'), status: hid.available ? 'ok' : undefined },
    { label: t('statusCard.currentMode'), value: mouseMode.value === 'absolute' ? t('statusCard.absolute') : t('statusCard.relative'), status: 'ok' },
  ]

  // Add HID channel info based on video mode
  if (videoMode.value !== 'mjpeg') {
    // WebRTC mode - show DataChannel status
    if (webrtc.dataChannelReady.value) {
      details.push({ label: t('statusCard.channel'), value: 'DataChannel (WebRTC)', status: 'ok' })
    } else if (webrtc.isConnecting.value || webrtc.isConnected.value) {
      details.push({ label: t('statusCard.channel'), value: 'DataChannel', status: 'warning' })
    } else {
      // Fallback to WebSocket
      details.push({ label: t('statusCard.channel'), value: 'WebSocket (fallback)', status: hidWs.connected.value ? 'ok' : 'warning' })
    }
  } else {
    // MJPEG mode - WebSocket HID
    details.push({ label: t('statusCard.channel'), value: 'WebSocket', status: hidWs.connected.value ? 'ok' : 'warning' })
  }

  // Add connection status for WebSocket (only relevant for MJPEG or fallback)
  if (videoMode.value === 'mjpeg' || !webrtc.dataChannelReady.value) {
    if (hidWs.networkError.value) {
      details.push({ label: t('statusCard.connection'), value: t('statusCard.networkError'), status: 'warning' })
    } else if (!hidWs.connected.value) {
      details.push({ label: t('statusCard.connection'), value: t('statusCard.disconnected'), status: 'warning' })
    } else if (hidWs.hidUnavailable.value) {
      details.push({ label: t('statusCard.availability'), value: t('statusCard.hidUnavailable'), status: 'warning' })
    }
  }

  return details
})

// Audio status computed
const audioStatus = computed<'connected' | 'connecting' | 'disconnected' | 'error'>(() => {
  const audio = systemStore.audio
  if (!audio?.available) return 'disconnected'
  if (audio.error) return 'error'
  if (audio.streaming) return 'connected'
  return 'disconnected'
})

// Helper function to translate audio quality
function translateAudioQuality(quality: string | undefined): string {
  if (!quality) return t('common.unknown')
  const qualityLower = quality.toLowerCase()
  if (qualityLower === 'voice') return t('actionbar.qualityVoice')
  if (qualityLower === 'balanced') return t('actionbar.qualityBalanced')
  if (qualityLower === 'high') return t('actionbar.qualityHigh')
  return quality // fallback to original value
}

const audioQuickInfo = computed(() => {
  const audio = systemStore.audio
  if (!audio?.available) return ''
  if (audio.streaming) return translateAudioQuality(audio.quality)
  return t('statusCard.off')
})

const audioErrorMessage = computed(() => {
  return systemStore.audio?.error || ''
})

const audioDetails = computed<StatusDetail[]>(() => {
  const audio = systemStore.audio
  if (!audio) return []

  return [
    { label: t('statusCard.device'), value: audio.device || t('statusCard.defaultDevice') },
    { label: t('statusCard.quality'), value: translateAudioQuality(audio.quality) },
    { label: t('statusCard.streaming'), value: audio.streaming ? t('statusCard.yes') : t('statusCard.no'), status: audio.streaming ? 'ok' : undefined },
  ]
})

// MSD status computed
const msdStatus = computed<'connected' | 'connecting' | 'disconnected' | 'error'>(() => {
  const msd = systemStore.msd
  if (!msd?.available) return 'disconnected'
  if (msd.error) return 'error'
  if (msd.connected) return 'connected'
  return 'disconnected'
})

const msdQuickInfo = computed(() => {
  const msd = systemStore.msd
  if (!msd?.available) return ''
  if (msd.mode === 'none') return t('statusCard.msdStandby')
  if (msd.mode === 'image') return t('statusCard.msdImageMode')
  if (msd.mode === 'drive') return t('statusCard.msdDriveMode')
  return t('common.unknown')
})

const msdErrorMessage = computed(() => {
  return systemStore.msd?.error || ''
})

const msdDetails = computed<StatusDetail[]>(() => {
  const msd = systemStore.msd
  if (!msd) return []

  const details: StatusDetail[] = []

  // 状态：待机 / 已连接
  if (msd.mode === 'none') {
    details.push({
      label: t('statusCard.msdStatus'),
      value: t('statusCard.msdStandby'),
      status: undefined
    })
  } else {
    details.push({
      label: t('statusCard.msdStatus'),
      value: t('statusCard.connected'),
      status: 'ok'
    })
  }

  // 模式
  const modeDisplay = msd.mode === 'none'
    ? '-'
    : msd.mode === 'image'
      ? t('statusCard.msdImageMode')
      : t('statusCard.msdDriveMode')
  details.push({
    label: t('statusCard.mode'),
    value: modeDisplay,
    status: msd.mode !== 'none' ? 'ok' : undefined
  })

  // 当前镜像（仅在 image 模式下显示）
  if (msd.mode === 'image') {
    details.push({
      label: t('statusCard.msdCurrentImage'),
      value: msd.imageId || t('statusCard.msdNoImage')
    })
  }

  return details
})

// Video handling
let retryTimeoutId: number | null = null
let retryCount = 0
let gracePeriodTimeoutId: number | null = null
let consecutiveErrors = 0
const BASE_RETRY_DELAY = 2000
const GRACE_PERIOD = 2000 // Ignore errors for 2s after config change (reduced from 3s)
const MAX_CONSECUTIVE_ERRORS = 2 // If 2+ errors in grace period, it's a real problem

// Last-frame overlay (prevents black flash during mode switches)
const frameOverlayUrl = ref<string | null>(null)

function clearFrameOverlay() {
  frameOverlayUrl.value = null
}

async function captureFrameOverlay() {
  try {
    const canvas = document.createElement('canvas')
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const MAX_WIDTH = 1280

    if (videoMode.value === 'mjpeg') {
      const img = videoRef.value
      if (!img || !img.naturalWidth || !img.naturalHeight) return

      const scale = Math.min(1, MAX_WIDTH / img.naturalWidth)
      canvas.width = Math.max(1, Math.round(img.naturalWidth * scale))
      canvas.height = Math.max(1, Math.round(img.naturalHeight * scale))
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
    } else {
      const video = webrtcVideoRef.value
      if (!video || !video.videoWidth || !video.videoHeight) return

      const scale = Math.min(1, MAX_WIDTH / video.videoWidth)
      canvas.width = Math.max(1, Math.round(video.videoWidth * scale))
      canvas.height = Math.max(1, Math.round(video.videoHeight * scale))
      ctx.drawImage(video, 0, 0, canvas.width, canvas.height)
    }

    // Use JPEG to keep memory reasonable
    frameOverlayUrl.value = canvas.toDataURL('image/jpeg', 0.7)
  } catch {
    // Best-effort only
  }
}

function waitForVideoFirstFrame(el: HTMLVideoElement, timeoutMs = 2000): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false

    const cleanup = () => {
      el.removeEventListener('loadeddata', onReady)
      el.removeEventListener('playing', onReady)
    }

    const onReady = () => {
      if (done) return
      done = true
      cleanup()
      resolve(true)
    }

    el.addEventListener('loadeddata', onReady)
    el.addEventListener('playing', onReady)

    setTimeout(() => {
      if (done) return
      done = true
      cleanup()
      resolve(false)
    }, timeoutMs)
  })
}

function handleVideoLoad() {
  // MJPEG video frame loaded successfully - update stream online status
  // This fixes the timing issue where device_info event may arrive before stream is fully active
  if (videoMode.value === 'mjpeg') {
    systemStore.setStreamOnline(true)
    // Update aspect ratio from MJPEG image dimensions
    const img = videoRef.value
    if (img && img.naturalWidth && img.naturalHeight) {
      videoAspectRatio.value = `${img.naturalWidth}/${img.naturalHeight}`
    }
  }

  if (!videoLoading.value) {
    // 非首帧只做计数
    return
  }

  // Clear any pending retries and grace period
  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }

  // Reset all error states
  videoLoading.value = false
  videoError.value = false
  videoErrorMessage.value = ''
  videoRestarting.value = false
  retryCount = 0
  consecutiveErrors = 0
  clearFrameOverlay()

  // Auto-focus video container for immediate keyboard input
  const container = videoContainerRef.value
  if (container && typeof container.focus === 'function') {
    container.focus()
  }
}

function handleVideoError() {
  // 如果当前是 WebRTC 模式，忽略 MJPEG 错误（因为我们主动清空了 src）
  if (videoMode.value !== 'mjpeg') {
    return
  }

  // 如果正在切换模式，忽略错误（可能是 503 错误，因为后端已切换模式）
  if (isModeSwitching.value) {
    return
  }

  // 如果正在刷新视频，忽略清空 src 时触发的错误
  if (isRefreshingVideo) {
    return
  }

  // Count consecutive errors even in grace period
  consecutiveErrors++

  // If too many errors even in grace period, it's a real failure
  if (consecutiveErrors > MAX_CONSECUTIVE_ERRORS && gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
    videoRestarting.value = false
  }

  // If in grace period and not too many errors, ignore
  if (videoRestarting.value || gracePeriodTimeoutId !== null) {
    return
  }

  // Clear any pending retries to avoid duplicate attempts
  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }

  // Show loading state immediately
  videoLoading.value = true

  // Auto-retry with exponential backoff (infinite retry, capped delay)
  retryCount++
  const delay = BASE_RETRY_DELAY * Math.pow(1.5, Math.min(retryCount - 1, 5))

  retryTimeoutId = window.setTimeout(() => {
    retryTimeoutId = null
    refreshVideo()
  }, delay)
}

// Stream device monitoring handlers (UI-only; notifications/state are handled by useConsoleEvents)
function handleStreamDeviceLost(data: { device: string; reason: string }) {
  videoError.value = true
  videoErrorMessage.value = t('console.deviceLostDesc', { device: data.device, reason: data.reason })
}

function handleStreamRecovered(_data: { device: string }) {
  // Reset video error state
  videoError.value = false
  videoErrorMessage.value = ''
  // Refresh video stream
  refreshVideo()
}

async function handleAudioStateChanged(data: { streaming: boolean; device: string | null }) {
  if (!data.streaming) {
    // Audio stopped, disconnect
    unifiedAudio.disconnect()
    return
  }

  // Audio started streaming
  if (videoMode.value !== 'mjpeg' && webrtc.isConnected.value) {
    // WebRTC mode: check if we have an audio track
    if (!webrtc.audioTrack.value) {
      // No audio track - need to reconnect WebRTC to get one
      // This happens when audio was enabled after WebRTC session was created
      webrtc.disconnect()
      await new Promise(resolve => setTimeout(resolve, 300))
      await webrtc.connect()
      // After reconnect, the new session will have audio track
      // and the watch on audioTrack will add it to MediaStream
    } else {
      // We have audio track, ensure it's in MediaStream
      const currentStream = webrtcVideoRef.value?.srcObject as MediaStream | null
      if (currentStream && currentStream.getAudioTracks().length === 0) {
        currentStream.addTrack(webrtc.audioTrack.value)
      }
    }
  }

  // Connect unified audio when streaming starts (works for both MJPEG and WebRTC modes)
  // In MJPEG mode, this connects the WebSocket audio player
  // In WebRTC mode, this unmutes the video element
  await unifiedAudio.connect()
}

function handleStreamConfigChanging(data: any) {
  // Clear any existing retries and grace periods
  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }

  // Reset all counters and states
  videoRestarting.value = true
  videoLoading.value = true
  videoError.value = false
  retryCount = 0
  consecutiveErrors = 0

  // Reset FPS when config changes (backend will send new FPS via WebSocket)
  backendFps.value = 0

  toast.info(t('console.videoRestarting'), {
    description: data.reason === 'device_switch' ? t('console.deviceSwitching') : t('console.configChanging'),
    duration: 5000,
  })
}

async function handleStreamConfigApplied(data: any) {
  // Reset consecutive error counter for new config
  consecutiveErrors = 0

  // Start grace period to ignore transient errors
  gracePeriodTimeoutId = window.setTimeout(() => {
    gracePeriodTimeoutId = null
    consecutiveErrors = 0 // Also reset when grace period ends
  }, GRACE_PERIOD)

  // Refresh video based on current mode
  videoRestarting.value = false

  // 如果正在进行模式切换，不需要在这里处理（WebRTCReady 事件会处理）
  if (isModeSwitching.value) {
    console.log('[StreamConfigApplied] Mode switch in progress, waiting for WebRTCReady')
    return
  }

  if (videoMode.value !== 'mjpeg') {
    // In WebRTC mode, reconnect WebRTC (session was closed due to config change)
    const ready = await videoSession.waitForWebRTCReadyAny(3000)
    if (!ready) {
      console.warn('[WebRTC] Backend not ready after timeout (config change), attempting connection anyway')
    }
    switchToWebRTC(videoMode.value)
  } else {
    // In MJPEG mode, refresh the MJPEG stream
    refreshVideo()
  }

  toast.success(t('console.videoRestarted'), {
    description: `${data.device} - ${data.resolution[0]}x${data.resolution[1]} @ ${data.fps}fps`,
    duration: 3000,
  })
}

// 处理 WebRTC 就绪事件 - 这是后端真正准备好接受 WebRTC 连接的信号
function handleWebRTCReady(data: { codec: string; hardware: boolean; transition_id?: string }) {
  console.log(`[WebRTCReady] Backend ready: codec=${data.codec}, hardware=${data.hardware}, transition_id=${data.transition_id || '-'}`)
  videoSession.onWebRTCReady(data)
}

function handleStreamModeReady(data: { transition_id: string; mode: string }) {
  videoSession.onModeReady(data)
  videoRestarting.value = false
}

function handleStreamModeSwitching(data: { transition_id: string; to_mode: string; from_mode: string }) {
  // External mode switches: keep UI responsive and avoid black flash
  if (!isModeSwitching.value) {
    videoRestarting.value = true
    videoLoading.value = true
    captureFrameOverlay().catch(() => {})
  }
  videoSession.onModeSwitching(data)
}

function handleStreamStateChanged(data: any) {
  if (data.state === 'error') {
    videoError.value = true
    videoErrorMessage.value = t('console.streamError')
  }
}

function handleStreamStatsUpdate(data: any) {
  // Always update clients count in store (for MJPEG mode display)
  if (typeof data.clients === 'number') {
    systemStore.updateStreamClients(data.clients)
  }

  // Only update FPS from MJPEG stats when in MJPEG mode
  // In WebRTC mode, FPS is updated from WebRTC stats
  if (videoMode.value !== 'mjpeg') {
    // Still update clientsStats for display purposes, but don't touch backendFps
    if (data.clients_stat && typeof data.clients_stat === 'object') {
      clientsStats.value = data.clients_stat
    }
    return
  }

  if (data.clients_stat && typeof data.clients_stat === 'object') {
    clientsStats.value = data.clients_stat
    const myStats = data.clients_stat[myClientId]
    if (myStats) {
      backendFps.value = myStats.fps || 0
    } else {
      const fpsList = Object.values(data.clients_stat)
        .map((s: any) => s?.fps || 0)
        .filter(f => f > 0)
      backendFps.value = fpsList.length > 0 ? Math.min(...fpsList) : 0
    }
  } else {
    backendFps.value = 0
  }
}

// Track if we've received the initial device_info
let initialDeviceInfoReceived = false

function handleDeviceInfo(data: any) {
  systemStore.updateFromDeviceInfo(data)

  // Skip mode sync if video config is being changed
  // This prevents false-positive mode changes during config switching
  if (data.video?.config_changing) {
    return
  }

  // Sync video mode from server's stream_mode
  if (data.video?.stream_mode) {
    // Server returns: 'mjpeg', 'h264', 'h265', 'vp8', 'vp9', or 'webrtc'
    const serverStreamMode = data.video.stream_mode
    const serverMode = serverStreamMode === 'webrtc' ? 'h264' : serverStreamMode as VideoMode

    if (!initialDeviceInfoReceived) {
      // First device_info - initialize to server mode
      initialDeviceInfoReceived = true

      if (serverMode !== videoMode.value) {
        // Server mode differs from default, sync to server mode without calling setMode
        videoMode.value = serverMode
        if (serverMode !== 'mjpeg') {
          setTimeout(() => connectWebRTCOnly(serverMode), 100)
        } else {
          setTimeout(() => refreshVideo(), 100)
        }
      } else if (serverMode !== 'mjpeg') {
        // Server is in WebRTC mode and client default matches, connect WebRTC (no setMode)
        setTimeout(() => connectWebRTCOnly(serverMode), 100)
      } else if (serverMode === 'mjpeg') {
        // Server is in MJPEG mode and client default is also mjpeg, start MJPEG stream
        setTimeout(() => refreshVideo(), 100)
      }
    } else if (serverMode !== videoMode.value) {
      // Subsequent device_info with mode change - sync to server (no setMode)
      syncToServerMode(serverMode as VideoMode)
    }
  }
}

// Handle stream mode change event from server (WebSocket broadcast)
function handleStreamModeChanged(data: { mode: string; previous_mode: string }) {
  // Server returns: 'mjpeg', 'h264', 'h265', 'vp8', 'vp9', or 'webrtc'
  const newMode = data.mode === 'webrtc' ? 'h264' : data.mode as VideoMode

  // 如果正在进行模式切换，忽略这个事件（这是我们自己触发的切换产生的）
  if (isModeSwitching.value) {
    console.log('[StreamModeChanged] Mode switch in progress, ignoring event')
    return
  }

  // Show toast notification only if this is an external mode change
  toast.info(t('console.streamModeChanged'), {
    description: t('console.streamModeChangedDesc', { mode: data.mode.toUpperCase() }),
    duration: 5000,
  })

  // Switch to new mode (external sync handled by device_info after mode_ready)
  if (newMode !== videoMode.value) {
    syncToServerMode(newMode as VideoMode)
  }
}

// 标记是否正在刷新视频（用于忽略清空 src 时触发的 error 事件）
let isRefreshingVideo = false
// 标记是否正在切换模式（防止竞态条件和 503 错误）
const isModeSwitching = videoSession.localSwitching

function reloadPage() {
  window.location.reload()
}

function refreshVideo() {
  backendFps.value = 0
  videoError.value = false
  videoErrorMessage.value = ''

  // Update timestamp to force MJPEG reconnection via reactive URL
  isRefreshingVideo = true
  videoLoading.value = true
  mjpegTimestamp.value = Date.now()

  // For MJPEG streams, the 'load' event fires when first frame arrives
  // But on reconnection it may not fire again, so use a timeout as fallback
  setTimeout(() => {
    isRefreshingVideo = false
    // Clear loading state after timeout - if stream failed, error handler will show error
    if (videoLoading.value) {
      videoLoading.value = false
    }
  }, 1500)
}

// MJPEG URL with cache-busting timestamp (reactive)
// Only return valid URL when in MJPEG mode to prevent unnecessary requests
const mjpegTimestamp = ref(0) // Start with 0 to prevent initial load
const mjpegUrl = computed(() => {
  if (videoMode.value !== 'mjpeg') {
    return '' // Don't load MJPEG when in H264 mode
  }
  if (mjpegTimestamp.value === 0) {
    return '' // Don't load until refreshVideo() is called
  }
  return `${streamApi.getMjpegUrl(myClientId)}&t=${mjpegTimestamp.value}`
})

// Connect to WebRTC without changing server mode (for new clients joining existing session)
async function connectWebRTCOnly(codec: VideoMode = 'h264') {
  // 清除 MJPEG 相关的定时器
  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }
  retryCount = 0
  consecutiveErrors = 0

  // 停止 MJPEG 流 - 重置 timestamp 以停止请求
  mjpegTimestamp.value = 0
  if (videoRef.value) {
    videoRef.value.src = ''
    videoRef.value.removeAttribute('src')
  }

  videoLoading.value = true
  videoError.value = false
  videoErrorMessage.value = ''

  try {
    const success = await webrtc.connect()
    if (success) {
      toast.success(t('console.webrtcConnected'), {
        description: t('console.webrtcConnectedDesc'),
        duration: 3000,
      })

      // 强制重新绑定视频（即使 track 已存在）
      // 这解决了页面返回时视频不显示的问题
      await rebindWebRTCVideo()

      videoLoading.value = false
      videoMode.value = codec
      unifiedAudio.switchMode('webrtc')
    } else {
      throw new Error('WebRTC connection failed')
    }
  } catch {
    await fallbackToMJPEG(t('console.webrtcFailed'), t('console.fallingBackToMjpeg'))
  }
}

// 强制重新绑定 WebRTC 视频到视频元素
// 解决页面切换后视频不显示的问题
async function rebindWebRTCVideo() {
  if (!webrtcVideoRef.value) return

  // 先清空再重新绑定，确保浏览器重新渲染
  webrtcVideoRef.value.srcObject = null
  await nextTick()

  if (webrtc.videoTrack.value) {
    const stream = webrtc.getMediaStream()
    if (stream) {
      webrtcVideoRef.value.srcObject = stream
      try {
        await webrtcVideoRef.value.play()
      } catch {
        // AbortError is expected when switching modes quickly, ignore it
      }
      await waitForVideoFirstFrame(webrtcVideoRef.value, 2000)
      clearFrameOverlay()
    }
  }
}

// WebRTC video mode handling (switches server mode)
async function switchToWebRTC(codec: VideoMode = 'h264') {
  // 清除 MJPEG 相关的定时器，防止切换后重新加载 MJPEG
  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }
  retryCount = 0
  consecutiveErrors = 0

  // 停止 MJPEG 流 - 重置 timestamp 以停止请求
  mjpegTimestamp.value = 0
  if (videoRef.value) {
    videoRef.value.src = ''
  }

  videoLoading.value = true
  videoError.value = false
  videoErrorMessage.value = ''

  try {
    // Step 1: Disconnect existing WebRTC connection FIRST
    // This prevents ICE candidates from being sent to stale sessions
    // when backend closes sessions during codec switch
    if (webrtc.isConnected.value || webrtc.sessionId.value) {
      await webrtc.disconnect()
    }

    // Step 2: Call backend API to switch mode with specific codec
    const modeResp = await streamApi.setMode(codec)
    if (modeResp.transition_id) {
      videoSession.registerTransition(modeResp.transition_id)
      const [mode, webrtcReady] = await Promise.all([
        videoSession.waitForModeReady(modeResp.transition_id, 5000),
        videoSession.waitForWebRTCReady(modeResp.transition_id, 3000),
      ])

      if (mode && mode !== codec && mode !== 'webrtc') {
        console.warn(`[WebRTC] Backend mode_ready returned '${mode}', expected '${codec}', falling back`)
        throw new Error(`Backend switched to unexpected mode: ${mode}`)
      }

      if (!webrtcReady) {
        console.warn('[WebRTC] Backend not ready after timeout, attempting connection anyway')
      } else {
        console.log('[WebRTC] Backend ready signal received, connecting')
      }
    }

    // Step 3: Connect WebRTC with retry
    let retries = 3
    let success = false
    while (retries > 0 && !success) {
      success = await webrtc.connect()
      if (!success) {
        retries--
        if (retries > 0) {
          console.log(`[WebRTC] Connection failed, retrying (${retries} attempts left)`)
          await new Promise(resolve => setTimeout(resolve, 500))
        }
      }
    }
    if (success) {
      toast.success(t('console.webrtcConnected'), {
        description: t('console.webrtcConnectedDesc'),
        duration: 3000,
      })

      // 强制重新绑定视频
      await rebindWebRTCVideo()

      videoLoading.value = false

      // Step 4: Switch audio to WebRTC mode
      unifiedAudio.switchMode('webrtc')
    } else {
      throw new Error('WebRTC connection failed')
    }
  } catch {
    await fallbackToMJPEG(t('console.webrtcFailed'), t('console.fallingBackToMjpeg'), true)
  }
}

async function fallbackToMJPEG(reason: string, description?: string, force = false) {
  if (fallbackInProgress) return
  if (videoMode.value === 'mjpeg') return
  if (!force && (videoSession.localSwitching.value || videoSession.backendSwitching.value)) return

  fallbackInProgress = true
  videoError.value = true
  videoErrorMessage.value = reason
  videoLoading.value = false

  toast.error(reason, {
    description: description ?? '',
    duration: 5000,
  })

  videoMode.value = 'mjpeg'
  try {
    await switchToMJPEG()
  } finally {
    fallbackInProgress = false
  }
}

async function switchToMJPEG() {
  videoLoading.value = true
  videoError.value = false
  videoErrorMessage.value = ''

  // Step 1: Call backend API to switch mode FIRST
  // This ensures the MJPEG endpoint will accept our request
  try {
    const modeResp = await streamApi.setMode('mjpeg')
    if (modeResp.transition_id) {
      videoSession.registerTransition(modeResp.transition_id)
      const mode = await videoSession.waitForModeReady(modeResp.transition_id, 5000)
      if (mode && mode !== 'mjpeg') {
        console.warn(`[MJPEG] Backend mode_ready returned '${mode}', expected 'mjpeg'`)
      }
    }
  } catch (e) {
    console.error('Failed to switch to MJPEG mode:', e)
    // Continue anyway - the mode might already be correct
  }

  // Step 2: Disconnect WebRTC if connected
  if (webrtc.isConnected.value) {
    webrtc.disconnect()
  }

  // Clear WebRTC video
  if (webrtcVideoRef.value) {
    webrtcVideoRef.value.srcObject = null
  }

  // Step 3: Switch audio to WebSocket mode
  unifiedAudio.switchMode('ws')

  // Refresh MJPEG stream
  refreshVideo()
}

function syncToServerMode(mode: VideoMode) {
  if (videoSession.localSwitching.value || videoSession.backendSwitching.value) return
  if (mode === videoMode.value) return

  videoMode.value = mode
  localStorage.setItem('videoMode', mode)

  if (mode !== 'mjpeg') {
    connectWebRTCOnly(mode)
  } else {
    refreshVideo()
  }
}

// Handle video mode change
async function handleVideoModeChange(mode: VideoMode) {
  // 防止重复切换和竞态条件
  if (mode === videoMode.value) return
  if (!videoSession.tryStartLocalSwitch()) {
    console.log('[VideoMode] Switch throttled or in progress, ignoring')
    return
  }

  try {
    await captureFrameOverlay()

    // Reset mjpegTimestamp to 0 when switching away from MJPEG
    // This prevents mjpegUrl from returning a valid URL and stops MJPEG requests
    if (mode !== 'mjpeg') {
      mjpegTimestamp.value = 0
      // 完全清理 MJPEG 图片元素
      if (videoRef.value) {
        videoRef.value.src = ''
        videoRef.value.removeAttribute('src')
      }
      // 等待一小段时间确保浏览器取消 pending 请求
      await new Promise(resolve => setTimeout(resolve, 50))
    }

    videoMode.value = mode
    localStorage.setItem('videoMode', mode)

    // All WebRTC modes: h264, h265, vp8, vp9
    if (mode !== 'mjpeg') {
      await switchToWebRTC(mode)
    } else {
      await switchToMJPEG()
    }
  } finally {
    videoSession.endLocalSwitch()
  }
}

// Watch for WebRTC video track changes
watch(() => webrtc.videoTrack.value, async (track) => {
  if (track && webrtcVideoRef.value && videoMode.value !== 'mjpeg') {
    // 使用统一的重新绑定函数
    await rebindWebRTCVideo()
  }
})

// Watch for WebRTC audio track changes - update MediaStream when audio arrives
watch(() => webrtc.audioTrack.value, async (track) => {
  if (track && webrtcVideoRef.value && videoMode.value !== 'mjpeg') {
    // Audio track arrived, update the MediaStream to include it
    const currentStream = webrtcVideoRef.value.srcObject as MediaStream | null
    if (currentStream && currentStream.getAudioTracks().length === 0) {
      // Add audio track to existing stream
      currentStream.addTrack(track)
    }
  }
})

// Watch for WebRTC video element ref changes - set unified audio element
watch(webrtcVideoRef, (el) => {
  unifiedAudio.setWebRTCElement(el)
}, { immediate: true })

// Watch for WebRTC stats to update FPS display
// Watch the ref directly with deep: true to detect property changes
watch(webrtc.stats, (stats) => {
  if (videoMode.value !== 'mjpeg' && stats.framesPerSecond > 0) {
    backendFps.value = Math.round(stats.framesPerSecond)
    // WebRTC is receiving frames, set stream online
    systemStore.setStreamOnline(true)
    // Update aspect ratio from WebRTC video dimensions
    if (stats.frameWidth && stats.frameHeight) {
      videoAspectRatio.value = `${stats.frameWidth}/${stats.frameHeight}`
    }
  }
}, { deep: true })

// Watch for WebRTC connection state changes - auto-reconnect on disconnect
let webrtcReconnectTimeout: ReturnType<typeof setTimeout> | null = null
let webrtcReconnectFailures = 0
let fallbackInProgress = false
watch(() => webrtc.state.value, (newState, oldState) => {
  console.log('[WebRTC] State changed:', oldState, '->', newState)

  // Update stream online status based on WebRTC connection state
  if (videoMode.value !== 'mjpeg') {
    if (newState === 'connected') {
      systemStore.setStreamOnline(true)
      webrtcReconnectFailures = 0
    } else if (newState === 'disconnected' || newState === 'failed') {
      // Don't immediately set offline - wait for potential reconnect
      // The device_info event will eventually sync the correct state
    }
  }

  // Clear any pending reconnect
  if (webrtcReconnectTimeout) {
    clearTimeout(webrtcReconnectTimeout)
    webrtcReconnectTimeout = null
  }

  // Auto-reconnect when disconnected (but was previously connected)
  if (newState === 'disconnected' && oldState === 'connected' && videoMode.value !== 'mjpeg') {
    webrtcReconnectTimeout = setTimeout(async () => {
      if (videoMode.value !== 'mjpeg' && webrtc.state.value === 'disconnected') {
        try {
          const success = await webrtc.connect()
          if (!success) {
            webrtcReconnectFailures += 1
            if (webrtcReconnectFailures >= 2) {
              await fallbackToMJPEG(t('console.webrtcFailed'), t('console.fallingBackToMjpeg'))
            }
          }
        } catch {
          webrtcReconnectFailures += 1
          if (webrtcReconnectFailures >= 2) {
            await fallbackToMJPEG(t('console.webrtcFailed'), t('console.fallingBackToMjpeg'))
          }
        }
      }
    }, 1000)
  }

  if (newState === 'failed' && videoMode.value !== 'mjpeg') {
    webrtcReconnectFailures += 1
    if (webrtcReconnectFailures >= 1) {
      fallbackToMJPEG(t('console.webrtcFailed'), t('console.fallingBackToMjpeg')).catch(() => {})
    }
  }
})

async function toggleFullscreen() {
  if (!videoContainerRef.value) return
  if (!document.fullscreenElement) {
    await videoContainerRef.value.requestFullscreen()
    isFullscreen.value = true
  } else {
    await document.exitFullscreen()
    isFullscreen.value = false
  }
}

// Theme toggle
function toggleTheme() {
  isDark.value = !isDark.value
  document.documentElement.classList.toggle('dark', isDark.value)
  localStorage.setItem('theme', isDark.value ? 'dark' : 'light')
}

// Language toggle
function toggleLanguage() {
  const newLang = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN'
  setLanguage(newLang)
}

// Logout
async function logout() {
  await authStore.logout()
  router.push('/login')
}

// Change password function
async function handleChangePassword() {
  if (!newPassword.value || !confirmPassword.value) {
    toast.error(t('auth.passwordRequired'))
    return
  }

  if (newPassword.value !== confirmPassword.value) {
    toast.error(t('auth.passwordMismatch'))
    return
  }

  if (newPassword.value.length < 4) {
    toast.error(t('auth.passwordTooShort'))
    return
  }

  changingPassword.value = true
  try {
    await authApi.changePassword(currentPassword.value, newPassword.value)
    toast.success(t('auth.passwordChanged'))

    // Reset form and close dialog
    currentPassword.value = ''
    newPassword.value = ''
    confirmPassword.value = ''
    changePasswordDialogOpen.value = false
  } catch (e) {
    // Error toast is shown by API layer
    console.info('[ChangePassword] Failed:', e)
  } finally {
    changingPassword.value = false
  }
}

// ttyd (web terminal) functions
async function fetchTtydStatus() {
  try {
    ttydStatus.value = await extensionsApi.getTtydStatus()
  } catch {
    ttydStatus.value = null
  }
}

function openTerminal() {
  if (!ttydStatus.value?.running) return
  showTerminalDialog.value = true
}

function openTerminalInNewTab() {
  window.open('/api/terminal/', '_blank')
}

// ATX actions
async function handlePowerShort() {
  try {
    await atxApi.power('short')
    await systemStore.fetchAtxState()
  } catch {
    // ATX action failed
  }
}

async function handlePowerLong() {
  try {
    await atxApi.power('long')
    await systemStore.fetchAtxState()
  } catch {
    // ATX action failed
  }
}

async function handleReset() {
  try {
    await atxApi.power('reset')
    await systemStore.fetchAtxState()
  } catch {
    // ATX action failed
  }
}

async function handleWol(mac: string) {
  try {
    await atxConfigApi.sendWol(mac)
    toast.success(t('atx.wolSent'))
  } catch (e) {
    toast.error(t('atx.wolFailed'))
  }
}

// HID error handling - silently handle all HID errors
function handleHidError(_error: any, _operation: string) {
  // All HID errors are silently ignored
}

// HID channel selection: use WebRTC DataChannel when available, fallback to WebSocket
function sendKeyboardEvent(type: 'down' | 'up', key: number, modifiers?: { ctrl?: boolean; shift?: boolean; alt?: boolean; meta?: boolean }) {
  // In WebRTC mode with DataChannel ready, use DataChannel for lower latency
  if (videoMode.value !== 'mjpeg' && webrtc.dataChannelReady.value) {
    const event: HidKeyboardEvent = {
      type: type === 'down' ? 'keydown' : 'keyup',
      key,
      modifiers,
    }
    const sent = webrtc.sendKeyboard(event)
    if (sent) return
    // Fallback to WebSocket if DataChannel send failed
  }
  // Use WebSocket as fallback or for MJPEG mode
  hidApi.keyboard(type, key, modifiers).catch(err => handleHidError(err, `keyboard ${type}`))
}

function sendMouseEvent(data: { type: 'move' | 'move_abs' | 'down' | 'up' | 'scroll'; x?: number; y?: number; button?: 'left' | 'right' | 'middle'; scroll?: number }) {
  // In WebRTC mode with DataChannel ready, use DataChannel for lower latency
  if (videoMode.value !== 'mjpeg' && webrtc.dataChannelReady.value) {
    const event: HidMouseEvent = {
      type: data.type === 'move_abs' ? 'moveabs' : data.type,
      x: data.x,
      y: data.y,
      button: data.button === 'left' ? 0 : data.button === 'middle' ? 1 : data.button === 'right' ? 2 : undefined,
      scroll: data.scroll,
    }
    const sent = webrtc.sendMouse(event)
    if (sent) return
    // Fallback to WebSocket if DataChannel send failed
  }
  // Use WebSocket as fallback or for MJPEG mode
  hidApi.mouse(data).catch(err => handleHidError(err, `mouse ${data.type}`))
}

// Check if a key should be blocked (prevented from default behavior)
function shouldBlockKey(e: KeyboardEvent): boolean {
  // In fullscreen mode, block all keys for maximum capture
  if (isFullscreen.value) {
    return true
  }

  // Don't block critical browser shortcuts in non-fullscreen mode
  const key = e.key.toUpperCase()

  // Don't block Ctrl+W (close tab), Ctrl+T (new tab), Ctrl+N (new window)
  if (e.ctrlKey && ['W', 'T', 'N'].includes(key)) return false

  // Don't block F11 (browser fullscreen toggle)
  if (key === 'F11') return false

  // Don't block Alt+Tab (already can't capture it anyway)
  if (e.altKey && key === 'TAB') return false

  // Block everything else
  return true
}

// Keyboard/Mouse event handling
function handleKeyDown(e: KeyboardEvent) {
  const container = videoContainerRef.value
  if (!container) return

  // Check focus in non-fullscreen mode
  if (!isFullscreen.value && !container.contains(document.activeElement)) return

  // Try to block the key if appropriate
  if (shouldBlockKey(e)) {
    e.preventDefault()
    e.stopPropagation()
  }

  // Show hint for Meta key in non-fullscreen mode
  if (!isFullscreen.value && (e.metaKey || e.key === 'Meta')) {
    toast.info(t('console.metaKeyHint'), {
      description: t('console.metaKeyHintDesc'),
      duration: 3000,
    })
  }

  const keyName = e.key === ' ' ? 'Space' : e.key
  if (!pressedKeys.value.includes(keyName)) {
    pressedKeys.value = [...pressedKeys.value, keyName]
  }

  keyboardLed.value.capsLock = e.getModifierState('CapsLock')

  const modifiers = {
    ctrl: e.ctrlKey,
    shift: e.shiftKey,
    alt: e.altKey,
    meta: e.metaKey,
  }

  sendKeyboardEvent('down', e.keyCode, modifiers)
}

function handleKeyUp(e: KeyboardEvent) {
  const container = videoContainerRef.value
  if (!container) return

  // Check focus in non-fullscreen mode
  if (!isFullscreen.value && !container.contains(document.activeElement)) return

  // Try to block the key if appropriate
  if (shouldBlockKey(e)) {
    e.preventDefault()
    e.stopPropagation()
  }

  const keyName = e.key === ' ' ? 'Space' : e.key
  pressedKeys.value = pressedKeys.value.filter(k => k !== keyName)

  sendKeyboardEvent('up', e.keyCode)
}

function handleMouseMove(e: MouseEvent) {
  // Use the appropriate video element based on current mode (WebRTC for h264/h265/vp8/vp9, MJPEG for mjpeg)
  const videoElement = videoMode.value !== 'mjpeg' ? webrtcVideoRef.value : videoRef.value
  if (!videoElement) return

  if (mouseMode.value === 'absolute') {
    // Absolute mode: send absolute coordinates (0-32767 range)
    const rect = videoElement.getBoundingClientRect()
    const x = Math.round((e.clientX - rect.left) / rect.width * 32767)
    const y = Math.round((e.clientY - rect.top) / rect.height * 32767)

    mousePosition.value = { x, y }
    // Queue for throttled sending (absolute mode: just update pending position)
    pendingMouseMove = { type: 'move_abs', x, y }
    requestMouseMoveFlush()
  } else {
    // Relative mode: use movementX/Y when pointer is locked
    if (isPointerLocked.value) {
      const dx = e.movementX
      const dy = e.movementY

      // Only accumulate if there's actual movement
      if (dx !== 0 || dy !== 0) {
        // Accumulate deltas for throttled sending
        accumulatedDelta.x += dx
        accumulatedDelta.y += dy
        requestMouseMoveFlush()
      }

      // Update display position (accumulated delta for display only)
      mousePosition.value = {
        x: mousePosition.value.x + dx,
        y: mousePosition.value.y + dy,
      }
    }
  }
}

function hasPendingMouseMove(): boolean {
  if (mouseMode.value === 'absolute') return pendingMouseMove !== null
  return accumulatedDelta.x !== 0 || accumulatedDelta.y !== 0
}

function flushMouseMoveOnce(): boolean {
  if (mouseMode.value === 'absolute') {
    if (!pendingMouseMove) return false
    sendMouseEvent(pendingMouseMove)
    pendingMouseMove = null
    return true
  }

  if (accumulatedDelta.x === 0 && accumulatedDelta.y === 0) return false

  // Clamp to i8 range (-127 to 127)
  const clampedDx = Math.max(-127, Math.min(127, accumulatedDelta.x))
  const clampedDy = Math.max(-127, Math.min(127, accumulatedDelta.y))

  sendMouseEvent({ type: 'move', x: clampedDx, y: clampedDy })

  // Subtract sent amount (keep remainder for next send if clamped)
  accumulatedDelta.x -= clampedDx
  accumulatedDelta.y -= clampedDy
  return true
}

function scheduleMouseMoveFlush() {
  if (mouseFlushTimer !== null) return

  const interval = mouseMoveSendIntervalMs
  const now = Date.now()
  const elapsed = now - lastMouseMoveSendTime
  const delay = interval <= 0 ? 0 : Math.max(0, interval - elapsed)

  mouseFlushTimer = setTimeout(() => {
    mouseFlushTimer = null

    const burstLimit = mouseMoveSendIntervalMs <= 0 ? 8 : 1
    let sent = false
    for (let i = 0; i < burstLimit; i++) {
      if (!flushMouseMoveOnce()) break
      sent = true
      if (!hasPendingMouseMove()) break
    }
    if (sent) lastMouseMoveSendTime = Date.now()

    if (hasPendingMouseMove()) {
      scheduleMouseMoveFlush()
    }
  }, delay)
}

function requestMouseMoveFlush() {
  const interval = mouseMoveSendIntervalMs
  const now = Date.now()

  if (interval <= 0 || now - lastMouseMoveSendTime >= interval) {
    const burstLimit = interval <= 0 ? 8 : 1
    let sent = false
    for (let i = 0; i < burstLimit; i++) {
      if (!flushMouseMoveOnce()) break
      sent = true
      if (!hasPendingMouseMove()) break
    }
    if (sent) lastMouseMoveSendTime = Date.now()

    if (hasPendingMouseMove()) {
      scheduleMouseMoveFlush()
    }
    return
  }

  scheduleMouseMoveFlush()
}

// Track pressed mouse button for window-level mouseup handling
const pressedMouseButton = ref<'left' | 'right' | 'middle' | null>(null)

function handleMouseDown(e: MouseEvent) {
  e.preventDefault()

  // Auto-focus the video container to enable keyboard input
  const container = videoContainerRef.value
  if (container && document.activeElement !== container) {
    if (typeof container.focus === 'function') {
      container.focus()
    }
  }

  // In relative mode, request pointer lock on first click
  if (mouseMode.value === 'relative' && !isPointerLocked.value) {
    requestPointerLock()
    return
  }

  const button = e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle'
  pressedMouseButton.value = button
  sendMouseEvent({ type: 'down', button })
}

function handleMouseUp(e: MouseEvent) {
  e.preventDefault()
  handleMouseUpInternal(e.button)
}

// Window-level mouseup handler (catches releases outside the container)
function handleWindowMouseUp(e: MouseEvent) {
  if (pressedMouseButton.value !== null) {
    handleMouseUpInternal(e.button)
  }
}

function handleMouseUpInternal(rawButton: number) {
  if (mouseMode.value === 'relative' && !isPointerLocked.value) {
    pressedMouseButton.value = null
    return
  }

  const button = rawButton === 0 ? 'left' : rawButton === 2 ? 'right' : 'middle'

  // Only send if this button was actually pressed
  if (pressedMouseButton.value !== button) {
    return
  }

  pressedMouseButton.value = null
  sendMouseEvent({ type: 'up', button })
}

function handleWheel(e: WheelEvent) {
  e.preventDefault()
  const scroll = e.deltaY > 0 ? -1 : 1
  sendMouseEvent({ type: 'scroll', scroll })
}

function handleContextMenu(e: MouseEvent) {
  e.preventDefault()
}

// Pointer Lock API for relative mouse mode
function requestPointerLock() {
  const container = videoContainerRef.value
  if (!container) return

  container.requestPointerLock().catch((err: Error) => {
    toast.error(t('console.pointerLockFailed'), {
      description: err.message,
    })
  })
}

function exitPointerLock() {
  if (document.pointerLockElement) {
    document.exitPointerLock()
  }
}

function handlePointerLockChange() {
  const container = videoContainerRef.value
  isPointerLocked.value = document.pointerLockElement === container

  if (isPointerLocked.value) {
    // Reset mouse position display when locked
    mousePosition.value = { x: 0, y: 0 }
    toast.info(t('console.pointerLocked'), {
      description: t('console.pointerLockedDesc'),
      duration: 3000,
    })
  }
}

function handlePointerLockError() {
  isPointerLocked.value = false
}

function handleBlur() {
  pressedKeys.value = []
  // Release any pressed mouse button when window loses focus
  if (pressedMouseButton.value !== null) {
    const button = pressedMouseButton.value
    pressedMouseButton.value = null
    sendMouseEvent({ type: 'up', button })
  }
}

// Handle cursor visibility change from HidConfigPopover
function handleCursorVisibilityChange(e: Event) {
  const customEvent = e as CustomEvent<{ visible: boolean }>
  cursorVisible.value = customEvent.detail.visible
}

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

function setMouseMoveSendInterval(ms: number) {
  mouseMoveSendIntervalMs = clampMouseMoveSendIntervalMs(ms)

  if (mouseFlushTimer !== null) {
    clearTimeout(mouseFlushTimer)
    mouseFlushTimer = null
  }
  if (hasPendingMouseMove()) {
    requestMouseMoveFlush()
  }
}

function handleMouseSendIntervalChange(e: Event) {
  const customEvent = e as CustomEvent<{ intervalMs: number }>
  setMouseMoveSendInterval(customEvent.detail?.intervalMs)
}

function handleMouseSendIntervalStorage(e: StorageEvent) {
  if (e.key !== 'hidMouseThrottle') return
  setMouseMoveSendInterval(loadMouseMoveSendIntervalFromStorage())
}

// ActionBar handlers
// (MSD and Settings are now handled by ActionBar component directly)

function handleToggleVirtualKeyboard() {
  virtualKeyboardVisible.value = !virtualKeyboardVisible.value
}

// Virtual keyboard key event handlers
function handleVirtualKeyDown(key: string) {
  // Add to pressedKeys for InfoBar display
  if (!pressedKeys.value.includes(key)) {
    pressedKeys.value = [...pressedKeys.value, key]
  }
  // Toggle CapsLock state when virtual keyboard presses CapsLock
  if (key === 'CapsLock') {
    keyboardLed.value.capsLock = !keyboardLed.value.capsLock
  }
}

function handleVirtualKeyUp(key: string) {
  // Remove from pressedKeys
  pressedKeys.value = pressedKeys.value.filter(k => k !== key)
}

function handleToggleMouseMode() {
  // Exit pointer lock when switching away from relative mode
  if (mouseMode.value === 'relative' && isPointerLocked.value) {
    exitPointerLock()
  }

  mouseMode.value = mouseMode.value === 'absolute' ? 'relative' : 'absolute'
  pendingMouseMove = null
  accumulatedDelta = { x: 0, y: 0 }
  // Reset position when switching modes
  lastMousePosition.value = { x: 0, y: 0 }
  mousePosition.value = { x: 0, y: 0 }

  if (mouseMode.value === 'relative') {
    toast.info(t('console.relativeModeHint'), {
      description: t('console.relativeModeHintDesc'),
      duration: 5000,
    })
  }
}

// Lifecycle
onMounted(async () => {
  // 1. 先订阅 WebSocket 事件，再连接（内部会 connect）
  consoleEvents.subscribe()

  // 3. Watch WebSocket connection states and sync to store
  watch([wsConnected, wsNetworkError], ([connected, netError], [_prevConnected, prevNetError]) => {
    systemStore.updateWsConnection(connected, netError)

    // Auto-refresh video when network recovers (wsNetworkError: true -> false)
    if (prevNetError === true && netError === false && connected === true) {
      refreshVideo()
    }
  }, { immediate: true })

  watch([() => hidWs.connected.value, () => hidWs.networkError.value], ([connected, netError]) => {
    systemStore.updateHidWsConnection(connected, netError)
  }, { immediate: true })

  // 4. 其他初始化
  await systemStore.startStream().catch(() => {})
  await systemStore.fetchAllStates()

  window.addEventListener('keydown', handleKeyDown)
  window.addEventListener('keyup', handleKeyUp)
  window.addEventListener('blur', handleBlur)
  window.addEventListener('mouseup', handleWindowMouseUp)

  setMouseMoveSendInterval(loadMouseMoveSendIntervalFromStorage())

  // Listen for cursor visibility changes from HidConfigPopover
  window.addEventListener('hidCursorVisibilityChanged', handleCursorVisibilityChange as EventListener)
  window.addEventListener('hidMouseSendIntervalChanged', handleMouseSendIntervalChange as EventListener)
  window.addEventListener('storage', handleMouseSendIntervalStorage)

  // Pointer Lock event listeners
  document.addEventListener('pointerlockchange', handlePointerLockChange)
  document.addEventListener('pointerlockerror', handlePointerLockError)

  document.addEventListener('fullscreenchange', () => {
    isFullscreen.value = !!document.fullscreenElement
  })

  const storedTheme = localStorage.getItem('theme')
  if (storedTheme === 'dark' || (!storedTheme && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
    isDark.value = true
    document.documentElement.classList.add('dark')
  }

  // Fetch ttyd status initially and poll every 10 seconds
  fetchTtydStatus()
  ttydPollInterval = setInterval(fetchTtydStatus, 10000)

  // Note: Video mode is now synced from server via device_info event
  // The handleDeviceInfo function will automatically switch to the server's mode
  // localStorage preference is only used when server mode matches
})

onUnmounted(() => {
  // Reset initial device info flag
  initialDeviceInfoReceived = false

  // Clear mouse flush timer
  if (mouseFlushTimer !== null) {
    clearTimeout(mouseFlushTimer)
    mouseFlushTimer = null
  }

  // Clear ttyd poll interval
  if (ttydPollInterval) {
    clearInterval(ttydPollInterval)
    ttydPollInterval = null
  }

  // Clear all timers
  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }
  videoSession.clearWaiters()

  // Reset counters
  retryCount = 0

  consoleEvents.unsubscribe()
  consecutiveErrors = 0

  // Disconnect WebRTC if connected
  if (webrtc.isConnected.value) {
    webrtc.disconnect()
  }

  // Exit pointer lock if active
  exitPointerLock()

  window.removeEventListener('keydown', handleKeyDown)
  window.removeEventListener('keyup', handleKeyUp)
  window.removeEventListener('blur', handleBlur)
  window.removeEventListener('mouseup', handleWindowMouseUp)
  window.removeEventListener('hidCursorVisibilityChanged', handleCursorVisibilityChange as EventListener)
  window.removeEventListener('hidMouseSendIntervalChanged', handleMouseSendIntervalChange as EventListener)
  window.removeEventListener('storage', handleMouseSendIntervalStorage)

  // Remove pointer lock event listeners
  document.removeEventListener('pointerlockchange', handlePointerLockChange)
  document.removeEventListener('pointerlockerror', handlePointerLockError)
})
</script>

<template>
  <div class="h-screen flex flex-col bg-background">
    <!-- Header -->
    <header class="shrink-0 border-b border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div class="px-4">
        <div class="h-14 flex items-center justify-between">
          <!-- Left: Logo -->
          <div class="flex items-center gap-6">
            <div class="flex items-center gap-2">
              <Monitor class="h-6 w-6 text-primary" />
              <span class="font-bold text-lg">One-KVM</span>
            </div>
          </div>

          <!-- Right: Status Cards + User Menu -->
          <div class="flex items-center gap-2">
            <div class="hidden md:flex items-center gap-2">
              <!-- Video Status -->
              <StatusCard
                :title="t('statusCard.video')"
                type="video"
                :status="videoStatus"
                :quick-info="videoQuickInfo"
                :error-message="videoErrorMessage"
                :details="videoDetails"
              />

              <!-- Audio Status -->
              <StatusCard
                v-if="systemStore.audio?.available"
                :title="t('statusCard.audio')"
                type="audio"
                :status="audioStatus"
                :quick-info="audioQuickInfo"
                :error-message="audioErrorMessage"
                :details="audioDetails"
              />

              <!-- HID Status -->
              <StatusCard
                :title="t('statusCard.hid')"
                type="hid"
                :status="hidStatus"
                :quick-info="hidQuickInfo"
                :details="hidDetails"
              />

              <!-- MSD Status - Hidden when CH9329 backend (no USB gadget support) -->
              <StatusCard
                v-if="systemStore.msd?.available && systemStore.hid?.backend !== 'ch9329'"
                :title="t('statusCard.msd')"
                type="msd"
                :status="msdStatus"
                :quick-info="msdQuickInfo"
                :error-message="msdErrorMessage"
                :details="msdDetails"
                hover-align="end"
              />
            </div>

            <!-- Separator -->
            <div class="h-6 w-px bg-slate-200 dark:bg-slate-700 hidden md:block mx-1" />

            <!-- Theme Toggle -->
            <Button variant="ghost" size="icon" class="h-8 w-8 hidden md:flex" @click="toggleTheme">
              <Sun v-if="isDark" class="h-4 w-4" />
              <Moon v-else class="h-4 w-4" />
            </Button>

            <!-- Language Toggle -->
            <Button variant="ghost" size="icon" class="h-8 w-8 hidden md:flex" @click="toggleLanguage">
              <Languages class="h-4 w-4" />
            </Button>

            <!-- User Menu -->
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button variant="outline" size="sm" class="gap-1.5">
                  <span class="text-xs max-w-[100px] truncate">{{ authStore.user || 'admin' }}</span>
                  <ChevronDown class="h-3.5 w-3.5" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem class="md:hidden" @click="toggleTheme">
                  <Sun v-if="isDark" class="h-4 w-4 mr-2" />
                  <Moon v-else class="h-4 w-4 mr-2" />
                  {{ isDark ? t('settings.lightMode') : t('settings.darkMode') }}
                </DropdownMenuItem>
                <DropdownMenuItem class="md:hidden" @click="toggleLanguage">
                  <Languages class="h-4 w-4 mr-2" />
                  {{ locale === 'zh-CN' ? 'English' : '中文' }}
                </DropdownMenuItem>
                <DropdownMenuSeparator class="md:hidden" />
                <DropdownMenuItem @click="changePasswordDialogOpen = true">
                  <KeyRound class="h-4 w-4 mr-2" />
                  {{ t('auth.changePassword') }}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem @click="logout">
                  <LogOut class="h-4 w-4 mr-2" />
                  {{ t('auth.logout') }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>

        <!-- Mobile Status Row -->
        <div class="md:hidden pb-2">
          <div class="flex items-center gap-2 overflow-x-auto">
            <div class="shrink-0">
              <StatusCard
                :title="t('statusCard.video')"
                type="video"
                :status="videoStatus"
                :quick-info="videoQuickInfo"
                :error-message="videoErrorMessage"
                :details="videoDetails"
                compact
              />
            </div>

            <div v-if="systemStore.audio?.available" class="shrink-0">
              <StatusCard
                :title="t('statusCard.audio')"
                type="audio"
                :status="audioStatus"
                :quick-info="audioQuickInfo"
                :error-message="audioErrorMessage"
                :details="audioDetails"
                compact
              />
            </div>

            <div class="shrink-0">
              <StatusCard
                :title="t('statusCard.hid')"
                type="hid"
                :status="hidStatus"
                :quick-info="hidQuickInfo"
                :details="hidDetails"
                compact
              />
            </div>

            <div v-if="systemStore.msd?.available && systemStore.hid?.backend !== 'ch9329'" class="shrink-0">
              <StatusCard
                :title="t('statusCard.msd')"
                type="msd"
                :status="msdStatus"
                :quick-info="msdQuickInfo"
                :error-message="msdErrorMessage"
                :details="msdDetails"
                hover-align="end"
                compact
              />
            </div>
          </div>
        </div>
      </div>
    </header>

    <!-- ActionBar -->
    <ActionBar
      :mouse-mode="mouseMode"
      :video-mode="videoMode"
      :ttyd-running="ttydStatus?.running"
      @toggle-fullscreen="toggleFullscreen"
      @toggle-stats="statsSheetOpen = true"
      @toggle-virtual-keyboard="handleToggleVirtualKeyboard"
      @toggle-mouse-mode="handleToggleMouseMode"
      @update:video-mode="handleVideoModeChange"
      @power-short="handlePowerShort"
      @power-long="handlePowerLong"
      @reset="handleReset"
      @wol="handleWol"
      @open-terminal="openTerminal"
    />

    <!-- Main Video Area -->
    <div class="flex-1 overflow-hidden relative">
      <!-- Dot Pattern Background -->
      <div
        class="absolute inset-0 bg-slate-100/80 dark:bg-slate-800/40 opacity-80"
        style="
          background-image: radial-gradient(circle, rgb(148 163 184 / 0.4) 1px, transparent 1px);
          background-size: 20px 20px;
        "
      />

      <!-- Video Container -->
      <div class="relative h-full w-full flex items-center justify-center p-2 sm:p-4">
        <div
          ref="videoContainerRef"
          class="relative bg-black overflow-hidden flex items-center justify-center"
          :style="{
            aspectRatio: videoAspectRatio ?? '16/9',
            maxWidth: '100%',
            maxHeight: '100%',
            minWidth: '320px',
            minHeight: '180px',
          }"
          :class="{
            'opacity-60': videoLoading || videoError,
            'cursor-crosshair': cursorVisible,
            'cursor-none': !cursorVisible
          }"
          tabindex="0"
          @mousemove="handleMouseMove"
          @mousedown="handleMouseDown"
          @mouseup="handleMouseUp"
          @wheel.prevent="handleWheel"
          @contextmenu="handleContextMenu"
        >
          <!-- MJPEG Stream -->
          <img
            v-show="videoMode === 'mjpeg'"
            ref="videoRef"
            :src="mjpegUrl"
            class="w-full h-full object-contain"
            :alt="t('console.videoAlt')"
            @load="handleVideoLoad"
            @error="handleVideoError"
          />

          <!-- WebRTC Stream (H.264/H.265/VP8/VP9) -->
          <!-- Note: muted is controlled by unifiedAudio, not hardcoded -->
          <video
            v-show="videoMode !== 'mjpeg'"
            ref="webrtcVideoRef"
            class="w-full h-full object-contain"
            autoplay
            playsinline
          />

          <!-- Last-frame overlay (reduces black flash when switching modes) -->
          <img
            v-if="frameOverlayUrl"
            :src="frameOverlayUrl"
            class="absolute inset-0 w-full h-full object-contain pointer-events-none"
            alt=""
          />

          <!-- Loading Overlay with smooth transition and visual feedback -->
          <Transition name="fade">
            <div
              v-if="videoLoading"
              class="absolute inset-0 flex flex-col items-center justify-center bg-black/70 backdrop-blur-sm transition-opacity duration-300"
            >
              <!-- Animated scan line for visual feedback -->
              <div class="absolute inset-0 overflow-hidden pointer-events-none">
                <div class="absolute w-full h-0.5 bg-gradient-to-r from-transparent via-primary/40 to-transparent animate-pulse" style="top: 50%; animation-duration: 1.5s;" />
              </div>

              <Spinner class="h-16 w-16 text-white mb-4" />
              <p class="text-white/90 text-lg font-medium">
                {{ videoRestarting ? t('console.videoRestarting') : t('console.connecting') }}
              </p>
              <p class="text-white/50 text-sm mt-2">
                {{ t('console.pleaseWait') }}
              </p>
            </div>
          </Transition>

          <!-- Error Overlay with smooth transition and detailed info -->
          <Transition name="fade">
            <div
              v-if="videoError && !videoLoading"
              class="absolute inset-0 flex flex-col items-center justify-center bg-black/85 text-white gap-4 transition-opacity duration-300 p-4"
            >
              <MonitorOff class="h-16 w-16 text-slate-400" />
              <div class="text-center max-w-md">
                <p class="font-medium text-lg mb-2">{{ t('console.connectionFailed') }}</p>
                <p class="text-sm text-slate-300 mb-3">{{ t('console.connectionFailedDesc') }}</p>
                <!-- Expandable error details -->
                <div v-if="videoErrorMessage" class="bg-slate-800/60 rounded-lg p-3 text-left">
                  <p class="text-xs text-slate-400 mb-1">{{ t('console.errorDetails') }}:</p>
                  <p class="text-sm text-slate-300 font-mono break-all">{{ videoErrorMessage }}</p>
                </div>
              </div>
              <div class="flex gap-2">
                <Button variant="secondary" size="sm" @click="reloadPage">
                  <RefreshCw class="h-4 w-4 mr-2" />
                  {{ t('console.reconnect') }}
                </Button>
              </div>
            </div>
          </Transition>
        </div>
      </div>
    </div>

    <!-- Virtual Keyboard - Above InfoBar when attached, or in body when floating -->
    <Teleport :to="virtualKeyboardAttached ? '#keyboard-anchor' : 'body'" :disabled="virtualKeyboardAttached">
      <VirtualKeyboard
        v-if="virtualKeyboardVisible"
        v-model:visible="virtualKeyboardVisible"
        v-model:attached="virtualKeyboardAttached"
        @key-down="handleVirtualKeyDown"
        @key-up="handleVirtualKeyUp"
      />
    </Teleport>

    <!-- Anchor for attached keyboard -->
    <div id="keyboard-anchor"></div>

    <!-- InfoBar (Status Bar) -->
    <InfoBar
      :pressed-keys="pressedKeys"
      :caps-lock="keyboardLed.capsLock"
      :mouse-position="mousePosition"
      :debug-mode="false"
    />

    <!-- Stats Sheet -->
    <StatsSheet
      v-model:open="statsSheetOpen"
      :video-mode="videoMode"
      :mjpeg-fps="backendFps"
      :ws-latency="0"
      :webrtc-stats="webrtc.stats.value"
    />

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

    <!-- Change Password Dialog -->
    <Dialog v-model:open="changePasswordDialogOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('auth.changePassword') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-4 py-4">
          <div class="space-y-2">
            <Label for="currentPassword">{{ t('auth.currentPassword') }}</Label>
            <Input
              id="currentPassword"
              v-model="currentPassword"
              type="password"
              :placeholder="t('auth.currentPasswordPlaceholder')"
            />
          </div>
          <div class="space-y-2">
            <Label for="newPassword">{{ t('auth.newPassword') }}</Label>
            <Input
              id="newPassword"
              v-model="newPassword"
              type="password"
              :placeholder="t('auth.newPasswordPlaceholder')"
            />
          </div>
          <div class="space-y-2">
            <Label for="confirmPassword">{{ t('auth.confirmPassword') }}</Label>
            <Input
              id="confirmPassword"
              v-model="confirmPassword"
              type="password"
              :placeholder="t('auth.confirmPasswordPlaceholder')"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="changePasswordDialogOpen = false">
            {{ t('common.cancel') }}
          </Button>
          <Button @click="handleChangePassword" :disabled="changingPassword">
            <Loader2 v-if="changingPassword" class="h-4 w-4 mr-2 animate-spin" />
            {{ t('common.confirm') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

  </div>
</template>

<style scoped>
/* Smooth fade transition for video overlays */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
