// API client for One-KVM backend

import { request, ApiError } from './request'

const API_BASE = '/api'

// Auth API
export const authApi = {
  login: (username: string, password: string) =>
    request<{ success: boolean; message?: string }>('/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    }),

  logout: () =>
    request<{ success: boolean }>('/auth/logout', { method: 'POST' }),

  check: () =>
    request<{ authenticated: boolean; user?: string }>('/auth/check'),

  changePassword: (currentPassword: string, newPassword: string) =>
    request<{ success: boolean }>('/auth/password', {
      method: 'POST',
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    }),

  changeUsername: (username: string, currentPassword: string) =>
    request<{ success: boolean }>('/auth/username', {
      method: 'POST',
      body: JSON.stringify({ username, current_password: currentPassword }),
    }),
}

// System API
export interface NetworkAddress {
  interface: string
  ip: string
}

export interface DeviceInfo {
  hostname: string
  cpu_model: string
  cpu_usage: number
  memory_total: number
  memory_used: number
  network_addresses: NetworkAddress[]
}

export const systemApi = {
  info: () =>
    request<{
      version: string
      build_date: string
      initialized: boolean
      capabilities: {
        video: { available: boolean; backend?: string }
        hid: { available: boolean; backend?: string }
        msd: { available: boolean }
        atx: { available: boolean; backend?: string }
        audio: { available: boolean; backend?: string }
      }
      disk_space?: {
        total: number
        available: number
        used: number
      }
      device_info?: DeviceInfo
    }>('/info'),

  health: () => request<{ status: string; version: string }>('/health'),

  setupStatus: () =>
    request<{ initialized: boolean; needs_setup: boolean }>('/setup'),

  setup: (data: {
    username: string
    password: string
    video_device?: string
    video_format?: string
    video_width?: number
    video_height?: number
    video_fps?: number
    hid_backend?: string
    hid_ch9329_port?: string
    hid_ch9329_baudrate?: number
    hid_otg_udc?: string
    hid_otg_profile?: string
    encoder_backend?: string
    audio_device?: string
    ttyd_enabled?: boolean
    rustdesk_enabled?: boolean
  }) =>
    request<{ success: boolean; message?: string }>('/setup/init', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  restart: () =>
    request<{ success: boolean; message?: string }>('/system/restart', {
      method: 'POST',
    }),
}

// Stream API
export interface VideoCodecInfo {
  id: string
  name: string
  protocol: 'http' | 'webrtc'
  hardware: boolean
  backend: string | null
  available: boolean
}

export interface EncoderBackendInfo {
  id: string
  name: string
  is_hardware: boolean
  supported_formats: string[]
}

export interface AvailableCodecsResponse {
  success: boolean
  backends: EncoderBackendInfo[]
  codecs: VideoCodecInfo[]
}

export interface StreamConstraintsResponse {
  success: boolean
  allowed_codecs: string[]
  locked_codec: string | null
  disallow_mjpeg: boolean
  sources: {
    rustdesk: boolean
    rtsp: boolean
  }
  reason: string
  current_mode: string
}

export const streamApi = {
  status: () =>
    request<{
      state: 'uninitialized' | 'ready' | 'streaming' | 'no_signal' | 'error'
      device: string | null
      format: string | null
      resolution: [number, number] | null
      clients: number
      target_fps: number
      fps: number
    }>('/stream/status'),

  start: () =>
    request<{ success: boolean }>('/stream/start', { method: 'POST' }),

  stop: () =>
    request<{ success: boolean }>('/stream/stop', { method: 'POST' }),

  getMjpegUrl: (clientId?: string) => {
    const base = `${API_BASE}/stream/mjpeg`
    return clientId ? `${base}?client_id=${clientId}` : base
  },

  getSnapshotUrl: () => `${API_BASE}/snapshot`,

  getMode: () =>
    request<{ success: boolean; mode: string; transition_id?: string; switching?: boolean; message?: string }>('/stream/mode'),

  setMode: (mode: string) =>
    request<{ success: boolean; mode: string; transition_id?: string; switching?: boolean; message?: string }>('/stream/mode', {
      method: 'POST',
      body: JSON.stringify({ mode }),
    }),

  getCodecs: () =>
    request<AvailableCodecsResponse>('/stream/codecs'),

  getConstraints: () =>
    request<StreamConstraintsResponse>('/stream/constraints'),

  setBitratePreset: (bitrate_preset: import('@/types/generated').BitratePreset) =>
    request<{ success: boolean; message?: string }>('/stream/bitrate', {
      method: 'POST',
      body: JSON.stringify({ bitrate_preset }),
    }),
}

// WebRTC API
export interface IceCandidate {
  candidate: string
  sdpMid?: string
  sdpMLineIndex?: number
  usernameFragment?: string
}

export interface IceServerConfig {
  urls: string[]
  username?: string
  credential?: string
}

export const webrtcApi = {
  createSession: () =>
    request<{ session_id: string }>('/webrtc/session', { method: 'POST' }),

  offer: (sdp: string, clientId?: string) =>
    request<{ sdp: string; session_id: string; ice_candidates: IceCandidate[] }>('/webrtc/offer', {
      method: 'POST',
      body: JSON.stringify({ sdp, client_id: clientId }),
    }),

  addIceCandidate: (sessionId: string, candidate: IceCandidate) =>
    request<{ success: boolean }>('/webrtc/ice', {
      method: 'POST',
      body: JSON.stringify({ session_id: sessionId, candidate }),
    }),

  status: () =>
    request<{
      session_count: number
      sessions: Array<{ session_id: string; state: string }>
    }>('/webrtc/status'),

  close: (sessionId: string) =>
    request<{ success: boolean }>('/webrtc/close', {
      method: 'POST',
      body: JSON.stringify({ session_id: sessionId }),
    }),

  getIceServers: () =>
    request<{ ice_servers: IceServerConfig[]; mdns_mode: string }>('/webrtc/ice-servers'),
}

// HID API
// Import HID WebSocket composable
import { useHidWebSocket, type HidKeyboardEvent, type HidMouseEvent } from '@/composables/useHidWebSocket'

// Create shared HID WebSocket instance
const hidWs = useHidWebSocket()
let hidWsInitialized = false

// Initialize HID WebSocket connection
async function ensureHidConnection() {
  if (!hidWsInitialized) {
    hidWsInitialized = true
    await hidWs.connect()
  }
}

// Map button string to number
function mapButton(button?: 'left' | 'right' | 'middle'): number | undefined {
  if (!button) return undefined
  const buttonMap = { left: 0, middle: 1, right: 2 }
  return buttonMap[button]
}

export const hidApi = {
  status: () =>
    request<{
      available: boolean
      backend: string
      initialized: boolean
      supports_absolute_mouse: boolean
      screen_resolution: [number, number] | null
    }>('/hid/status'),

  keyboard: async (type: 'down' | 'up', key: number, modifiers?: {
    ctrl?: boolean
    shift?: boolean
    alt?: boolean
    meta?: boolean
  }) => {
    await ensureHidConnection()
    const event: HidKeyboardEvent = {
      type: type === 'down' ? 'keydown' : 'keyup',
      key,
      modifiers,
    }
    await hidWs.sendKeyboard(event)
    return { success: true }
  },

  mouse: async (data: {
    type: 'move' | 'move_abs' | 'down' | 'up' | 'scroll'
    x?: number | null
    y?: number | null
    button?: 'left' | 'right' | 'middle' | null
    scroll?: number | null
  }) => {
    await ensureHidConnection()
    // Ensure all values are properly typed (convert null to undefined)
    const event: HidMouseEvent = {
      type: data.type === 'move_abs' ? 'moveabs' : data.type,
      x: data.x ?? undefined,
      y: data.y ?? undefined,
      button: mapButton(data.button ?? undefined),
      scroll: data.scroll ?? undefined,
    }
    await hidWs.sendMouse(event)
    return { success: true }
  },

  reset: () =>
    request<{ success: boolean }>('/hid/reset', { method: 'POST' }),

  consumer: async (usage: number) => {
    await ensureHidConnection()
    await hidWs.sendConsumer({ usage })
    return { success: true }
  },

  // WebSocket connection management
  connectWebSocket: () => hidWs.connect(),
  disconnectWebSocket: () => hidWs.disconnect(),
  isWebSocketConnected: () => hidWs.connected.value,
}

// ATX API
export const atxApi = {
  status: () =>
    request<{
      available: boolean
      backend: string
      initialized: boolean
      power_status: 'on' | 'off' | 'unknown'
      led_supported: boolean
    }>('/atx/status'),

  power: (action: 'short' | 'long' | 'reset') =>
    request<{ success: boolean; message?: string }>('/atx/power', {
      method: 'POST',
      body: JSON.stringify({ action }),
    }),
}

// MSD API
export interface MsdImage {
  id: string
  name: string
  size: number
  created_at: string
}

export interface DriveFile {
  name: string
  path: string
  is_dir: boolean
  size: number
}

export const msdApi = {
  status: () =>
    request<{
      available: boolean
      state: {
        connected: boolean
        mode: 'none' | 'image' | 'drive'
        current_image: {
          id: string
          name: string
          size: number
          created_at: string
        } | null
        drive_info: {
          size: number
          used: number
          free: number
          initialized: boolean
        } | null
      }
    }>('/msd/status'),

  // Image management
  listImages: () => request<MsdImage[]>('/msd/images'),

  uploadImage: async (file: File, onProgress?: (progress: number) => void) => {
    const formData = new FormData()
    formData.append('file', file)

    const xhr = new XMLHttpRequest()
    xhr.open('POST', `${API_BASE}/msd/images`)
    xhr.withCredentials = true

    return new Promise<MsdImage>((resolve, reject) => {
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable && onProgress) {
          onProgress((e.loaded / e.total) * 100)
        }
      }

      xhr.onload = () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          resolve(JSON.parse(xhr.responseText))
        } else {
          reject(new ApiError(xhr.status, 'Upload failed'))
        }
      }

      xhr.onerror = () => reject(new ApiError(0, 'Network error'))
      xhr.send(formData)
    })
  },

  deleteImage: (id: string) =>
    request<{ success: boolean }>(`/msd/images/${id}`, { method: 'DELETE' }),

  connect: (mode: 'image' | 'drive', imageId?: string, cdrom?: boolean, readOnly?: boolean) =>
    request<{ success: boolean }>('/msd/connect', {
      method: 'POST',
      body: JSON.stringify({ mode, image_id: imageId, cdrom, read_only: readOnly }),
    }),

  disconnect: () =>
    request<{ success: boolean }>('/msd/disconnect', { method: 'POST' }),

  // Virtual drive
  driveInfo: () =>
    request<{
      size: number
      used: number
      free: number
      initialized: boolean
    }>('/msd/drive'),

  initDrive: (sizeMb?: number) =>
    request<{ path: string; size_mb: number }>('/msd/drive/init', {
      method: 'POST',
      body: JSON.stringify({ size_mb: sizeMb }),
    }),

  deleteDrive: () =>
    request<{ success: boolean }>('/msd/drive', { method: 'DELETE' }),

  listDriveFiles: (path = '/') =>
    request<DriveFile[]>(`/msd/drive/files?path=${encodeURIComponent(path)}`),

  uploadDriveFile: async (file: File, targetPath = '/', onProgress?: (progress: number) => void) => {
    const formData = new FormData()
    formData.append('file', file)

    const xhr = new XMLHttpRequest()
    xhr.open('POST', `${API_BASE}/msd/drive/files?path=${encodeURIComponent(targetPath)}`)
    xhr.withCredentials = true

    return new Promise<{ success: boolean; message?: string }>((resolve, reject) => {
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable && onProgress) {
          onProgress((e.loaded / e.total) * 100)
        }
      }

      xhr.onload = () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          resolve(JSON.parse(xhr.responseText))
        } else {
          reject(new ApiError(xhr.status, 'Upload failed'))
        }
      }

      xhr.onerror = () => reject(new ApiError(0, 'Network error'))
      xhr.send(formData)
    })
  },

  downloadDriveFile: (path: string) =>
    `${API_BASE}/msd/drive/files${path.startsWith('/') ? path : '/' + path}`,

  deleteDriveFile: (path: string) =>
    request<{ success: boolean }>(`/msd/drive/files${path.startsWith('/') ? path : '/' + path}`, {
      method: 'DELETE',
    }),

  createDirectory: (path: string) =>
    request<{ success: boolean }>(`/msd/drive/mkdir${path.startsWith('/') ? path : '/' + path}`, {
      method: 'POST',
    }),

  // Download from URL
  downloadFromUrl: (url: string, filename?: string) =>
    request<{
      download_id: string
      url: string
      filename: string
      bytes_downloaded: number
      total_bytes: number | null
      progress_pct: number | null
      status: string
      error: string | null
    }>('/msd/images/download', {
      method: 'POST',
      body: JSON.stringify({ url, filename }),
    }),

  cancelDownload: (downloadId: string) =>
    request<{ success: boolean }>('/msd/images/download/cancel', {
      method: 'POST',
      body: JSON.stringify({ download_id: downloadId }),
    }),
}

