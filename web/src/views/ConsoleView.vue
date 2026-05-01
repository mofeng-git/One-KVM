<script setup lang="ts">
import { ref, onMounted, onUnmounted, onActivated, onDeactivated, computed, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { useConfigStore } from '@/stores/config'
import { useAuthStore } from '@/stores/auth'
import { useWebSocket } from '@/composables/useWebSocket'
import { useConsoleEvents } from '@/composables/useConsoleEvents'
import { useHidWebSocket } from '@/composables/useHidWebSocket'
import { useWebRTC } from '@/composables/useWebRTC'
import { useVideoSession } from '@/composables/useVideoSession'
import { getUnifiedAudio } from '@/composables/useUnifiedAudio'
import { streamApi, hidApi, atxApi, atxConfigApi, authApi } from '@/api'
import { CanonicalKey, HidBackend } from '@/types/generated'
import type { HidKeyboardEvent, HidMouseEvent } from '@/types/hid'
import { keyboardEventToCanonicalKey, updateModifierMaskForKey } from '@/lib/keyboardMappings'
import { toast } from 'vue-sonner'
import { generateUUID } from '@/lib/utils'
import { formatFpsValue } from '@/lib/fps'
import type { VideoMode } from '@/components/VideoConfigPopover.vue'

import StatusCard, { type StatusDetail } from '@/components/StatusCard.vue'
import ActionBar from '@/components/ActionBar.vue'
import InfoBar from '@/components/InfoBar.vue'
import VirtualKeyboard from '@/components/VirtualKeyboard.vue'
import StatsSheet from '@/components/StatsSheet.vue'
import LanguageToggleButton from '@/components/LanguageToggleButton.vue'
import BrandMark from '@/components/BrandMark.vue'
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
  MonitorOff,
  RefreshCw,
  LogOut,
  Sun,
  Moon,
  ChevronDown,
  Terminal,
  ExternalLink,
  KeyRound,
  Loader2,
} from 'lucide-vue-next'

const { t, te } = useI18n()
const router = useRouter()
const systemStore = useSystemStore()
const configStore = useConfigStore()
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
})

const videoMode = ref<VideoMode>('mjpeg')

const videoRef = ref<HTMLImageElement | null>(null)
const webrtcVideoRef = ref<HTMLVideoElement | null>(null)
const videoContainerRef = ref<HTMLDivElement | null>(null)
const isFullscreen = ref(false)
const videoLoading = ref(true)
const videoError = ref(false)
const videoErrorMessage = ref('')
const videoRestarting = ref(false)
const mjpegFrameReceived = ref(false)

/** From `stream.state_changed`: ok | no_signal | device_lost | device_busy */
type StreamSignalState = 'ok' | 'no_signal' | 'device_lost' | 'device_busy'
const streamSignalState = ref<StreamSignalState>('ok')
const streamSignalReason = ref<string | null>(null)
const streamNextRetryMs = ref<number | null>(null)

const videoAspectRatio = ref<string | null>(null)

const backendFps = ref(0)

interface ClientStat {
  id: string
  fps: number
  connected_secs: number
}
const clientsStats = ref<Record<string, ClientStat>>({})

const myClientId = generateUUID()

const mouseMode = ref<'absolute' | 'relative'>('absolute')
const pressedKeys = ref<CanonicalKey[]>([])
const keyboardLed = computed(() => ({
  capsLock: systemStore.hid?.ledState.capsLock ?? false,
  numLock: systemStore.hid?.ledState.numLock ?? false,
  scrollLock: systemStore.hid?.ledState.scrollLock ?? false,
}))
const keyboardLedEnabled = computed(() => systemStore.hid?.keyboardLedsEnabled ?? false)
const activeModifierMask = ref(0)
const mousePosition = ref({ x: 0, y: 0 })
const lastMousePosition = ref({ x: 0, y: 0 })
const isPointerLocked = ref(false)

/** Local overlay crosshair position (px, relative to video container); HID uses mousePosition separately */
const localCrosshairPos = ref<{ x: number; y: number } | null>(null)

const DEFAULT_MOUSE_MOVE_SEND_INTERVAL_MS = 16
let mouseMoveSendIntervalMs = DEFAULT_MOUSE_MOVE_SEND_INTERVAL_MS
let mouseFlushTimer: ReturnType<typeof setTimeout> | null = null
let lastMouseMoveSendTime = 0
let pendingMouseMove: { type: 'move' | 'move_abs'; x: number; y: number } | null = null
let accumulatedDelta = { x: 0, y: 0 }

const cursorVisible = ref(localStorage.getItem('hidShowCursor') !== 'false')
let interactionListenersBound = false
const isConsoleActive = ref(false)

function syncMouseModeFromConfig() {
  const mouseAbsolute = configStore.hid?.mouse_absolute
  if (typeof mouseAbsolute !== 'boolean') return
  const nextMode: 'absolute' | 'relative' = mouseAbsolute ? 'absolute' : 'relative'
  if (mouseMode.value !== nextMode) {
    mouseMode.value = nextMode
  }
}

const virtualKeyboardVisible = ref(false)
const virtualKeyboardAttached = ref(true)
const statsSheetOpen = ref(false)
const virtualKeyboardConsumerEnabled = computed(() => {
  const hid = configStore.hid
  if (!hid) return true
  if (hid.backend !== HidBackend.Otg) return true
  return hid.otg_functions?.consumer !== false
})

const changePasswordDialogOpen = ref(false)
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const changingPassword = ref(false)

const ttydStatus = ref<{ available: boolean; running: boolean } | null>(null)
const showTerminalDialog = ref(false)

const isDark = ref(document.documentElement.classList.contains('dark'))

const videoStatus = computed<'connected' | 'connecting' | 'disconnected' | 'error'>(() => {
  if (wsNetworkError.value) return 'connecting'

  if (videoError.value) return 'error'
  if (videoLoading.value) return 'connecting'
  if (videoMode.value !== 'mjpeg') {
    if (webrtc.isConnecting.value) return 'connecting'
    if (webrtc.isConnected.value) return 'connected'
  }
  // MJPEG: check if frames have actually arrived (frontend-side detection)
  // This is more reliable than relying on stream.online from backend,
  // which can be stale due to the debounce delay in device_info broadcaster.
  // Also handles browsers that don't fire img.onload for multipart MJPEG streams.
  if (videoMode.value === 'mjpeg' && mjpegFrameReceived.value) return 'connected'
  if (systemStore.stream?.online) return 'connected'
  return 'disconnected'
})

function getResolutionShortName(width: number, height: number): string {
  if (height === 2160 || (height === 2160 && width === 4096)) return '4K'
  if (height === 1440) return '2K'
  if (height === 1080) return '1080p'
  if (height === 720) return '720p'
  if (height === 768) return '768p'
  if (height === 600) return '600p'
  if (height === 1024 && width === 1280) return '1024p'
  if (height === 960) return '960p'
  return `${height}p`
}

const videoQuickInfo = computed(() => {
  const stream = systemStore.stream
  if (!stream?.resolution) return ''
  const resShort = getResolutionShortName(stream.resolution[0], stream.resolution[1])
  return `${resShort} ${formatFpsValue(backendFps.value)}fps`
})

const videoDetails = computed<StatusDetail[]>(() => {
  const stream = systemStore.stream
  if (!stream) return []
  const receivedFps = backendFps.value

  const inputFmt = stream.format || 'MJPEG'
  const outputFmt = videoMode.value === 'mjpeg' ? 'MJPEG' : `${videoMode.value.toUpperCase()} (WebRTC)`
  const formatDisplay = inputFmt === outputFmt ? inputFmt : `${inputFmt} → ${outputFmt}`

  const fpsDisplay = `${formatFpsValue(stream.targetFps ?? 0)} / ${formatFpsValue(receivedFps)}`
  const fpsStatus: StatusDetail['status'] = receivedFps > 5 ? 'ok' : receivedFps > 0 ? 'warning' : undefined

  return [
    { label: t('statusCard.device'), value: stream.device || '-' },
    { label: t('statusCard.format'), value: formatDisplay },
    { label: t('statusCard.resolution'), value: stream.resolution ? `${stream.resolution[0]}x${stream.resolution[1]}` : '-' },
    { label: t('statusCard.fps'), value: fpsDisplay, status: fpsStatus },
  ]
})

const hidStatus = computed<'connected' | 'connecting' | 'disconnected' | 'error'>(() => {
  const hid = systemStore.hid
  if (hid?.errorCode === 'udc_not_configured') return 'disconnected'
  if (hid?.error) return 'error'

  if (videoMode.value !== 'mjpeg') {
    if (webrtc.dataChannelReady.value) return 'connected'
    if (webrtc.isConnecting.value) return 'connecting'
    if (webrtc.isConnected.value) return 'connecting'
  }

  if (hidWs.networkError.value) return 'connecting'

  if (!hidWs.connected.value) return 'disconnected'

  if (hidWs.hidUnavailable.value) return 'disconnected'

  if (hid?.available && hid.online) return 'connected'
  if (hid?.available && hid.initialized) return 'connecting'
  return 'disconnected'
})

