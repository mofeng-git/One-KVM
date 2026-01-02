// API client for One-KVM backend

import { toast } from 'vue-sonner'

const API_BASE = '/api'

// Toast debounce mechanism - prevent toast spam (5 seconds)
const toastDebounceMap = new Map<string, number>()
const TOAST_DEBOUNCE_TIME = 5000

function shouldShowToast(key: string): boolean {
  const now = Date.now()
  const lastToastTime = toastDebounceMap.get(key)

  if (!lastToastTime || now - lastToastTime >= TOAST_DEBOUNCE_TIME) {
    toastDebounceMap.set(key, now)
    return true
  }

  return false
}

class ApiError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

async function request<T>(
  endpoint: string,
  options: RequestInit = {}
): Promise<T> {
  const url = `${API_BASE}${endpoint}`

  try {
    const response = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
      credentials: 'include',
    })

    // Parse response body (all responses are 200 OK with success field)
    const data = await response.json().catch(() => ({
      success: false,
      message: 'Failed to parse response'
    }))

    // Check success field - all errors are indicated by success=false
    if (data && typeof data.success === 'boolean' && !data.success) {
      const errorMessage = data.message || 'Operation failed'
      const apiError = new ApiError(response.status, errorMessage)

      console.info(`[API] ${endpoint} failed:`, errorMessage)

      // Show toast notification to user (with debounce)
      if (shouldShowToast(`error_${endpoint}`)) {
        toast.error('Operation Failed', {
          description: errorMessage,
          duration: 4000,
        })
      }

      throw apiError
    }

    return data
  } catch (error) {
    // Network errors or JSON parsing errors
    if (error instanceof ApiError) {
      throw error // Already handled above
    }

    // Network connectivity issues
    console.info(`[API] Network error for ${endpoint}:`, error)

    // Show toast for network errors (with debounce)
    if (shouldShowToast('network_error')) {
      toast.error('Network Error', {
        description: 'Unable to connect to server. Please check your connection.',
        duration: 4000,
      })
    }

    throw new ApiError(0, 'Network error')
  }
}

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
    request<{ authenticated: boolean; user?: string; is_admin?: boolean }>('/auth/check'),
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
    encoder_backend?: string
  }) =>
    request<{ success: boolean; message?: string }>('/setup/init', {
      method: 'POST',
      body: JSON.stringify(data),
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
      frames_captured: number
      frames_dropped: number
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
    request<{ success: boolean; mode: string; message?: string }>('/stream/mode'),

  setMode: (mode: string) =>
    request<{ success: boolean; mode: string; message?: string }>('/stream/mode', {
      method: 'POST',
      body: JSON.stringify({ mode }),
    }),

  getCodecs: () =>
    request<AvailableCodecsResponse>('/stream/codecs'),

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
    request<{ ice_servers: IceServerConfig[] }>('/webrtc/ice-servers'),
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
      }>
      serial: Array<{ path: string; name: string }>
      audio: Array<{ name: string; description: string }>
      udc: Array<{ name: string }>
    }>('/devices'),
}

// 导出新的域分离配置 API
export {
  videoConfigApi,
  streamConfigApi,
  hidConfigApi,
  msdConfigApi,
  atxConfigApi,
  audioConfigApi,
  extensionsApi,
  rustdeskConfigApi,
  type RustDeskConfigResponse,
  type RustDeskStatusResponse,
  type RustDeskConfigUpdate,
  type RustDeskPasswordResponse,
} from './config'

// 导出生成的类型
export type {
  AppConfig,
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

// User Management API
export interface User {
  id: string
  username: string
  role: 'admin' | 'user'
  created_at: string
}

interface UserApiResponse {
  id: string
  username: string
  is_admin: boolean
  created_at: string
}

export const userApi = {
  list: async () => {
    const rawUsers = await request<UserApiResponse[]>('/users')
    const users: User[] = rawUsers.map(u => ({
      id: u.id,
      username: u.username,
      role: u.is_admin ? 'admin' : 'user',
      created_at: u.created_at,
    }))
    return { success: true, users }
  },

  create: (username: string, password: string, role: 'admin' | 'user' = 'user') =>
    request<UserApiResponse>('/users', {
      method: 'POST',
      body: JSON.stringify({ username, password, is_admin: role === 'admin' }),
    }),

  update: (id: string, data: { username?: string; role?: 'admin' | 'user' }) =>
    request<{ success: boolean }>(`/users/${id}`, {
      method: 'PUT',
      body: JSON.stringify({ username: data.username, is_admin: data.role === 'admin' }),
    }),

  delete: (id: string) =>
    request<{ success: boolean }>(`/users/${id}`, { method: 'DELETE' }),

  changePassword: (id: string, newPassword: string, currentPassword?: string) =>
    request<{ success: boolean }>(`/users/${id}/password`, {
      method: 'POST',
      body: JSON.stringify({ new_password: newPassword, current_password: currentPassword }),
    }),
}

export { ApiError }