// Config API
/** @deprecated 使用域特定 API（videoConfigApi, hidConfigApi 等）替代 */
export const configApi = {
  get: () => request<Record<string, unknown>>('/config'),

  /** @deprecated 使用域特定 API 的 update 方法替代 */
  update: (updates: Record<string, unknown>) =>
    request<{ success: boolean }>('/config', {
      method: 'POST',
      body: JSON.stringify(updates),
    }),

  listDevices: () =>
    request<{
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
        usb_bus: string | null
      }>
      serial: Array<{ path: string; name: string }>
      audio: Array<{
        name: string
        description: string
        is_hdmi: boolean
        usb_bus: string | null
      }>
      udc: Array<{ name: string }>
      extensions: {
        ttyd_available: boolean
        rustdesk_available: boolean
      }
    }>('/devices'),
}

// 导出新的域分离配置 API
export {
  authConfigApi,
  videoConfigApi,
  streamConfigApi,
  hidConfigApi,
  msdConfigApi,
  atxConfigApi,
  audioConfigApi,
  extensionsApi,
  rustdeskConfigApi,
  rtspConfigApi,
  webConfigApi,
  type RustDeskConfigResponse,
  type RustDeskStatusResponse,
  type RustDeskConfigUpdate,
  type RustDeskPasswordResponse,
  type RtspConfigResponse,
  type RtspConfigUpdate,
  type RtspStatusResponse,
  type WebConfig,
} from './config'