const hidQuickInfo = computed(() => {
  const hid = systemStore.hid
  if (!hid?.available) return ''
  return mouseMode.value === 'absolute' ? t('statusCard.absolute') : t('statusCard.relative')
})

function extractCh9329Command(reason?: string | null): string | null {
  if (!reason) return null
  const match = reason.match(/cmd 0x([0-9a-f]{2})/i)
  const cmd = match?.[1]
  return cmd ? `0x${cmd.toUpperCase()}` : null
}

function hidErrorHint(errorCode?: string | null, backend?: string | null, reason?: string | null): string {
  const ch9329Command = extractCh9329Command(reason)

  switch (errorCode) {
    case 'udc_not_configured':
      return t('hid.errorHints.udcNotConfigured')
    case 'disabled':
      return t('hid.errorHints.disabled')
    case 'enoent':
      return t('hid.errorHints.hidDeviceMissing')
    case 'not_opened':
      return t('hid.errorHints.notOpened')
    case 'port_not_found':
      return t('hid.errorHints.portNotFound')
    case 'invalid_config':
      return t('hid.errorHints.invalidConfig')
    case 'no_response':
      return t(ch9329Command ? 'hid.errorHints.noResponseWithCmd' : 'hid.errorHints.noResponse', {
        cmd: ch9329Command ?? '',
      })
    case 'protocol_error':
    case 'invalid_response':
      return t('hid.errorHints.protocolError')
    case 'enxio':
    case 'enodev':
      return t('hid.errorHints.deviceDisconnected')
    case 'eio':
    case 'epipe':
    case 'eshutdown':
    case 'io_error':
    case 'write_failed':
    case 'read_failed':
      if (backend === 'otg') return t('hid.errorHints.otgIoError')
      if (backend === 'ch9329') return t('hid.errorHints.ch9329IoError')
      return t('hid.errorHints.ioError')
    case 'serial_error':
      return t('hid.errorHints.serialError')
    case 'init_failed':
      return t('hid.errorHints.initFailed')
    case 'shutdown':
      return t('hid.errorHints.shutdown')
    default:
      return ''
  }
}

function buildHidErrorMessage(reason?: string | null, errorCode?: string | null, backend?: string | null): string {
  if (!reason && !errorCode) return ''
  const hint = hidErrorHint(errorCode, backend, reason)
  if (hint) return hint
  if (reason) return reason
  return hint || t('common.error')
}

const hidErrorMessage = computed(() => {
  const hid = systemStore.hid
  return buildHidErrorMessage(hid?.error, hid?.errorCode, hid?.backend)
})

const hidDetails = computed<StatusDetail[]>(() => {
  const hid = systemStore.hid
  if (!hid) return []
  const errorMessage = buildHidErrorMessage(hid.error, hid.errorCode, hid.backend)
  const hidErrorStatus: StatusDetail['status'] =
    hid.errorCode === 'udc_not_configured' ? 'warning' : 'error'

  const details: StatusDetail[] = []

  const backendStr = hid.backend || t('common.unknown')
  const deviceStr = hid.device ? ` @ ${hid.device}` : ''
  details.push({ label: t('statusCard.backend'), value: `${backendStr}${deviceStr}` })

  if (errorMessage) {
    const codeSuffix = hid.errorCode ? ` (${hid.errorCode})` : ''
    details.push({ label: t('common.error'), value: `${errorMessage}${codeSuffix}`, status: hidErrorStatus })
  } else if (hid.online) {
    details.push({ label: t('statusCard.currentMode'), value: mouseMode.value === 'absolute' ? t('statusCard.absolute') : t('statusCard.relative'), status: 'ok' })
    if (hid.keyboardLedsEnabled) {
      details.push({
        label: t('settings.otgKeyboardLeds'),
        value: `Caps:${hid.ledState.capsLock ? t('common.on') : t('common.off')} Num:${hid.ledState.numLock ? t('common.on') : t('common.off')} Scroll:${hid.ledState.scrollLock ? t('common.on') : t('common.off')}`,
        status: 'ok',
      })
    }
  }

  let channelValue: string
  let channelStatus: StatusDetail['status']
  if (videoMode.value !== 'mjpeg') {
    if (webrtc.dataChannelReady.value) {
      channelValue = 'DataChannel (WebRTC)'
      channelStatus = 'ok'
    } else if (webrtc.isConnecting.value || webrtc.isConnected.value) {
      channelValue = 'DataChannel'
      channelStatus = 'warning'
    } else {
      channelValue = 'WebSocket (fallback)'
      channelStatus = hidWs.connected.value ? 'ok' : 'warning'
    }
  } else {
    channelValue = 'WebSocket'
    channelStatus = hidWs.connected.value ? 'ok' : 'warning'
  }
  if (videoMode.value === 'mjpeg' || !webrtc.dataChannelReady.value) {
    if (hidWs.networkError.value) {
      channelValue += ` (${t('statusCard.networkError')})`
      channelStatus = 'warning'
    } else if (!hidWs.connected.value) {
      channelValue += ` (${t('statusCard.disconnected')})`
      channelStatus = 'warning'
    } else if (hidWs.hidUnavailable.value) {
      channelValue += ` (${t('statusCard.hidUnavailable')})`
      channelStatus = 'warning'
    }
  }
  details.push({ label: t('statusCard.channel'), value: channelValue, status: channelStatus })

  return details
})

const audioStatus = computed<'connected' | 'connecting' | 'disconnected' | 'error'>(() => {
  const audio = systemStore.audio
  if (!audio?.available) return 'disconnected'
  if (audio.error) return 'error'
  if (audio.streaming) return 'connected'
  return 'disconnected'
})

function translateAudioQuality(quality: string | undefined): string {
  if (!quality) return t('common.unknown')
  const qualityLower = quality.toLowerCase()
  if (qualityLower === 'voice') return t('actionbar.qualityVoice')
  if (qualityLower === 'balanced') return t('actionbar.qualityBalanced')
  if (qualityLower === 'high') return t('actionbar.qualityHigh')
  return quality
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

  if (msd.mode === 'image') {
    details.push({
      label: t('statusCard.msdCurrentImage'),
      value: msd.imageId || t('statusCard.msdNoImage')
    })
  }

  return details
})

const webrtcLoadingMessage = computed(() => {
  if (videoMode.value === 'mjpeg') {
    return videoRestarting.value ? t('console.videoRestarting') : t('console.connecting')
  }

  switch (webrtc.connectStage.value) {
    case 'fetching_ice_servers':
      return t('console.webrtcPhaseIceServers')
    case 'creating_peer_connection':
      return t('console.webrtcPhaseCreatePeer')
    case 'creating_data_channel':
      return t('console.webrtcPhaseCreateChannel')
    case 'creating_offer':
      return t('console.webrtcPhaseCreateOffer')
    case 'waiting_server_answer':
      return t('console.webrtcPhaseWaitAnswer')
    case 'setting_remote_description':
      return t('console.webrtcPhaseSetRemote')
    case 'applying_ice_candidates':
      return t('console.webrtcPhaseApplyIce')
    case 'waiting_connection':
      return t('console.webrtcPhaseNegotiating')
    case 'connected':
      return t('console.webrtcConnected')
    case 'failed':
      return t('console.webrtcFailed')
    default:
      return videoRestarting.value ? t('console.videoRestarting') : t('console.connecting')
  }
})

const showMsdStatusCard = computed(() => {
  return !!(systemStore.msd?.available && systemStore.hid?.backend !== 'ch9329')
})

const hidHoverAlign = computed<'start' | 'end'>(() => {
  return showMsdStatusCard.value ? 'start' : 'end'
})

let retryTimeoutId: number | null = null
let retryCount = 0
let gracePeriodTimeoutId: number | null = null
let consecutiveErrors = 0
const BASE_RETRY_DELAY = 2000
const GRACE_PERIOD = 2000
const MAX_CONSECUTIVE_ERRORS = 2
let pendingWebRTCReadyGate = false
let webrtcConnectTask: Promise<boolean> | null = null

