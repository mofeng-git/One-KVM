/**
 * 配置管理 API - 域分离架构
 *
 * 每个配置域（video, stream, hid, msd, atx, audio）有独立的 GET/PATCH 端点，
 * 避免配置项之间的相互干扰。
 */

import type {
  AppConfig,
  VideoConfig,
  VideoConfigUpdate,
  StreamConfigResponse,
  StreamConfigUpdate,
  HidConfig,
  HidConfigUpdate,
  MsdConfig,
  MsdConfigUpdate,
  AtxConfig,
  AtxConfigUpdate,
  AudioConfig,
  AudioConfigUpdate,
  ExtensionsStatus,
  ExtensionInfo,
  ExtensionLogs,
  TtydConfig,
  TtydConfigUpdate,
  GostcConfig,
  GostcConfigUpdate,
  EasytierConfig,
  EasytierConfigUpdate,
  TtydStatus,
} from '@/types/generated'

const API_BASE = '/api'

/**
 * 通用请求函数
 */
async function request<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
    credentials: 'include',
  })

  if (!response.ok) {
    const error = await response.json().catch(() => ({ message: 'Request failed' }))
    throw new Error(error.message || `HTTP ${response.status}`)
  }

  return response.json()
}

// ===== 全局配置 API =====
export const configApi = {
  /**
   * 获取完整配置
   */
  getAll: () => request<AppConfig>('/config'),
}

// ===== Video 配置 API =====
export const videoConfigApi = {
  /**
   * 获取视频配置
   */
  get: () => request<VideoConfig>('/config/video'),

  /**
   * 更新视频配置
   * @param config 要更新的字段（仅发送需要修改的字段）
   */
  update: (config: VideoConfigUpdate) =>
    request<VideoConfig>('/config/video', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

// ===== Stream 配置 API =====
export const streamConfigApi = {
  /**
   * 获取流配置
   */
  get: () => request<StreamConfigResponse>('/config/stream'),

  /**
   * 更新流配置
   * @param config 要更新的字段
   */
  update: (config: StreamConfigUpdate) =>
    request<StreamConfigResponse>('/config/stream', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

// ===== HID 配置 API =====
export const hidConfigApi = {
  /**
   * 获取 HID 配置
   */
  get: () => request<HidConfig>('/config/hid'),

  /**
   * 更新 HID 配置
   * @param config 要更新的字段
   */
  update: (config: HidConfigUpdate) =>
    request<HidConfig>('/config/hid', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

// ===== MSD 配置 API =====
export const msdConfigApi = {
  /**
   * 获取 MSD 配置
   */
  get: () => request<MsdConfig>('/config/msd'),

  /**
   * 更新 MSD 配置
   * @param config 要更新的字段
   */
  update: (config: MsdConfigUpdate) =>
    request<MsdConfig>('/config/msd', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

// ===== ATX 配置 API =====
import type { AtxDevices } from '@/types/generated'

export const atxConfigApi = {
  /**
   * 获取 ATX 配置
   */
  get: () => request<AtxConfig>('/config/atx'),

  /**
   * 更新 ATX 配置
   * @param config 要更新的字段
   */
  update: (config: AtxConfigUpdate) =>
    request<AtxConfig>('/config/atx', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  /**
   * 获取可用的 ATX 设备（GPIO chips, USB relays）
   */
  listDevices: () => request<AtxDevices>('/devices/atx'),

  /**
   * 发送 Wake-on-LAN 魔术包
   * @param macAddress 目标 MAC 地址
   */
  sendWol: (macAddress: string) =>
    request<{ success: boolean; message?: string }>('/atx/wol', {
      method: 'POST',
      body: JSON.stringify({ mac_address: macAddress }),
    }),
}

// ===== Audio 配置 API =====
export const audioConfigApi = {
  /**
   * 获取音频配置
   */
  get: () => request<AudioConfig>('/config/audio'),

  /**
   * 更新音频配置
   * @param config 要更新的字段
   */
  update: (config: AudioConfigUpdate) =>
    request<AudioConfig>('/config/audio', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

// ===== Extensions API =====
export const extensionsApi = {
  /**
   * 获取所有扩展状态
   */
  getAll: () => request<ExtensionsStatus>('/extensions'),

  /**
   * 获取单个扩展状态
   */
  get: (id: string) => request<ExtensionInfo>(`/extensions/${id}`),

  /**
   * 启动扩展
   */
  start: (id: string) =>
    request<ExtensionInfo>(`/extensions/${id}/start`, {
      method: 'POST',
    }),

  /**
   * 停止扩展
   */
  stop: (id: string) =>
    request<ExtensionInfo>(`/extensions/${id}/stop`, {
      method: 'POST',
    }),

  /**
   * 获取扩展日志
   */
  logs: (id: string, lines = 100) =>
    request<ExtensionLogs>(`/extensions/${id}/logs?lines=${lines}`),

  /**
   * 获取 ttyd 状态（简化版，用于控制台）
   */
  getTtydStatus: () => request<TtydStatus>('/extensions/ttyd/status'),

  /**
   * 更新 ttyd 配置
   */
  updateTtyd: (config: TtydConfigUpdate) =>
    request<TtydConfig>('/extensions/ttyd/config', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  /**
   * 更新 gostc 配置
   */
  updateGostc: (config: GostcConfigUpdate) =>
    request<GostcConfig>('/extensions/gostc/config', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  /**
   * 更新 easytier 配置
   */
  updateEasytier: (config: EasytierConfigUpdate) =>
    request<EasytierConfig>('/extensions/easytier/config', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

// ===== RustDesk 配置 API =====

/** RustDesk 配置响应 */
export interface RustDeskConfigResponse {
  enabled: boolean
  rendezvous_server: string
  relay_server: string | null
  device_id: string
  has_password: boolean
  has_keypair: boolean
}

/** RustDesk 状态响应 */
export interface RustDeskStatusResponse {
  config: RustDeskConfigResponse
  service_status: string
  rendezvous_status: string | null
}

/** RustDesk 配置更新 */
export interface RustDeskConfigUpdate {
  enabled?: boolean
  rendezvous_server?: string
  relay_server?: string
  device_password?: string
}

/** RustDesk 密码响应 */
export interface RustDeskPasswordResponse {
  device_id: string
  device_password: string
}

export const rustdeskConfigApi = {
  /**
   * 获取 RustDesk 配置
   */
  get: () => request<RustDeskConfigResponse>('/config/rustdesk'),

  /**
   * 更新 RustDesk 配置
   */
  update: (config: RustDeskConfigUpdate) =>
    request<RustDeskConfigResponse>('/config/rustdesk', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  /**
   * 获取 RustDesk 完整状态
   */
  getStatus: () => request<RustDeskStatusResponse>('/config/rustdesk/status'),

  /**
   * 获取设备密码（管理员专用）
   */
  getPassword: () => request<RustDeskPasswordResponse>('/config/rustdesk/password'),

  /**
   * 重新生成设备 ID
   */
  regenerateId: () =>
    request<RustDeskConfigResponse>('/config/rustdesk/regenerate-id', {
      method: 'POST',
    }),

  /**
   * 重新生成设备密码
   */
  regeneratePassword: () =>
    request<RustDeskConfigResponse>('/config/rustdesk/regenerate-password', {
      method: 'POST',
    }),
}
