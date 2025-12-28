import { defineStore } from 'pinia'
import { ref } from 'vue'
import { systemApi, streamApi, hidApi, atxApi, msdApi, type DeviceInfo } from '@/api'

interface SystemCapabilities {
  video: { available: boolean; backend?: string }
  hid: { available: boolean; backend?: string }
  msd: { available: boolean }
  atx: { available: boolean; backend?: string }
  audio: { available: boolean; backend?: string }
}

interface DiskSpaceInfo {
  total: number
  available: number
  used: number
}

interface StreamState {
  online: boolean
  active: boolean
  device: string | null
  format: string | null
  resolution: [number, number] | null
  targetFps: number
  clients: number
  framesCaptured: number
  framesDropped: number
  streamMode: string  // 'mjpeg' or 'webrtc'
  error: string | null
}

interface HidState {
  available: boolean
  backend: string
  initialized: boolean
  supportsAbsoluteMouse: boolean
  device: string | null
  error: string | null
}

interface AtxState {
  available: boolean
  backend: string
  initialized: boolean
  powerOn: boolean
  error: string | null
}

interface MsdState {
  available: boolean
  connected: boolean
  mode: 'none' | 'image' | 'drive'
  imageId: string | null
  error: string | null
}

interface AudioState {
  available: boolean
  streaming: boolean
  device: string | null
  quality: string
  error: string | null
}

interface ConnectionState {
  wsConnected: boolean
  hidWsConnected: boolean
  wsNetworkError: boolean
  hidWsNetworkError: boolean
}

// DeviceInfo event payload types (from WebSocket)
export interface VideoDeviceInfo {
  available: boolean
  device: string | null
  format: string | null
  resolution: [number, number] | null
  fps: number
  online: boolean
  stream_mode: string  // 'mjpeg' or 'webrtc'
  config_changing?: boolean
  error: string | null
}

export interface HidDeviceInfo {
  available: boolean
  backend: string
  initialized: boolean
  supports_absolute_mouse: boolean
  device: string | null
  error: string | null
}

export interface MsdDeviceInfo {
  available: boolean
  mode: string
  connected: boolean
  image_id: string | null
  error: string | null
}

export interface AtxDeviceInfo {
  available: boolean
  backend: string
  initialized: boolean
  power_on: boolean
  error: string | null
}

export interface AudioDeviceInfo {
  available: boolean
  streaming: boolean
  device: string | null
  quality: string
  error: string | null
}

export interface DeviceInfoEvent {
  video: VideoDeviceInfo
  hid: HidDeviceInfo
  msd: MsdDeviceInfo | null
  atx: AtxDeviceInfo | null
  audio: AudioDeviceInfo | null
}