let webrtcRecoveryTimerId: number | null = null
let webrtcRecoveryAttempts = 0
const MAX_WEBRTC_RECOVERY_ATTEMPTS = 8
const WEBRTC_RECOVERY_BASE_DELAY = 2000

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

    frameOverlayUrl.value = canvas.toDataURL('image/jpeg', 0.7)
  } catch {
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

/** For WebRTC watch: skip auto-reconnect when these hold. */
function shouldSuppressAutoReconnect(): boolean {
  return videoMode.value === 'mjpeg'
    || !isConsoleActive.value
    || videoSession.localSwitching.value
    || videoSession.backendSwitching.value
    || videoRestarting.value
}

function markWebRTCFailure(reason: string, description?: string) {
  pendingWebRTCReadyGate = false
  videoError.value = true
  videoErrorMessage.value = reason
  videoLoading.value = false
  systemStore.setStreamOnline(false)

  toast.error(reason, {
    description: description ?? '',
    duration: 5000,
  })
}

async function waitForWebRTCReadyGate(reason: string, timeoutMs = 3000): Promise<void> {
  if (!pendingWebRTCReadyGate) return
  const ready = await videoSession.waitForWebRTCReadyAny(timeoutMs)
  if (!ready) {
    console.warn(`[WebRTC] Ready gate timeout (${reason}), attempting connection anyway`)
  }
  pendingWebRTCReadyGate = false
}

async function connectWebRTCSerial(reason: string): Promise<boolean> {
  if (webrtcConnectTask) {
    return webrtcConnectTask
  }

  webrtcConnectTask = (async () => {
    await waitForWebRTCReadyGate(reason)
    return webrtc.connect()
  })()

  try {
    return await webrtcConnectTask
  } finally {
    webrtcConnectTask = null
  }
}

function handleVideoLoad() {
  if (videoMode.value === 'mjpeg') {
    mjpegFrameReceived.value = true
    systemStore.setStreamOnline(true)
    const img = videoRef.value
    if (img && img.naturalWidth && img.naturalHeight) {
      videoAspectRatio.value = `${img.naturalWidth}/${img.naturalHeight}`
    }
  }

  if (!videoLoading.value) {
    return
  }

  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }

  videoLoading.value = false
  videoError.value = false
  videoErrorMessage.value = ''
  videoRestarting.value = false
  retryCount = 0
  consecutiveErrors = 0
  clearFrameOverlay()

  const container = videoContainerRef.value
  if (container && typeof container.focus === 'function') {
    container.focus()
  }
}

function handleVideoError() {
  if (videoMode.value !== 'mjpeg') {
    return
  }

  if (isModeSwitching.value) {
    return
  }

  if (isRefreshingVideo) {
    return
  }

  if (streamSignalState.value !== 'ok') {
    if (retryTimeoutId !== null) {
      clearTimeout(retryTimeoutId)
      retryTimeoutId = null
    }
    videoLoading.value = false
    mjpegFrameReceived.value = false
    return
  }

  consecutiveErrors++

  if (consecutiveErrors > MAX_CONSECUTIVE_ERRORS && gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
    videoRestarting.value = false
  }

  if (videoRestarting.value || gracePeriodTimeoutId !== null) {
    return
  }

  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }

  videoLoading.value = true
  mjpegFrameReceived.value = false

  retryCount++
  const delay = BASE_RETRY_DELAY * Math.pow(1.5, Math.min(retryCount - 1, 5))

  retryTimeoutId = window.setTimeout(() => {
    retryTimeoutId = null
    refreshVideo()
  }, delay)
}

function handleStreamDeviceLost(data: { device: string; reason: string }) {
  videoError.value = true
  videoErrorMessage.value = t('console.deviceLostDesc', { device: data.device, reason: data.reason })

  if (videoMode.value !== 'mjpeg') {
    scheduleWebRTCRecovery()
  }
}

function scheduleWebRTCRecovery() {
  if (webrtcRecoveryTimerId !== null) {
    clearTimeout(webrtcRecoveryTimerId)
    webrtcRecoveryTimerId = null
  }

  if (webrtcRecoveryAttempts >= MAX_WEBRTC_RECOVERY_ATTEMPTS) {
    console.warn('[Recovery] Max WebRTC recovery attempts reached, giving up')
    webrtcRecoveryAttempts = 0
    return
  }

  const delay = Math.min(
    WEBRTC_RECOVERY_BASE_DELAY * Math.pow(2, webrtcRecoveryAttempts),
    30000,
  )

  console.log(
    `[Recovery] Scheduling WebRTC reconnect attempt ${webrtcRecoveryAttempts + 1}/${MAX_WEBRTC_RECOVERY_ATTEMPTS} in ${delay}ms`,
  )

  webrtcRecoveryTimerId = window.setTimeout(async () => {
    webrtcRecoveryTimerId = null
    webrtcRecoveryAttempts++

    if (videoMode.value === 'mjpeg' || !videoError.value) {
      webrtcRecoveryAttempts = 0
      return
    }

    console.log(`[Recovery] Attempting WebRTC reconnect (attempt ${webrtcRecoveryAttempts})`)
    try {
      await webrtc.disconnect()
      const ok = await connectWebRTCSerial('device-recovery')
      if (ok) {
        console.log('[Recovery] WebRTC reconnected successfully')
        videoError.value = false
        videoErrorMessage.value = ''
        webrtcRecoveryAttempts = 0
      } else {
        scheduleWebRTCRecovery()
      }
    } catch {
      scheduleWebRTCRecovery()
    }
  }, delay)
}

function cancelWebRTCRecovery() {
  if (webrtcRecoveryTimerId !== null) {
    clearTimeout(webrtcRecoveryTimerId)
    webrtcRecoveryTimerId = null
  }
  webrtcRecoveryAttempts = 0
}

function handleStreamRecovered(_data: { device: string }) {
  cancelWebRTCRecovery()

  videoError.value = false
  videoErrorMessage.value = ''
  refreshVideo()
}

async function handleAudioStateChanged(data: { streaming: boolean; device: string | null }) {
  if (!data.streaming) {
    unifiedAudio.disconnect()
    return
  }

  if (videoMode.value !== 'mjpeg' && webrtc.isConnected.value) {
    if (!webrtc.audioTrack.value) {
      await webrtc.disconnect()
      await new Promise(resolve => setTimeout(resolve, 300))
      await connectWebRTCSerial('audio track refresh')
    } else {
      const currentStream = webrtcVideoRef.value?.srcObject as MediaStream | null
      if (currentStream && currentStream.getAudioTracks().length === 0) {
        currentStream.addTrack(webrtc.audioTrack.value)
      }
    }
  }

  await unifiedAudio.connect()
}

function handleStreamConfigChanging(data: any) {
  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }

  videoRestarting.value = true
  pendingWebRTCReadyGate = true
  videoLoading.value = true
  videoError.value = false
  retryCount = 0
  consecutiveErrors = 0

  backendFps.value = 0

  toast.info(t('console.videoRestarting'), {
    description: data.reason === 'device_switch' ? t('console.deviceSwitching') : t('console.configChanging'),
    duration: 5000,
  })
}

async function handleStreamConfigApplied(data: any) {
  consecutiveErrors = 0

  gracePeriodTimeoutId = window.setTimeout(() => {
    gracePeriodTimeoutId = null
    consecutiveErrors = 0
  }, GRACE_PERIOD)

  videoRestarting.value = true

  if (isModeSwitching.value) {
    console.log('[StreamConfigApplied] Mode switch in progress, waiting for WebRTCReady')
    return
  }

  if (videoMode.value !== 'mjpeg') {
    await switchToWebRTC(videoMode.value)
  } else {
    refreshVideo()
  }

  videoRestarting.value = false

  toast.success(t('console.videoRestarted'), {
    description: `${data.device} - ${data.resolution[0]}x${data.resolution[1]} @ ${formatFpsValue(data.fps)}fps`,
    duration: 3000,
  })
}

function handleWebRTCReady(data: { codec: string; hardware: boolean; transition_id?: string }) {
  console.log(`[WebRTCReady] Backend ready: codec=${data.codec}, hardware=${data.hardware}, transition_id=${data.transition_id || '-'}`)
  pendingWebRTCReadyGate = false
  videoSession.onWebRTCReady(data)
}

function handleStreamModeReady(data: { transition_id: string; mode: string }) {
  videoSession.onModeReady(data)
  if (data.mode === 'mjpeg') {
    pendingWebRTCReadyGate = false
  }
  videoRestarting.value = false
}

function handleStreamModeSwitching(data: { transition_id: string; to_mode: string; from_mode: string }) {
  if (!isModeSwitching.value) {
    videoRestarting.value = true
    videoLoading.value = true
    captureFrameOverlay().catch(() => {})
  }
  pendingWebRTCReadyGate = true
  videoSession.onModeSwitching(data)
}