// 导出生成的类型
export type {
  AppConfig,
  AuthConfig,
  AuthConfigUpdate,
  VideoConfig,
  VideoConfigUpdate,
  StreamConfig,
  StreamConfigUpdate,
  HidConfig,
  HidConfigUpdate,
  MsdConfig,
  MsdConfigUpdate,
  AtxConfig,
  AtxConfigUpdate,
  AudioConfig,
  AudioConfigUpdate,
  HidBackend,
  StreamMode,
  EncoderType,
  BitratePreset,
} from '@/types/generated'

// Audio API
export const audioApi = {
  status: () =>
    request<{
      enabled: boolean
      streaming: boolean
      device: string | null
      sample_rate: number
      channels: number
      quality: string
      subscriber_count: number
      frames_encoded: number
      bytes_output: number
      error: string | null
    }>('/audio/status'),

  start: () =>
    request<{ success: boolean }>('/audio/start', { method: 'POST' }),

  stop: () =>
    request<{ success: boolean }>('/audio/stop', { method: 'POST' }),

  setQuality: (quality: 'voice' | 'balanced' | 'high') =>
    request<{ success: boolean }>('/audio/quality', {
      method: 'POST',
      body: JSON.stringify({ quality }),
    }),

  selectDevice: (device: string) =>
    request<{ success: boolean }>('/audio/device', {
      method: 'POST',
      body: JSON.stringify({ device }),
    }),
}

export { ApiError }