export const useSystemStore = defineStore('system', () => {
  const version = ref<string>('')
  const buildDate = ref<string>('')
  const capabilities = ref<SystemCapabilities | null>(null)
  const diskSpace = ref<DiskSpaceInfo | null>(null)
  const deviceInfo = ref<DeviceInfo | null>(null)
  const stream = ref<StreamState | null>(null)
  const hid = ref<HidState | null>(null)
  const atx = ref<AtxState | null>(null)
  const msd = ref<MsdState | null>(null)
  const audio = ref<AudioState | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const connectionState = ref<ConnectionState>({
    wsConnected: false,
    hidWsConnected: false,
    wsNetworkError: false,
    hidWsNetworkError: false,
  })

  async function fetchSystemInfo() {
    try {
      const info = await systemApi.info()
      version.value = info.version
      buildDate.value = info.build_date
      capabilities.value = info.capabilities
      diskSpace.value = info.disk_space ?? null
      deviceInfo.value = info.device_info ?? null
      return info
    } catch (e) {
      console.error('Failed to fetch system info:', e)
      throw e
    }
  }

  async function startStream() {
    try {
      await streamApi.start()
    } catch (e) {
      console.error('Failed to start stream:', e)
      throw e
    }
  }

  async function stopStream() {
    try {
      await streamApi.stop()
    } catch (e) {
      console.error('Failed to stop stream:', e)
      throw e
    }
  }

  async function fetchHidState() {
    try {
      const state = await hidApi.status()
      hid.value = {
        available: state.available,
        backend: state.backend,
        initialized: state.initialized,
        supportsAbsoluteMouse: state.supports_absolute_mouse,
        device: null,
        error: null,
      }
      return state
    } catch (e) {
      console.error('Failed to fetch HID state:', e)
      throw e
    }
  }

  async function fetchAtxState() {
    try {
      const state = await atxApi.status()
      atx.value = {
        available: state.available,
        backend: state.backend,
        initialized: state.initialized,
        powerOn: state.power_status === 'on',
        error: null,
      }
      return state
    } catch (e) {
      console.error('Failed to fetch ATX state:', e)
      throw e
    }
  }

  async function fetchMsdState() {
    try {
      const result = await msdApi.status()
      msd.value = {
        available: result.available,
        connected: result.state.connected,
        mode: result.state.mode,
        imageId: result.state.current_image?.id ?? null,
        error: null,
      }
      return result
    } catch (e) {
      console.error('Failed to fetch MSD state:', e)
      throw e
    }
  }

  async function fetchAllStates() {
    loading.value = true
    error.value = null

    try {
      await Promise.all([
        fetchSystemInfo(),
        // HID state is updated via WebSocket device_info event
        fetchAtxState().catch(() => null),
        fetchMsdState().catch(() => null),
      ])
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch states'
    } finally {
      loading.value = false
    }
  }

  /**
   * Update WebSocket connection state
   */
  function updateWsConnection(connected: boolean, networkError: boolean) {
    connectionState.value.wsConnected = connected
    connectionState.value.wsNetworkError = networkError
  }

  /**
   * Update HID WebSocket connection state
   */
  function updateHidWsConnection(connected: boolean, networkError: boolean) {
    connectionState.value.hidWsConnected = connected
    connectionState.value.hidWsNetworkError = networkError
  }

  /**
   * Update store state from WebSocket DeviceInfo event
   * Called when receiving system.device_info event from server
   */
  function updateFromDeviceInfo(data: DeviceInfoEvent) {
    // Update video/stream state
    stream.value = {
      online: data.video.online,
      active: data.video.online || data.video.available,
      device: data.video.device,
      format: data.video.format,
      resolution: data.video.resolution,
      targetFps: data.video.fps,
      clients: stream.value?.clients ?? 0,
      framesCaptured: stream.value?.framesCaptured ?? 0,
      framesDropped: stream.value?.framesDropped ?? 0,
      streamMode: data.video.stream_mode || 'mjpeg',
      error: data.video.error,
    }

    // Update HID state
    hid.value = {
      available: data.hid.available,
      backend: data.hid.backend,
      initialized: data.hid.initialized,
      supportsAbsoluteMouse: data.hid.supports_absolute_mouse,
      device: data.hid.device,
      error: data.hid.error,
    }

    // Update MSD state (optional)
    if (data.msd) {
      msd.value = {
        available: data.msd.available,
        connected: data.msd.connected,
        mode: data.msd.mode as 'none' | 'image' | 'drive',
        imageId: data.msd.image_id,
        error: data.msd.error,
      }
    } else {
      msd.value = null
    }

    // Update ATX state (optional)
    if (data.atx) {
      atx.value = {
        available: data.atx.available,
        backend: data.atx.backend,
        initialized: data.atx.initialized,
        powerOn: data.atx.power_on,
        error: data.atx.error,
      }
    } else {
      atx.value = null
    }

    // Update Audio state (optional)
    if (data.audio) {
      audio.value = {
        available: data.audio.available,
        streaming: data.audio.streaming,
        device: data.audio.device,
        quality: data.audio.quality,
        error: data.audio.error,
      }
    } else {
      audio.value = null
    }
  }

  /**
   * Update stream clients count from WebSocket stream.stats_update event
   */
  function updateStreamClients(clients: number) {
    if (stream.value) {
      stream.value = {
        ...stream.value,
        clients,
      }
    }
  }

  /**
   * Set stream online status
   * Called when MJPEG video successfully loads (frontend-side detection)
   * This fixes the timing issue where device_info event arrives before stream is fully active
   */
  function setStreamOnline(online: boolean) {
    if (stream.value && stream.value.online !== online) {
      console.log('[Store] setStreamOnline:', online, '(was:', stream.value.online, ')')
      stream.value = {
        ...stream.value,
        online,
      }
    }
  }

  return {
    version,
    buildDate,
    capabilities,
    diskSpace,
    deviceInfo,
    stream,
    hid,
    atx,
    msd,
    audio,
    loading,
    error,
    connectionState,
    fetchSystemInfo,
    startStream,
    stopStream,
    fetchHidState,
    fetchAtxState,
    fetchMsdState,
    fetchAllStates,
    updateWsConnection,
    updateHidWsConnection,
    updateFromDeviceInfo,
    updateStreamClients,
    setStreamOnline,
  }
})