function handleStreamStateChanged(data: any) {
  const state = typeof data?.state === 'string' ? data.state : ''
  const reason = typeof data?.reason === 'string' && data.reason.length > 0 ? data.reason : null
  const nextRetry = typeof data?.next_retry_ms === 'number' && data.next_retry_ms > 0
    ? data.next_retry_ms
    : null

  streamSignalReason.value = reason
  streamNextRetryMs.value = nextRetry

  const previous = streamSignalState.value

  switch (state) {
    case 'streaming':
    case 'ready':
    case 'uninitialized':
      streamSignalState.value = 'ok'
      break
    case 'no_signal':
      streamSignalState.value = 'no_signal'
      break
    case 'device_lost':
      streamSignalState.value = 'device_lost'
      break
    case 'device_busy':
      streamSignalState.value = 'device_busy'
      break
  }

  if (state === 'error') {
    videoError.value = true
    videoErrorMessage.value = t('console.streamError')
  } else if (state === 'no_signal' && videoMode.value !== 'mjpeg') {
    cancelWebRTCRecovery()
    videoRestarting.value = false
    videoError.value = false
    videoErrorMessage.value = ''
  } else if (state === 'device_busy' && videoMode.value !== 'mjpeg') {
    cancelWebRTCRecovery()
    videoRestarting.value = true
    videoLoading.value = true
    videoError.value = false
    videoErrorMessage.value = ''
    if (previous !== 'device_busy') {
      captureFrameOverlay().catch(() => {})
    }
  } else if (state === 'device_lost' && videoMode.value !== 'mjpeg') {
    if (webrtcRecoveryTimerId === null && webrtcRecoveryAttempts === 0) {
      scheduleWebRTCRecovery()
    }
  } else if (state === 'streaming') {
    cancelWebRTCRecovery()
    videoError.value = false
    videoErrorMessage.value = ''
    videoRestarting.value = false
    if (
      videoMode.value === 'mjpeg'
      && (previous === 'no_signal' || previous === 'device_lost' || previous === 'device_busy')
    ) {
      refreshVideo()
    } else if (
      videoMode.value !== 'mjpeg'
      && (previous === 'no_signal' || previous === 'device_busy' || previous === 'device_lost')
    ) {
      if (webrtc.isConnected.value && !webrtc.isConnecting.value) {
        void rebindWebRTCVideo().then(() => {
          videoLoading.value = false
        })
      } else if (!webrtc.isConnected.value && !webrtc.isConnecting.value) {
        void connectWebRTCSerial('stream recovered').then(async (ok) => {
          if (ok) {
            await rebindWebRTCVideo()
            videoLoading.value = false
          } else if (webrtcRecoveryTimerId === null && webrtcRecoveryAttempts === 0) {
            scheduleWebRTCRecovery()
          }
        })
      }
    }
  }
}

const showSignalOverlay = computed(() => streamSignalState.value !== 'ok')

const signalOverlayInfo = computed(() => {
  const reason = streamSignalReason.value
  const reasonHintKey = reason ? `console.signal.reason.${reason}` : ''
  const hint = reasonHintKey && te(reasonHintKey) ? t(reasonHintKey) : ''

  if (streamSignalState.value === 'no_signal' && reason) {
    const titleKey = `console.signal.${reason}.title`
    const detailKey = `console.signal.${reason}.detail`
    if (te(titleKey) && te(detailKey)) {
      return {
        title: t(titleKey),
        detail: t(detailKey),
        hint,
        tone: 'info' as const,
      }
    }
  }

  switch (streamSignalState.value) {
    case 'no_signal':
      return {
        title: t('console.signal.noSignal.title'),
        detail: t('console.signal.noSignal.detail'),
        hint,
        tone: 'info' as const,
      }
    case 'device_lost':
      return {
        title: t('console.signal.deviceLost.title'),
        detail: t('console.signal.deviceLost.detail'),
        hint,
        tone: 'error' as const,
      }
    case 'device_busy':
      return {
        title: t('console.signal.deviceBusy.title'),
        detail: t('console.signal.deviceBusy.detail'),
        hint,
        tone: 'info' as const,
      }
    default:
      return { title: '', detail: '', hint: '', tone: 'info' as const }
  }
})

