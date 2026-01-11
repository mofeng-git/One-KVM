// Legacy MJPEG-only streaming composable.
// Deprecated: Console now uses useVideoSession for all switching/connection logic.

import { ref, computed, type Ref } from 'vue'
import { useSystemStore } from '@/stores/system'
import { streamApi } from '@/api'
import { generateUUID } from '@/lib/utils'

export type VideoMode = 'mjpeg'

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

const BASE_RETRY_DELAY = 2000
const GRACE_PERIOD = 2000
const MAX_CONSECUTIVE_ERRORS = 2

/** @deprecated Use useVideoSession + ConsoleView instead. */
export function useVideoStream() {
  const systemStore = useSystemStore()

  const videoMode = ref<VideoMode>('mjpeg')
  const videoLoading = ref(true)
  const videoError = ref(false)
  const videoErrorMessage = ref('')
  const videoRestarting = ref(false)
  const backendFps = ref(0)
  const mjpegTimestamp = ref(0)
  const clientId = generateUUID()
  const clientsStats = ref<Record<string, { id: string; fps: number; connected_secs: number }>>({})

  let retryTimeoutId: number | null = null
  let retryCount = 0
  let gracePeriodTimeoutId: number | null = null
  let consecutiveErrors = 0
  let isRefreshingVideo = false
  let initialDeviceInfoReceived = false

  const mjpegUrl = computed(() => {
    if (videoMode.value !== 'mjpeg') return ''
    if (mjpegTimestamp.value === 0) return ''
    return `${streamApi.getMjpegUrl(clientId)}&t=${mjpegTimestamp.value}`
  })

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

    consecutiveErrors += 1

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
    retryCount += 1
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

  function handleStreamConfigChanging() {
    clearRetryTimers()
    videoRestarting.value = true
    videoLoading.value = true
    videoError.value = false
    retryCount = 0
    consecutiveErrors = 0
    backendFps.value = 0
  }

  function handleStreamConfigApplied() {
    consecutiveErrors = 0

    gracePeriodTimeoutId = window.setTimeout(() => {
      gracePeriodTimeoutId = null
      consecutiveErrors = 0
    }, GRACE_PERIOD)

    videoRestarting.value = false
    refreshVideo()
  }

  function handleStreamStatsUpdate(data: { clients?: number; clients_stat?: Record<string, { fps: number }> }) {
    if (typeof data.clients === 'number') {
      systemStore.updateStreamClients(data.clients)
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

    if (!initialDeviceInfoReceived) {
      initialDeviceInfoReceived = true
      if (data.video?.stream_mode === 'mjpeg') {
        setTimeout(() => refreshVideo(), 100)
      }
    }
  }

  function handleModeChange(mode: VideoMode) {
    if (mode !== 'mjpeg') return
    if (mode === videoMode.value) return
    videoMode.value = mode
    localStorage.setItem('videoMode', mode)
    refreshVideo()
  }

  function cleanup() {
    clearRetryTimers()
  }

  const state: VideoStreamState = {
    mode: videoMode,
    loading: videoLoading,
    error: videoError,
    errorMessage: videoErrorMessage,
    restarting: videoRestarting,
    fps: backendFps,
    mjpegUrl,
    clientId,
  }

  return {
    state,
    clientsStats,
    refreshVideo,
    handleVideoLoad,
    handleVideoError,
    handleStreamConfigChanging,
    handleStreamConfigApplied,
    handleStreamStatsUpdate,
    handleDeviceInfo,
    handleModeChange,
    cleanup,
  }
}
