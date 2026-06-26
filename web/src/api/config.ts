import type {
  AppConfig,
  AuthConfig,
  AuthConfigUpdate,
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
  AtxDevices,
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
  FrpcConfig,
  FrpcConfigUpdate,
  WebConfigResponse,
  WebConfigUpdate,
} from '@/types/generated'

import { request } from './request'

export const configApi = {
  getAll: () => request<AppConfig>('/config'),
}

export const authConfigApi = {
  get: () => request<AuthConfig>('/config/auth'),

  update: (config: AuthConfigUpdate) =>
    request<AuthConfig>('/config/auth', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export const videoConfigApi = {
  get: () => request<VideoConfig>('/config/video'),

  update: (config: VideoConfigUpdate) =>
    request<VideoConfig>('/config/video', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export const streamConfigApi = {
  get: () => request<StreamConfigResponse>('/config/stream'),

  update: (config: StreamConfigUpdate) =>
    request<StreamConfigResponse>('/config/stream', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export const hidConfigApi = {
  get: () => request<HidConfig>('/config/hid'),

  update: (config: HidConfigUpdate) =>
    request<HidConfig>('/config/hid', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export const msdConfigApi = {
  get: () => request<MsdConfig>('/config/msd'),

  update: (config: MsdConfigUpdate) =>
    request<MsdConfig>('/config/msd', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export interface WolHistoryEntry {
  mac_address: string
  updated_at: number
}

export interface WolHistoryResponse {
  history: WolHistoryEntry[]
}

export const atxConfigApi = {
  get: () => request<AtxConfig>('/config/atx'),

  update: (config: AtxConfigUpdate) =>
    request<AtxConfig>('/config/atx', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  listDevices: () => request<AtxDevices>('/devices/atx'),

  sendWol: (macAddress: string) =>
    request<{ success: boolean; message?: string }>('/atx/wol', {
      method: 'POST',
      body: JSON.stringify({ mac_address: macAddress }),
    }),

  getWolHistory: (limit = 5) =>
    request<WolHistoryResponse>(`/atx/wol/history?limit=${Math.max(1, Math.min(50, limit))}`),
}

export const audioConfigApi = {
  get: () => request<AudioConfig>('/config/audio'),

  update: (config: AudioConfigUpdate) =>
    request<AudioConfig>('/config/audio', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export const extensionsApi = {
  getAll: () => request<ExtensionsStatus>('/extensions'),

  get: (id: string) => request<ExtensionInfo>(`/extensions/${id}`),

  start: (id: string) =>
    request<ExtensionInfo>(`/extensions/${id}/start`, {
      method: 'POST',
    }),

  stop: (id: string) =>
    request<ExtensionInfo>(`/extensions/${id}/stop`, {
      method: 'POST',
    }),

  logs: (id: string, lines = 100) =>
    request<ExtensionLogs>(`/extensions/${id}/logs?lines=${lines}`),

  updateTtyd: (config: TtydConfigUpdate) =>
    request<TtydConfig>('/extensions/ttyd/config', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  updateGostc: (config: GostcConfigUpdate) =>
    request<GostcConfig>('/extensions/gostc/config', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  updateEasytier: (config: EasytierConfigUpdate) =>
    request<EasytierConfig>('/extensions/easytier/config', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  updateFrpc: (config: FrpcConfigUpdate) =>
    request<FrpcConfig>('/extensions/frpc/config', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export interface RustDeskConfigResponse {
  enabled: boolean
  codec: 'h264' | 'h265'
  rendezvous_server: string
  relay_server: string | null
  device_id: string
  has_password: boolean
  has_keypair: boolean
  relay_key: string | null
}

export interface RustDeskStatusResponse {
  config: RustDeskConfigResponse
  service_status: string
  rendezvous_status: string | null
}

export interface RustDeskConfigUpdate {
  enabled?: boolean
  codec?: 'h264' | 'h265'
  rendezvous_server?: string
  relay_server?: string
  relay_key?: string
  device_password?: string
}

export interface RustDeskPasswordResponse {
  device_id: string
  device_password: string
}

export const rustdeskConfigApi = {
  get: () => request<RustDeskConfigResponse>('/config/rustdesk'),

  update: (config: RustDeskConfigUpdate) =>
    request<RustDeskConfigResponse>('/config/rustdesk', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  getStatus: () => request<RustDeskStatusResponse>('/config/rustdesk/status'),

  getPassword: () => request<RustDeskPasswordResponse>('/config/rustdesk/password'),

  regenerateId: () =>
    request<RustDeskConfigResponse>('/config/rustdesk/regenerate-id', {
      method: 'POST',
    }),

  regeneratePassword: () =>
    request<RustDeskConfigResponse>('/config/rustdesk/regenerate-password', {
      method: 'POST',
    }),

  start: () => request<RustDeskStatusResponse>('/config/rustdesk/start', { method: 'POST' }),

  stop: () => request<RustDeskStatusResponse>('/config/rustdesk/stop', { method: 'POST' }),
}

export type RtspCodec = 'h264' | 'h265'

export interface RtspConfigResponse {
  enabled: boolean
  bind: string
  port: number
  path: string
  allow_one_client: boolean
  codec: RtspCodec
  username?: string | null
  password: string | null
}

export interface RtspConfigUpdate {
  enabled?: boolean
  bind?: string
  port?: number
  path?: string
  allow_one_client?: boolean
  codec?: RtspCodec
  username?: string
  password?: string
}

export interface RtspStatusResponse {
  config: RtspConfigResponse
  service_status: string
}

export const rtspConfigApi = {
  get: () => request<RtspConfigResponse>('/config/rtsp'),

  update: (config: RtspConfigUpdate) =>
    request<RtspConfigResponse>('/config/rtsp', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  getStatus: () => request<RtspStatusResponse>('/config/rtsp/status'),

  start: () => request<RtspStatusResponse>('/config/rtsp/start', { method: 'POST' }),

  stop: () => request<RtspStatusResponse>('/config/rtsp/stop', { method: 'POST' }),
}

export type VncEncoding = 'tight_jpeg' | 'h264'

export interface VncConfigResponse {
  enabled: boolean
  bind: string
  port: number
  encoding: VncEncoding
  jpeg_quality: number
  allow_one_client: boolean
  has_password: boolean
}

export interface VncConfigUpdate {
  enabled?: boolean
  bind?: string
  port?: number
  encoding?: VncEncoding
  jpeg_quality?: number
  allow_one_client?: boolean
  password?: string
}

export interface VncStatusResponse {
  config: VncConfigResponse
  service_status: string
  connection_count: number
}

export const vncConfigApi = {
  get: () => request<VncConfigResponse>('/config/vnc'),

  update: (config: VncConfigUpdate) =>
    request<VncConfigResponse>('/config/vnc', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  getStatus: () => request<VncStatusResponse>('/config/vnc/status'),

  start: () => request<VncStatusResponse>('/config/vnc/start', { method: 'POST' }),

  stop: () => request<VncStatusResponse>('/config/vnc/stop', { method: 'POST' }),
}

export type WebConfig = WebConfigResponse

export type { WebConfigUpdate }

export const webConfigApi = {
  get: () => request<WebConfigResponse>('/config/web'),

  update: (config: WebConfigUpdate) =>
    request<WebConfigResponse>('/config/web', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export interface RedfishConfigResponse {
  enabled: boolean
}

export interface RedfishConfigUpdate {
  enabled?: boolean
}

export const redfishConfigApi = {
  get: () => request<RedfishConfigResponse>('/config/redfish'),

  update: (config: RedfishConfigUpdate) =>
    request<RedfishConfigResponse>('/config/redfish', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export const systemApi = {
  restart: () =>
    request<{ success: boolean; message?: string }>('/system/restart', {
      method: 'POST',
    }),
}