function handleStreamStatsUpdate(data: any) {
  if (typeof data.clients === 'number') {
    systemStore.updateStreamClients(data.clients)
  }

  if (videoMode.value !== 'mjpeg') {
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

let initialDeviceInfoReceived = false
let initialModeRestoreDone = false
let initialModeRestoreInProgress = false

function normalizeServerMode(mode: string | undefined): VideoMode | null {
  if (!mode) return null
  if (mode === 'webrtc') return 'h264'
  if (mode === 'mjpeg' || mode === 'h264' || mode === 'h265' || mode === 'vp8' || mode === 'vp9') {
    return mode
  }
  return null
}

async function restoreInitialMode(serverMode: VideoMode) {
  if (initialModeRestoreDone || initialModeRestoreInProgress) return
  initialModeRestoreInProgress = true

  try {
    initialDeviceInfoReceived = true
    if (serverMode !== videoMode.value) {
      videoMode.value = serverMode
      localStorage.setItem('videoMode', serverMode)
    }

    if (serverMode !== 'mjpeg') {
      await connectWebRTCOnly(serverMode)
    } else if (mjpegTimestamp.value === 0) {
      refreshVideo()
    }

    initialModeRestoreDone = true
  } finally {
    initialModeRestoreInProgress = false
  }
}

function handleDeviceInfo(data: any) {
  const prevAudioStreaming = systemStore.audio?.streaming ?? false
  const prevAudioDevice = systemStore.audio?.device ?? null
  systemStore.updateFromDeviceInfo(data)
  ttydStatus.value = data.ttyd ?? null

  const nextAudioStreaming = systemStore.audio?.streaming ?? false
  const nextAudioDevice = systemStore.audio?.device ?? null
  if (
    prevAudioStreaming !== nextAudioStreaming ||
    prevAudioDevice !== nextAudioDevice
  ) {
    void handleAudioStateChanged({
      streaming: nextAudioStreaming,
      device: nextAudioDevice,
    })
  }

  // This prevents false-positive mode changes during config switching
  if (data.video?.config_changing) {
    return
  }

  if (data.video?.stream_mode) {
    const serverMode = normalizeServerMode(data.video.stream_mode)
    if (!serverMode) return

    if (!initialDeviceInfoReceived) {
      initialDeviceInfoReceived = true
      if (!initialModeRestoreDone && !initialModeRestoreInProgress) {
        void restoreInitialMode(serverMode)
        return
      }
    }

    if (initialModeRestoreInProgress) return
    if (serverMode !== videoMode.value) {
      syncToServerMode(serverMode)
    }
  }
}

function handleStreamModeChanged(data: { mode: string; previous_mode: string }) {
  const newMode = normalizeServerMode(data.mode)
  if (!newMode) return

  // Ignore this during a local mode switch because it was triggered by our own request
  if (isModeSwitching.value) {
    console.log('[StreamModeChanged] Mode switch in progress, ignoring event')
    return
  }

  toast.info(t('console.streamModeChanged'), {
    description: t('console.streamModeChangedDesc', { mode: data.mode.toUpperCase() }),
    duration: 5000,
  })

  if (newMode !== videoMode.value) {
    syncToServerMode(newMode)
  }
}

let isRefreshingVideo = false
const isModeSwitching = videoSession.localSwitching

function reloadPage() {
  window.location.reload()
}

function refreshVideo() {
  backendFps.value = 0
  videoError.value = false
  videoErrorMessage.value = ''
  mjpegFrameReceived.value = false

  isRefreshingVideo = true
  videoLoading.value = true
  mjpegTimestamp.value = Date.now()

  setTimeout(() => {
    isRefreshingVideo = false
    if (videoLoading.value) {
      videoLoading.value = false
    }
  }, 1500)
}

// MJPEG URL with cache-busting timestamp (reactive)
// Only return valid URL when in MJPEG mode and the backend reports a
// healthy stream.  When the backend goes offline (no_signal / device_lost
// / device_busy) we deliberately return an empty string so the `<img>`
// tag has no `src` and the 4-state overlay fully owns the video area —
// no more fake placeholder JPEG peeking through.
const mjpegTimestamp = ref(0)
const mjpegUrl = computed(() => {
  if (videoMode.value !== 'mjpeg') {
    return ''
  }
  if (mjpegTimestamp.value === 0) {
    return ''
  }
  if (streamSignalState.value !== 'ok') {
    return ''
  }
  return `${streamApi.getMjpegUrl(myClientId)}&t=${mjpegTimestamp.value}`
})

async function connectWebRTCOnly(codec: VideoMode = 'h264') {
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

  mjpegTimestamp.value = 0
  if (videoRef.value) {
    videoRef.value.src = ''
    videoRef.value.removeAttribute('src')
  }

  videoLoading.value = true
  videoError.value = false
  videoErrorMessage.value = ''

  try {
    const success = await connectWebRTCSerial('connectWebRTCOnly')
    if (success) {
      toast.success(t('console.webrtcConnected'), {
        description: t('console.webrtcConnectedDesc'),
        duration: 3000,
      })

      // Force video rebind even when the track already exists
      // This fixes missing video after returning to the page
      await rebindWebRTCVideo()

      videoLoading.value = false
      videoMode.value = codec
      unifiedAudio.switchMode('webrtc')
    } else {
      throw new Error('WebRTC connection failed')
    }
  } catch {
    markWebRTCFailure(t('console.webrtcFailed'))
  }
}

async function rebindWebRTCVideo() {
  if (!webrtcVideoRef.value) return

  webrtcVideoRef.value.srcObject = null
  await nextTick()

  if (webrtc.videoTrack.value) {
    const stream = webrtc.getMediaStream()
    if (stream) {
      webrtcVideoRef.value.srcObject = stream
      try {
        await webrtcVideoRef.value.play()
      } catch {
      }
      await waitForVideoFirstFrame(webrtcVideoRef.value, 2000)
      clearFrameOverlay()
    }
  }
}

async function switchToWebRTC(codec: VideoMode = 'h264') {
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

  mjpegTimestamp.value = 0
  if (videoRef.value) {
    videoRef.value.src = ''
  }

  videoLoading.value = true
  videoError.value = false
  videoErrorMessage.value = ''
  pendingWebRTCReadyGate = true

  try {
    // Disconnect first so ICE candidates are not sent to stale sessions during backend codec switch.
    if (webrtc.isConnected.value || webrtc.sessionId.value) {
      await webrtc.disconnect()
    }

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

    const MAX_ATTEMPTS = 3
    const RETRY_DELAYS = [200, 800]
    let success = false
    for (let attempt = 0; attempt < MAX_ATTEMPTS && !success; attempt++) {
      if (attempt > 0) {
        const delay = RETRY_DELAYS[attempt - 1] ?? RETRY_DELAYS[RETRY_DELAYS.length - 1]
        console.log(`[WebRTC] Connection failed, retrying in ${delay}ms (${MAX_ATTEMPTS - attempt} attempts left)`)
        await new Promise(resolve => setTimeout(resolve, delay))
      }
      success = await connectWebRTCSerial('switchToWebRTC')
    }
    if (success) {
      toast.success(t('console.webrtcConnected'), {
        description: t('console.webrtcConnectedDesc'),
        duration: 3000,
      })

      await rebindWebRTCVideo()

      videoLoading.value = false

      unifiedAudio.switchMode('webrtc')
    } else {
      throw new Error('WebRTC connection failed')
    }
  } catch {
    markWebRTCFailure(t('console.webrtcFailed'))
  }
}

async function switchToMJPEG() {
  videoLoading.value = true
  videoError.value = false
  videoErrorMessage.value = ''
  pendingWebRTCReadyGate = false

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
  }

  if (webrtc.isConnected.value || webrtc.sessionId.value) {
    await webrtc.disconnect()
  }

  if (webrtcVideoRef.value) {
    webrtcVideoRef.value.srcObject = null
  }

  unifiedAudio.switchMode('ws')

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

async function handleVideoModeChange(mode: VideoMode) {
  if (mode === videoMode.value) return
  if (!videoSession.tryStartLocalSwitch()) {
    console.log('[VideoMode] Switch throttled or in progress, ignoring')
    return
  }

  try {
    await captureFrameOverlay()

    if (mode !== 'mjpeg') {
      mjpegTimestamp.value = 0
      if (videoRef.value) {
        videoRef.value.src = ''
        videoRef.value.removeAttribute('src')
      }
      await new Promise(resolve => setTimeout(resolve, 50))
    }

    videoMode.value = mode
    localStorage.setItem('videoMode', mode)

    if (mode !== 'mjpeg') {
      await switchToWebRTC(mode)
    } else {
      await switchToMJPEG()
    }
  } finally {
    videoSession.endLocalSwitch()
  }
}

watch(() => webrtc.videoTrack.value, async (track) => {
  if (track && webrtcVideoRef.value && videoMode.value !== 'mjpeg') {
    await rebindWebRTCVideo()
  }
})

watch(() => webrtc.audioTrack.value, async (track) => {
  if (track && webrtcVideoRef.value && videoMode.value !== 'mjpeg') {
    const currentStream = webrtcVideoRef.value.srcObject as MediaStream | null
    if (currentStream && currentStream.getAudioTracks().length === 0) {
      currentStream.addTrack(track)
    }
  }
})

watch(webrtcVideoRef, (el) => {
  unifiedAudio.setWebRTCElement(el)
}, { immediate: true })

watch(webrtc.stats, (stats) => {
  if (videoMode.value !== 'mjpeg' && stats.framesPerSecond > 0) {
    backendFps.value = Math.round(stats.framesPerSecond)
    systemStore.setStreamOnline(true)
    if (stats.frameWidth && stats.frameHeight) {
      videoAspectRatio.value = `${stats.frameWidth}/${stats.frameHeight}`
    }
  }
}, { deep: true })

let webrtcReconnectTimeout: ReturnType<typeof setTimeout> | null = null
let webrtcReconnectFailures = 0
watch(() => webrtc.state.value, (newState, oldState) => {
  console.log('[WebRTC] State changed:', oldState, '->', newState)

  if (webrtcReconnectTimeout) {
    clearTimeout(webrtcReconnectTimeout)
    webrtcReconnectTimeout = null
  }

  // Run before `shouldSuppressAutoReconnect()` so `device_busy` / `videoRestarting`
  // never blocks clearing the loading overlay when ICE becomes connected.
  if (videoMode.value !== 'mjpeg') {
    if (newState === 'connected') {
      systemStore.setStreamOnline(true)
      webrtcReconnectFailures = 0
      if (videoLoading.value) {
        void rebindWebRTCVideo().then(() => {
          videoLoading.value = false
        })
      }
    }
  }

  if (shouldSuppressAutoReconnect()) {
    return
  }

  if (newState === 'disconnected' && oldState === 'connected' && videoMode.value !== 'mjpeg') {
    webrtcReconnectTimeout = setTimeout(async () => {
      if (videoMode.value !== 'mjpeg' && webrtc.state.value === 'disconnected') {
        try {
          const success = await connectWebRTCSerial('auto reconnect')
          if (!success) {
            webrtcReconnectFailures += 1
            if (webrtcReconnectFailures >= 2) {
              markWebRTCFailure(t('console.webrtcFailed'))
            }
          }
        } catch {
          webrtcReconnectFailures += 1
          if (webrtcReconnectFailures >= 2) {
            markWebRTCFailure(t('console.webrtcFailed'))
          }
        }
      }
    }, 1000)
  }

  if (newState === 'failed' && videoMode.value !== 'mjpeg') {
    webrtcReconnectFailures += 1
    if (webrtcReconnectFailures >= 2) {
      markWebRTCFailure(t('console.webrtcFailed'))
    } else {
      webrtcReconnectTimeout = setTimeout(async () => {
        if (videoMode.value !== 'mjpeg' && webrtc.state.value !== 'connected') {
          const success = await connectWebRTCSerial('auto reconnect after failed')
          if (!success) {
            markWebRTCFailure(t('console.webrtcFailed'))
          }
        }
      }, 1000)
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

function toggleTheme() {
  isDark.value = !isDark.value
  document.documentElement.classList.toggle('dark', isDark.value)
  localStorage.setItem('theme', isDark.value ? 'dark' : 'light')
}

async function logout() {
  await authStore.logout()
  router.push('/login')
}

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

    currentPassword.value = ''
    newPassword.value = ''
    confirmPassword.value = ''
    changePasswordDialogOpen.value = false
  } catch (e) {
    console.info('[ChangePassword] Failed:', e)
  } finally {
    changingPassword.value = false
  }
}

function openTerminal() {
  if (!ttydStatus.value?.running) return
  showTerminalDialog.value = true
}

function openTerminalInNewTab() {
  window.open('/api/terminal/', '_blank')
}

async function handlePowerShort() {
  try {
    await atxApi.power('short')
    await systemStore.fetchAtxState()
  } catch {
  }
}

async function handlePowerLong() {
  try {
    await atxApi.power('long')
    await systemStore.fetchAtxState()
  } catch {
  }
}

async function handleReset() {
  try {
    await atxApi.power('reset')
    await systemStore.fetchAtxState()
  } catch {
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

function handleHidError(_error: any, _operation: string) {
}

function sendKeyboardEvent(type: 'down' | 'up', key: CanonicalKey, modifier?: number) {
  if (videoMode.value !== 'mjpeg' && webrtc.dataChannelReady.value) {
    const event: HidKeyboardEvent = {
      type: type === 'down' ? 'keydown' : 'keyup',
      key,
      modifier,
    }
    const sent = webrtc.sendKeyboard(event)
    if (sent) return
  }
  hidApi.keyboard(type, key, modifier).catch(err => handleHidError(err, `keyboard ${type}`))
}

function sendMouseEvent(data: { type: 'move' | 'move_abs' | 'down' | 'up' | 'scroll'; x?: number; y?: number; button?: 'left' | 'right' | 'middle'; scroll?: number }) {
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
  }
  hidApi.mouse(data).catch(err => handleHidError(err, `mouse ${data.type}`))
}

function shouldBlockKey(e: KeyboardEvent): boolean {
  if (isFullscreen.value) {
    return true
  }

  const key = e.key.toUpperCase()

  if (e.ctrlKey && ['W', 'T', 'N'].includes(key)) return false

  if (key === 'F11') return false

  if (e.altKey && key === 'TAB') return false

  return true
}

function handleKeyDown(e: KeyboardEvent) {
  const container = videoContainerRef.value
  if (!container) return

  if (!isFullscreen.value && !container.contains(document.activeElement)) return

  if (shouldBlockKey(e)) {
    e.preventDefault()
    e.stopPropagation()
  }

  if (!isFullscreen.value && (e.metaKey || e.key === 'Meta')) {
    toast.info(t('console.metaKeyHint'), {
      description: t('console.metaKeyHintDesc'),
      duration: 3000,
    })
  }

  const canonicalKey = keyboardEventToCanonicalKey(e.code, e.key)
  if (canonicalKey === undefined) {
    console.warn(`[HID] Unmapped key down: code=${e.code}, key=${e.key}`)
    return
  }

  if (!pressedKeys.value.includes(canonicalKey)) {
    pressedKeys.value = [...pressedKeys.value, canonicalKey]
  }

  const modifierMask = updateModifierMaskForKey(activeModifierMask.value, canonicalKey, true)
  activeModifierMask.value = modifierMask
  sendKeyboardEvent('down', canonicalKey, modifierMask)
}

function handleKeyUp(e: KeyboardEvent) {
  const container = videoContainerRef.value
  if (!container) return

  if (!isFullscreen.value && !container.contains(document.activeElement)) return

  if (shouldBlockKey(e)) {
    e.preventDefault()
    e.stopPropagation()
  }

  const canonicalKey = keyboardEventToCanonicalKey(e.code, e.key)
  if (canonicalKey === undefined) {
    console.warn(`[HID] Unmapped key up: code=${e.code}, key=${e.key}`)
    return
  }

  pressedKeys.value = pressedKeys.value.filter(k => k !== canonicalKey)

  const modifierMask = updateModifierMaskForKey(activeModifierMask.value, canonicalKey, false)
  activeModifierMask.value = modifierMask
  sendKeyboardEvent('up', canonicalKey, modifierMask)
}

function getActiveVideoElement(): HTMLImageElement | HTMLVideoElement | null {
  return videoMode.value !== 'mjpeg' ? webrtcVideoRef.value : videoRef.value
}

function getActiveVideoAspectRatio(): number | null {
  if (videoMode.value !== 'mjpeg') {
    const video = webrtcVideoRef.value
    if (video?.videoWidth && video.videoHeight) {
      return video.videoWidth / video.videoHeight
    }
  } else {
    const image = videoRef.value
    if (image?.naturalWidth && image.naturalHeight) {
      return image.naturalWidth / image.naturalHeight
    }
  }

  if (!videoAspectRatio.value) return null
  const [width, height] = videoAspectRatio.value.split('/').map(Number)
  if (!width || !height) return null
  return width / height
}

function getRenderedVideoRect() {
  const videoElement = getActiveVideoElement()
  if (!videoElement) return null

  const rect = videoElement.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) return null

  const contentAspectRatio = getActiveVideoAspectRatio()
  if (!contentAspectRatio) {
    return rect
  }

  const boxAspectRatio = rect.width / rect.height
  if (!Number.isFinite(boxAspectRatio) || boxAspectRatio <= 0) {
    return rect
  }

  if (boxAspectRatio > contentAspectRatio) {
    const width = rect.height * contentAspectRatio
    return {
      left: rect.left + (rect.width - width) / 2,
      top: rect.top,
      width,
      height: rect.height,
    }
  }

  const height = rect.width / contentAspectRatio
  return {
    left: rect.left,
    top: rect.top + (rect.height - height) / 2,
    width: rect.width,
    height,
  }
}

function getAbsoluteMousePosition(e: MouseEvent) {
  const rect = getRenderedVideoRect()
  if (!rect) return null

  const normalizedX = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width))
  const normalizedY = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height))

  return {
    x: Math.round(normalizedX * 32767),
    y: Math.round(normalizedY * 32767),
  }
}

