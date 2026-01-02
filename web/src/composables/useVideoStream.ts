// Video streaming composable - manages MJPEG/WebRTC video modes
// Extracted from ConsoleView.vue for better separation of concerns

import { ref, computed, watch, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { streamApi } from '@/api'
import { useWebRTC } from '@/composables/useWebRTC'
import { getUnifiedAudio } from '@/composables/useUnifiedAudio'
import { useSystemStore } from '@/stores/system'
import { generateUUID } from '@/lib/utils'

export type VideoMode = 'mjpeg' | 'h264' | 'h265' | 'vp8' | 'vp9'

export interface VideoStreamState {
  mode: Ref<VideoMode>
  loading: Ref<boolean>
  error: Ref<boolean>
  errorMessage: Ref<string>
  restarting: Ref<boolean>
  fps: Ref<number>
  mjpegUrl: Ref<string>
  clientId: string
}

export interface UseVideoStreamOptions {
  webrtcVideoRef: Ref<HTMLVideoElement | null>
  mjpegVideoRef: Ref<HTMLImageElement | null>
}

// Retry configuration
const BASE_RETRY_DELAY = 2000
const GRACE_PERIOD = 2000
const MAX_CONSECUTIVE_ERRORS = 2

export function useVideoStream(options: UseVideoStreamOptions) {
  const { t } = useI18n()
  const systemStore = useSystemStore()
  const webrtc = useWebRTC()
  const unifiedAudio = getUnifiedAudio()

  // State
  const videoMode = ref<VideoMode>('mjpeg')
  const videoLoading = ref(true)
  const videoError = ref(false)
  const videoErrorMessage = ref('')
  const videoRestarting = ref(false)
  const backendFps = ref(0)
  const mjpegTimestamp = ref(0)
  const clientId = generateUUID()

  // Per-client statistics
  const clientsStats = ref<Record<string, { id: string; fps: number; connected_secs: number }>>({})

  // Internal state
  let retryTimeoutId: number | null = null
  let retryCount = 0
  let gracePeriodTimeoutId: number | null = null
  let consecutiveErrors = 0
  let isRefreshingVideo = false
  let initialDeviceInfoReceived = false
  let webrtcReconnectTimeout: ReturnType<typeof setTimeout> | null = null

  // Computed
  const mjpegUrl = computed(() => {
    if (videoMode.value !== 'mjpeg') return ''
    if (mjpegTimestamp.value === 0) return ''
    return `${streamApi.getMjpegUrl(clientId)}&t=${mjpegTimestamp.value}`
  })

  const isWebRTCMode = computed(() => videoMode.value !== 'mjpeg')

  // Methods
  function refreshVideo() {
    backendFps.value = 0
    videoError.value = false
    videoErrorMessage.value = ''
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

  function handleVideoLoad() {
    if (videoMode.value === 'mjpeg') {
      systemStore.setStreamOnline(true)
    }

    if (!videoLoading.value) return

    clearRetryTimers()
    videoLoading.value = false
    videoError.value = false
    videoErrorMessage.value = ''
    videoRestarting.value = false
    retryCount = 0
    consecutiveErrors = 0
  }

  function handleVideoError() {
    if (videoMode.value !== 'mjpeg') return
    if (isRefreshingVideo) return

    consecutiveErrors++

    if (consecutiveErrors > MAX_CONSECUTIVE_ERRORS && gracePeriodTimeoutId !== null) {
      clearTimeout(gracePeriodTimeoutId)
      gracePeriodTimeoutId = null
      videoRestarting.value = false
    }

    if (videoRestarting.value || gracePeriodTimeoutId !== null) return

    if (retryTimeoutId !== null) {
      clearTimeout(retryTimeoutId)
      retryTimeoutId = null
    }

    videoLoading.value = true
    retryCount++
    const delay = BASE_RETRY_DELAY * Math.pow(1.5, Math.min(retryCount - 1, 5))

    retryTimeoutId = window.setTimeout(() => {
      retryTimeoutId = null
      refreshVideo()
    }, delay)
  }

  function clearRetryTimers() {
    if (retryTimeoutId !== null) {
      clearTimeout(retryTimeoutId)
      retryTimeoutId = null
    }
    if (gracePeriodTimeoutId !== null) {
      clearTimeout(gracePeriodTimeoutId)
      gracePeriodTimeoutId = null
    }
  }

  async function connectWebRTCOnly(codec: VideoMode = 'h264') {
    clearRetryTimers()
    retryCount = 0
    consecutiveErrors = 0
    mjpegTimestamp.value = 0

    if (options.mjpegVideoRef.value) {
      options.mjpegVideoRef.value.src = ''
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

        if (webrtc.videoTrack.value && options.webrtcVideoRef.value) {
          const stream = webrtc.getMediaStream()
          if (stream) {
            options.webrtcVideoRef.value.srcObject = stream
            try {
              await options.webrtcVideoRef.value.play()
            } catch {
              // AbortError expected when switching modes quickly
            }
          }
        }

        videoLoading.value = false
        videoMode.value = codec
        unifiedAudio.switchMode('webrtc')
      } else {
        throw new Error('WebRTC connection failed')
      }
    } catch {
      videoError.value = true
      videoErrorMessage.value = 'WebRTC connection failed'
      videoLoading.value = false
    }
  }

  async function switchToWebRTC(codec: VideoMode = 'h264') {
    clearRetryTimers()
    retryCount = 0
    consecutiveErrors = 0
    mjpegTimestamp.value = 0

    if (options.mjpegVideoRef.value) {
      options.mjpegVideoRef.value.src = ''
    }

    videoLoading.value = true
    videoError.value = false
    videoErrorMessage.value = ''

    try {
      if (webrtc.isConnected.value || webrtc.sessionId.value) {
        await webrtc.disconnect()
      }

      await streamApi.setMode(codec)
      const success = await webrtc.connect()

      if (success) {
        toast.success(t('console.webrtcConnected'), {
          description: t('console.webrtcConnectedDesc'),
          duration: 3000,
        })

        if (webrtc.videoTrack.value && options.webrtcVideoRef.value) {
          const stream = webrtc.getMediaStream()
          if (stream) {
            options.webrtcVideoRef.value.srcObject = stream
            try {
              await options.webrtcVideoRef.value.play()
            } catch {
              // AbortError expected
            }
          }
        }

        videoLoading.value = false
        unifiedAudio.switchMode('webrtc')
      } else {
        throw new Error('WebRTC connection failed')
      }
    } catch {
      videoError.value = true
      videoErrorMessage.value = t('console.webrtcFailed')
      videoLoading.value = false

      toast.error(t('console.webrtcFailed'), {
        description: t('console.fallingBackToMjpeg'),
        duration: 5000,
      })
      videoMode.value = 'mjpeg'
    }
  }

  async function switchToMJPEG() {
    videoLoading.value = true
    videoError.value = false
    videoErrorMessage.value = ''

    try {
      await streamApi.setMode('mjpeg')
    } catch {
      // Continue anyway
    }

    if (webrtc.isConnected.value) {
      webrtc.disconnect()
    }

    if (options.webrtcVideoRef.value) {
      options.webrtcVideoRef.value.srcObject = null
    }

    unifiedAudio.switchMode('ws')
    refreshVideo()
  }

  function handleModeChange(mode: VideoMode) {
    if (mode === videoMode.value) return

    if (mode !== 'mjpeg') {
      mjpegTimestamp.value = 0
    }

    videoMode.value = mode
    localStorage.setItem('videoMode', mode)

    if (mode !== 'mjpeg') {
      switchToWebRTC(mode)
    } else {
      switchToMJPEG()
    }
  }

  // Handle stream config events
  function handleStreamConfigChanging(data: { reason?: string }) {
    clearRetryTimers()
    videoRestarting.value = true
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

  function handleStreamConfigApplied(data: { device: string; resolution: [number, number]; fps: number }) {
    consecutiveErrors = 0

    gracePeriodTimeoutId = window.setTimeout(() => {
      gracePeriodTimeoutId = null
      consecutiveErrors = 0
    }, GRACE_PERIOD)

    videoRestarting.value = false

    if (videoMode.value !== 'mjpeg') {
      switchToWebRTC(videoMode.value)
    } else {
      refreshVideo()
    }

    toast.success(t('console.videoRestarted'), {
      description: `${data.device} - ${data.resolution[0]}x${data.resolution[1]} @ ${data.fps}fps`,
      duration: 3000,
    })
  }

  function handleStreamStatsUpdate(data: { clients?: number; clients_stat?: Record<string, { fps: number }> }) {
    if (typeof data.clients === 'number') {
      systemStore.updateStreamClients(data.clients)
    }

    if (videoMode.value !== 'mjpeg') {
      if (data.clients_stat) {
        clientsStats.value = data.clients_stat as any
      }
      return
    }

    if (data.clients_stat) {
      clientsStats.value = data.clients_stat as any
      const myStats = data.clients_stat[clientId]
      if (myStats) {
        backendFps.value = myStats.fps || 0
      } else {
        const fpsList = Object.values(data.clients_stat)
          .map((s) => s?.fps || 0)
          .filter(f => f > 0)
        backendFps.value = fpsList.length > 0 ? Math.min(...fpsList) : 0
      }
    } else {
      backendFps.value = 0
    }
  }

  function handleDeviceInfo(data: any) {
    systemStore.updateFromDeviceInfo(data)

    if (data.video?.config_changing) return

    if (data.video?.stream_mode) {
      const serverStreamMode = data.video.stream_mode
      const serverMode = serverStreamMode === 'webrtc' ? 'h264' : serverStreamMode as VideoMode

      if (!initialDeviceInfoReceived) {
        initialDeviceInfoReceived = true

        if (serverMode !== videoMode.value) {
          videoMode.value = serverMode
          if (serverMode !== 'mjpeg') {
            setTimeout(() => connectWebRTCOnly(serverMode), 100)
          } else {
            setTimeout(() => refreshVideo(), 100)
          }
        } else if (serverMode !== 'mjpeg') {
          setTimeout(() => connectWebRTCOnly(serverMode), 100)
        } else {
          setTimeout(() => refreshVideo(), 100)
        }
      } else if (serverMode !== videoMode.value) {
        handleModeChange(serverMode)
      }
    }
  }

  function handleStreamModeChanged(data: { mode: string; previous_mode: string }) {
    const newMode = data.mode === 'webrtc' ? 'h264' : data.mode as VideoMode

    toast.info(t('console.streamModeChanged'), {
      description: t('console.streamModeChangedDesc', { mode: data.mode.toUpperCase() }),
      duration: 5000,
    })

    if (newMode !== videoMode.value) {
      handleModeChange(newMode)
    }
  }

  // Watch WebRTC video track
  watch(() => webrtc.videoTrack.value, async (track) => {
    if (track && options.webrtcVideoRef.value && videoMode.value !== 'mjpeg') {
      const stream = webrtc.getMediaStream()
      if (stream) {
        options.webrtcVideoRef.value.srcObject = stream
        try {
          await options.webrtcVideoRef.value.play()
        } catch {
          // AbortError expected
        }
      }
    }
  })

  // Watch WebRTC audio track
  watch(() => webrtc.audioTrack.value, (track) => {
    if (track && options.webrtcVideoRef.value && videoMode.value !== 'mjpeg') {
      const currentStream = options.webrtcVideoRef.value.srcObject as MediaStream | null
      if (currentStream && currentStream.getAudioTracks().length === 0) {
        currentStream.addTrack(track)
      }
    }
  })

  // Watch WebRTC element for unified audio
  watch(options.webrtcVideoRef, (el) => {
    unifiedAudio.setWebRTCElement(el)
  }, { immediate: true })

  // Watch WebRTC stats for FPS
  watch(webrtc.stats, (stats) => {
    if (videoMode.value !== 'mjpeg' && stats.framesPerSecond > 0) {
      backendFps.value = Math.round(stats.framesPerSecond)
      systemStore.setStreamOnline(true)
    }
  }, { deep: true })

  // Watch WebRTC state for auto-reconnect
  watch(() => webrtc.state.value, (newState, oldState) => {
    if (videoMode.value !== 'mjpeg') {
      if (newState === 'connected') {
        systemStore.setStreamOnline(true)
      }
    }

    if (webrtcReconnectTimeout) {
      clearTimeout(webrtcReconnectTimeout)
      webrtcReconnectTimeout = null
    }

    if (newState === 'disconnected' && oldState === 'connected' && videoMode.value !== 'mjpeg') {
      webrtcReconnectTimeout = setTimeout(async () => {
        if (videoMode.value !== 'mjpeg' && webrtc.state.value === 'disconnected') {
          try {
            await webrtc.connect()
          } catch {
            // Will retry on next disconnect
          }
        }
      }, 1000)
    }
  })

  // Cleanup
  function cleanup() {
    clearRetryTimers()
    if (webrtcReconnectTimeout) {
      clearTimeout(webrtcReconnectTimeout)
    }
  }

  return {
    // State
    mode: videoMode,
    loading: videoLoading,
    error: videoError,
    errorMessage: videoErrorMessage,
    restarting: videoRestarting,
    fps: backendFps,
    mjpegUrl,
    clientId,
    clientsStats,
    isWebRTCMode,

    // WebRTC access
    webrtc,

    // Methods
    refreshVideo,
    handleVideoLoad,
    handleVideoError,
    handleModeChange,
    connectWebRTCOnly,
    switchToWebRTC,
    switchToMJPEG,

    // Event handlers
    handleStreamConfigChanging,
    handleStreamConfigApplied,
    handleStreamStatsUpdate,
    handleDeviceInfo,
    handleStreamModeChanged,

    // Cleanup
    cleanup,
  }
}