function updateLocalCrosshairFromEvent(e: MouseEvent) {
  if (!cursorVisible.value) {
    localCrosshairPos.value = null
    return
  }
  const container = videoContainerRef.value
  if (!container) return

  const rect = container.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) return

  if (mouseMode.value === 'relative' && isPointerLocked.value) {
    const cur = localCrosshairPos.value
    const nx = cur ? cur.x + e.movementX : rect.width / 2
    const ny = cur ? cur.y + e.movementY : rect.height / 2
    localCrosshairPos.value = {
      x: Math.max(0, Math.min(rect.width, nx)),
      y: Math.max(0, Math.min(rect.height, ny)),
    }
    return
  }

  localCrosshairPos.value = {
    x: Math.max(0, Math.min(rect.width, e.clientX - rect.left)),
    y: Math.max(0, Math.min(rect.height, e.clientY - rect.top)),
  }
}

function handleMouseLeaveVideo() {
  if (!isPointerLocked.value) {
    localCrosshairPos.value = null
  }
}

function handleMouseMove(e: MouseEvent) {
  updateLocalCrosshairFromEvent(e)

  const videoElement = getActiveVideoElement()
  if (!videoElement) return

  if (mouseMode.value === 'absolute') {
    const absolutePosition = getAbsoluteMousePosition(e)
    if (!absolutePosition) return
    const { x, y } = absolutePosition

    mousePosition.value = { x, y }
    pendingMouseMove = { type: 'move_abs', x, y }
    requestMouseMoveFlush()
  } else {
    if (isPointerLocked.value) {
      const dx = e.movementX
      const dy = e.movementY

      if (dx !== 0 || dy !== 0) {
        accumulatedDelta.x += dx
        accumulatedDelta.y += dy
        requestMouseMoveFlush()
      }

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

  const clampedDx = Math.max(-127, Math.min(127, accumulatedDelta.x))
  const clampedDy = Math.max(-127, Math.min(127, accumulatedDelta.y))

  sendMouseEvent({ type: 'move', x: clampedDx, y: clampedDy })

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

const pressedMouseButton = ref<'left' | 'right' | 'middle' | null>(null)

function handleMouseDown(e: MouseEvent) {
  e.preventDefault()

  const container = videoContainerRef.value
  if (container && document.activeElement !== container) {
    if (typeof container.focus === 'function') {
      container.focus()
    }
  }

  if (mouseMode.value === 'relative' && !isPointerLocked.value) {
    requestPointerLock()
    return
  }

  if (mouseMode.value === 'absolute') {
    const absolutePosition = getAbsoluteMousePosition(e)
    if (absolutePosition) {
      mousePosition.value = absolutePosition
      sendMouseEvent({ type: 'move_abs', ...absolutePosition })
      pendingMouseMove = null
    }
  }

  const button = e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle'
  pressedMouseButton.value = button
  sendMouseEvent({ type: 'down', button })
}

function handleMouseUp(e: MouseEvent) {
  e.preventDefault()
  handleMouseUpInternal(e.button)
}

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
    mousePosition.value = { x: 0, y: 0 }
    if (cursorVisible.value && container) {
      const r = container.getBoundingClientRect()
      if (r.width > 0 && r.height > 0) {
        localCrosshairPos.value = { x: r.width / 2, y: r.height / 2 }
      }
    }
    toast.info(t('console.pointerLocked'), {
      description: t('console.pointerLockedDesc'),
      duration: 3000,
    })
  }
}

function handlePointerLockError() {
  isPointerLocked.value = false
}

function handleFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement
}

function handleBlur() {
  pressedKeys.value = []
  activeModifierMask.value = 0
  if (pressedMouseButton.value !== null) {
    const button = pressedMouseButton.value
    pressedMouseButton.value = null
    sendMouseEvent({ type: 'up', button })
  }
}

function handleCursorVisibilityChange(e: Event) {
  const customEvent = e as CustomEvent<{ visible: boolean }>
  cursorVisible.value = customEvent.detail.visible
  if (!cursorVisible.value) {
    localCrosshairPos.value = null
  }
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

function registerInteractionListeners() {
  if (interactionListenersBound) return

  window.addEventListener('keydown', handleKeyDown)
  window.addEventListener('keyup', handleKeyUp)
  window.addEventListener('blur', handleBlur)
  window.addEventListener('mouseup', handleWindowMouseUp)
  window.addEventListener('hidCursorVisibilityChanged', handleCursorVisibilityChange as EventListener)
  window.addEventListener('hidMouseSendIntervalChanged', handleMouseSendIntervalChange as EventListener)
  window.addEventListener('storage', handleMouseSendIntervalStorage)

  document.addEventListener('pointerlockchange', handlePointerLockChange)
  document.addEventListener('pointerlockerror', handlePointerLockError)
  document.addEventListener('fullscreenchange', handleFullscreenChange)

  interactionListenersBound = true
}

function unregisterInteractionListeners() {
  if (!interactionListenersBound) return

  window.removeEventListener('keydown', handleKeyDown)
  window.removeEventListener('keyup', handleKeyUp)
  window.removeEventListener('blur', handleBlur)
  window.removeEventListener('mouseup', handleWindowMouseUp)
  window.removeEventListener('hidCursorVisibilityChanged', handleCursorVisibilityChange as EventListener)
  window.removeEventListener('hidMouseSendIntervalChanged', handleMouseSendIntervalChange as EventListener)
  window.removeEventListener('storage', handleMouseSendIntervalStorage)

  document.removeEventListener('pointerlockchange', handlePointerLockChange)
  document.removeEventListener('pointerlockerror', handlePointerLockError)
  document.removeEventListener('fullscreenchange', handleFullscreenChange)

  interactionListenersBound = false
}

async function activateConsoleView() {
  isConsoleActive.value = true
  registerInteractionListeners()

  // REST snapshot: returning from Settings (or other routes) may have missed WS device_info
  void systemStore.fetchAllStates()
  void configStore.refreshHid().then(() => syncMouseModeFromConfig()).catch(() => {})

  if (!hidWs.connected.value) {
    hidWs.connect().catch(() => {})
  }

  if (videoMode.value !== 'mjpeg' && webrtc.videoTrack.value) {
    await nextTick()
    await rebindWebRTCVideo()
    return
  }

  if (
    videoMode.value !== 'mjpeg'
    && !webrtc.isConnected.value
    && !webrtc.isConnecting.value
    && !videoSession.localSwitching.value
    && !videoSession.backendSwitching.value
    && !initialModeRestoreInProgress
  ) {
    await connectWebRTCOnly(videoMode.value)
  }
}

function deactivateConsoleView() {
  isConsoleActive.value = false
  handleBlur()
  exitPointerLock()
  unregisterInteractionListeners()
}


function handleToggleVirtualKeyboard() {
  virtualKeyboardVisible.value = !virtualKeyboardVisible.value
}

function handleVirtualKeyDown(key: CanonicalKey) {
  if (!pressedKeys.value.includes(key)) {
    pressedKeys.value = [...pressedKeys.value, key]
  }
}

function handleVirtualKeyUp(key: CanonicalKey) {
  pressedKeys.value = pressedKeys.value.filter(k => k !== key)
}

function handleToggleMouseMode() {
  if (mouseMode.value === 'relative' && isPointerLocked.value) {
    exitPointerLock()
  }

  mouseMode.value = mouseMode.value === 'absolute' ? 'relative' : 'absolute'
  pendingMouseMove = null
  accumulatedDelta = { x: 0, y: 0 }
  lastMousePosition.value = { x: 0, y: 0 }
  mousePosition.value = { x: 0, y: 0 }

  if (mouseMode.value === 'relative') {
    toast.info(t('console.relativeModeHint'), {
      description: t('console.relativeModeHintDesc'),
      duration: 5000,
    })
  }
}

onMounted(async () => {
  consoleEvents.subscribe()

  watch([wsConnected, wsNetworkError], ([connected, netError], [_prevConnected, prevNetError]) => {
    systemStore.updateWsConnection(connected, netError)

    if (prevNetError === true && netError === false && connected === true) {
      refreshVideo()
    }
  }, { immediate: true })

  watch([() => hidWs.connected.value, () => hidWs.networkError.value], ([connected, netError]) => {
    systemStore.updateHidWsConnection(connected, netError)
  }, { immediate: true })

  await systemStore.startStream().catch(() => {})
  await systemStore.fetchAllStates()
  await configStore.refreshHid().then(() => {
    syncMouseModeFromConfig()
  }).catch(() => {})

  setMouseMoveSendInterval(loadMouseMoveSendIntervalFromStorage())

  watch(() => configStore.hid?.mouse_absolute, () => {
    syncMouseModeFromConfig()
  })

  const storedTheme = localStorage.getItem('theme')
  if (storedTheme === 'dark' || (!storedTheme && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
    isDark.value = true
    document.documentElement.classList.add('dark')
  }

  try {
    const modeResp = await streamApi.getMode()
    const serverMode = normalizeServerMode(modeResp?.mode)
    if (serverMode && !initialModeRestoreDone && !initialModeRestoreInProgress) {
      await restoreInitialMode(serverMode)
    }
  } catch (err) {
    console.warn('[Console] Failed to fetch stream mode on enter, fallback to WS events:', err)
  }
})

onActivated(() => {
  void activateConsoleView()
})

onDeactivated(() => {
  deactivateConsoleView()
})

onUnmounted(() => {
  deactivateConsoleView()

  initialDeviceInfoReceived = false
  initialModeRestoreDone = false
  initialModeRestoreInProgress = false

  if (mouseFlushTimer !== null) {
    clearTimeout(mouseFlushTimer)
    mouseFlushTimer = null
  }

  if (retryTimeoutId !== null) {
    clearTimeout(retryTimeoutId)
    retryTimeoutId = null
  }
  if (gracePeriodTimeoutId !== null) {
    clearTimeout(gracePeriodTimeoutId)
    gracePeriodTimeoutId = null
  }
  cancelWebRTCRecovery()
  videoSession.clearWaiters()

  retryCount = 0

  consoleEvents.unsubscribe()
  consecutiveErrors = 0

  if (webrtc.isConnected.value || webrtc.sessionId.value) {
    void webrtc.disconnect()
  }

  exitPointerLock()
})
</script>

<template>
  <div class="h-screen h-dvh flex flex-col bg-background">
    <header class="shrink-0 border-b border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div class="px-2 sm:px-4">
        <div class="h-10 sm:h-14 flex items-center justify-between">
          <div class="flex items-center gap-2 sm:gap-6">
            <div class="flex items-center gap-1.5 sm:gap-2">
              <BrandMark size="md" class="hidden sm:block" />
              <BrandMark size="sm" class="sm:hidden" />
              <span class="font-bold text-sm sm:text-lg">One-KVM</span>
            </div>
            <div class="flex md:hidden items-center gap-1">
              <StatusCard
                :title="t('statusCard.video')"
                type="video"
                :status="videoStatus"
                :quick-info="videoQuickInfo"
                :error-message="videoErrorMessage"
                :details="videoDetails"
                compact
              />

              <StatusCard
                :title="t('statusCard.hid')"
                type="hid"
                :status="hidStatus"
                :quick-info="hidQuickInfo"
                :details="hidDetails"
                :hover-align="hidHoverAlign"
                compact
              />
            </div>
          </div>
          <div class="flex items-center gap-1 sm:gap-2">
            <div class="hidden md:flex items-center gap-2">
              <StatusCard
                :title="t('statusCard.video')"
                type="video"
                :status="videoStatus"
                :quick-info="videoQuickInfo"
                :error-message="videoErrorMessage"
                :details="videoDetails"
              />
              <StatusCard
                v-if="systemStore.audio?.available"
                :title="t('statusCard.audio')"
                type="audio"
                :status="audioStatus"
                :quick-info="audioQuickInfo"
                :error-message="audioErrorMessage"
                :details="audioDetails"
              />
              <StatusCard
                :title="t('statusCard.hid')"
                type="hid"
                :status="hidStatus"
                :quick-info="hidQuickInfo"
                :error-message="hidErrorMessage"
                :details="hidDetails"
                :hover-align="hidHoverAlign"
              />
              <StatusCard
                v-if="showMsdStatusCard"
                :title="t('statusCard.msd')"
                type="msd"
                :status="msdStatus"
                :quick-info="msdQuickInfo"
                :error-message="msdErrorMessage"
                :details="msdDetails"
                hover-align="end"
              />
            </div>
            <div class="h-6 w-px bg-slate-200 dark:bg-slate-700 hidden md:block mx-1" />
            <Button variant="ghost" size="icon" class="h-8 w-8 hidden md:flex" :aria-label="t('common.toggleTheme')" @click="toggleTheme">
              <Sun v-if="isDark" class="h-4 w-4" />
              <Moon v-else class="h-4 w-4" />
            </Button>
            <LanguageToggleButton class="h-8 w-8 hidden md:flex" />
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button variant="outline" size="sm" class="gap-1 sm:gap-1.5 h-7 sm:h-9 px-2 sm:px-3">
                  <span class="text-xs max-w-[60px] sm:max-w-[100px] truncate">{{ authStore.user || 'admin' }}</span>
                  <ChevronDown class="h-3 w-3 sm:h-3.5 sm:w-3.5" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem class="md:hidden" @click="toggleTheme">
                  <Sun v-if="isDark" class="h-4 w-4 mr-2" />
                  <Moon v-else class="h-4 w-4 mr-2" />
                  {{ isDark ? t('settings.lightMode') : t('settings.darkMode') }}
                </DropdownMenuItem>
                <DropdownMenuItem as-child class="md:hidden p-0">
                  <LanguageToggleButton
                    label-mode="next"
                    size="sm"
                    variant="ghost"
                    class="w-full justify-start rounded-sm px-2 py-1.5 font-normal shadow-none"
                  />
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
      </div>
    </header>
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
    <div class="flex-1 overflow-hidden relative">
      <div
        class="absolute inset-0 bg-slate-100/80 dark:bg-slate-800/40 opacity-80"
        style="
          background-image: radial-gradient(circle, rgb(148 163 184 / 0.4) 1px, transparent 1px);
          background-size: 20px 20px;
        "
      />
      <div class="relative h-full w-full flex items-center justify-center p-1 sm:p-4">
        <div
          ref="videoContainerRef"
          class="relative bg-black overflow-hidden flex items-center justify-center"
          :style="{
            aspectRatio: videoAspectRatio ?? '16/9',
            maxWidth: '100%',
            maxHeight: '100%',
            minHeight: '120px',
          }"
          :class="{
            'opacity-60': videoLoading || videoError,
            'cursor-none': true,
          }"
          tabindex="0"
          @mouseleave="handleMouseLeaveVideo"
          @mousemove="handleMouseMove"
          @mousedown="handleMouseDown"
          @mouseup="handleMouseUp"
          @wheel.prevent="handleWheel"
          @contextmenu="handleContextMenu"
        >
          <img
            v-show="videoMode === 'mjpeg'"
            ref="videoRef"
            :src="mjpegUrl"
            class="w-full h-full object-contain"
            :alt="t('console.videoAlt')"
            @load="handleVideoLoad"
            @error="handleVideoError"
          />
          <video
            v-show="videoMode !== 'mjpeg'"
            ref="webrtcVideoRef"
            class="w-full h-full object-contain"
            autoplay
            playsinline
          />
          <img
            v-if="frameOverlayUrl"
            :src="frameOverlayUrl"
            class="absolute inset-0 w-full h-full object-contain pointer-events-none"
            alt=""
          />
          <div
            v-if="cursorVisible && localCrosshairPos"
            class="pointer-events-none absolute z-[15] -translate-x-1/2 -translate-y-1/2"
            :style="{
              left: `${localCrosshairPos.x}px`,
              top: `${localCrosshairPos.y}px`,
            }"
            aria-hidden="true"
          >
            <svg
              width="23"
              height="23"
              viewBox="-11.5 -11.5 23 23"
              class="overflow-visible"
            >
              <g stroke-linecap="square">
                <line
                  x1="0"
                  y1="-10"
                  x2="0"
                  y2="10"
                  stroke="rgba(0,0,0,0.88)"
                  stroke-width="3"
                />
                <line
                  x1="-10"
                  y1="0"
                  x2="10"
                  y2="0"
                  stroke="rgba(0,0,0,0.88)"
                  stroke-width="3"
                />
                <line
                  x1="0"
                  y1="-10"
                  x2="0"
                  y2="10"
                  stroke="rgba(255,255,255,0.95)"
                  stroke-width="1"
                />
                <line
                  x1="-10"
                  y1="0"
                  x2="10"
                  y2="0"
                  stroke="rgba(255,255,255,0.95)"
                  stroke-width="1"
                />
              </g>
            </svg>
          </div>
          <Transition name="fade">
            <div
              v-if="videoLoading"
              class="absolute inset-0 flex flex-col items-center justify-center bg-black/70 backdrop-blur-sm transition-opacity duration-300"
            >
              <div class="absolute inset-0 overflow-hidden pointer-events-none">
                <div class="absolute w-full h-0.5 bg-gradient-to-r from-transparent via-primary/40 to-transparent animate-pulse" style="top: 50%; animation-duration: 1.5s;" />
              </div>

              <Spinner class="h-10 w-10 sm:h-16 sm:w-16 text-white mb-2 sm:mb-4" />
              <p class="text-white/90 text-sm sm:text-lg font-medium text-center px-4">
                {{ webrtcLoadingMessage }}
              </p>
              <p class="text-white/50 text-xs sm:text-sm mt-1 sm:mt-2">
                {{ t('console.pleaseWait') }}
              </p>
            </div>
          </Transition>
          <Transition name="fade">
            <div
              v-if="showSignalOverlay && !videoLoading && !videoError"
              class="absolute inset-0 flex flex-col items-center justify-center gap-3 p-4 transition-opacity duration-300 pointer-events-none"
              :class="{
                'bg-black/80 backdrop-blur-sm': signalOverlayInfo.tone === 'error',
                'bg-black/70 backdrop-blur-sm': signalOverlayInfo.tone !== 'error',
              }"
            >
              <MonitorOff
                class="h-10 w-10 sm:h-16 sm:w-16"
                :class="{
                  'text-slate-200': signalOverlayInfo.tone === 'info',
                  'text-red-300': signalOverlayInfo.tone === 'error',
                }"
              />
              <div class="text-center max-w-md">
                <p
                  class="font-semibold text-sm sm:text-lg text-white"
                >{{ signalOverlayInfo.title }}</p>
                <p
                  class="text-xs sm:text-sm mt-1 sm:mt-2"
                  :class="{
                    'text-slate-200/80': signalOverlayInfo.tone === 'info',
                    'text-red-100/80': signalOverlayInfo.tone === 'error',
                  }"
                >{{ signalOverlayInfo.detail }}</p>
                <p
                  v-if="signalOverlayInfo.hint"
                  class="text-[11px] sm:text-xs mt-2 text-white/50"
                >{{ signalOverlayInfo.hint }}</p>
              </div>
            </div>
          </Transition>
          <Transition name="fade">
            <div
              v-if="videoError && !videoLoading"
              class="absolute inset-0 flex flex-col items-center justify-center bg-black/85 text-white gap-4 transition-opacity duration-300 p-4"
            >
              <MonitorOff class="h-10 w-10 sm:h-16 sm:w-16 text-slate-400" />
              <div class="text-center max-w-md px-2">
                <p class="font-medium text-sm sm:text-lg mb-1 sm:mb-2">{{ t('console.connectionFailed') }}</p>
                <p class="text-xs sm:text-sm text-slate-300 mb-2 sm:mb-3">{{ t('console.connectionFailedDesc') }}</p>
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
    <Teleport :to="virtualKeyboardAttached ? '#keyboard-anchor' : 'body'" :disabled="virtualKeyboardAttached">
      <VirtualKeyboard
        v-if="virtualKeyboardVisible"
        v-model:visible="virtualKeyboardVisible"
        v-model:attached="virtualKeyboardAttached"
        :caps-lock="keyboardLed.capsLock"
        :pressed-keys="pressedKeys"
        :consumer-enabled="virtualKeyboardConsumerEnabled"
        @key-down="handleVirtualKeyDown"
        @key-up="handleVirtualKeyUp"
      />
    </Teleport>
    <div id="keyboard-anchor"></div>
    <InfoBar
      :pressed-keys="pressedKeys"
      :caps-lock="keyboardLed.capsLock"
      :num-lock="keyboardLed.numLock"
      :scroll-lock="keyboardLed.scrollLock"
      :keyboard-led-enabled="keyboardLedEnabled"
      :mouse-position="mousePosition"
      :debug-mode="false"
    />
    <StatsSheet
      v-model:open="statsSheetOpen"
      :video-mode="videoMode"
      :mjpeg-fps="backendFps"
      :ws-latency="0"
      :webrtc-stats="webrtc.stats.value"
    />
    <Dialog v-model:open="showTerminalDialog">
      <DialogContent class="w-[98vw] sm:w-[95vw] max-w-5xl h-[90dvh] sm:h-[85dvh] max-h-[720px] p-0 flex flex-col overflow-hidden">
        <DialogHeader class="px-3 sm:px-4 py-2 sm:py-3 border-b shrink-0">
          <DialogTitle class="flex items-center justify-between w-full">
            <div class="flex items-center gap-2">
              <Terminal class="h-4 w-4 sm:h-5 sm:w-5" />
              <span class="text-sm sm:text-base">{{ t('extensions.ttyd.title') }}</span>
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="h-7 w-7 sm:h-8 sm:w-8 mr-6 sm:mr-8"
              @click="openTerminalInNewTab"
              :aria-label="t('extensions.ttyd.openInNewTab')"
              :title="t('extensions.ttyd.openInNewTab')"
            >
              <ExternalLink class="h-3.5 w-3.5 sm:h-4 sm:w-4" />
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
    <Dialog v-model:open="changePasswordDialogOpen">
      <DialogContent class="w-[95vw] max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('auth.changePassword') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3 sm:space-y-4 py-2 sm:py-4">
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
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
