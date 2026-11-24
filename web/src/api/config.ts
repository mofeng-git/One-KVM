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
  NetworkInterfaceInfo,
  OtgNetworkConfig,
  OtgNetworkConfigUpdate,
  OtgNetworkStatus,
  OtgConfigUpdate,
  OtgConfigResponse,
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
  WebConfigResponse,
  WebConfigUpdate,
  WatchdogConfigResponse,
  WatchdogConfigUpdate,
} from '@/types/generated'

import { request } from './request'

export const configApi = {
  getAll: () => request<AppConfig>('/config'),
}

export const watchdogConfigApi = {
  get: () => request<WatchdogConfigResponse>('/config/watchdog', {}, { toastOnError: false }),

  update: (config: WatchdogConfigUpdate) =>
    request<WatchdogConfigResponse>('/config/watchdog', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }, { toastOnError: false }),
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

  update: (config: HidConfigUpdate, signal?: AbortSignal) =>
    request<HidConfig>('/config/hid', {
      signal,
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

export const otgNetworkApi = {
  get: () => request<OtgNetworkConfig>('/config/otg-network'),

  update: (config: OtgNetworkConfigUpdate) =>
    request<OtgNetworkConfig>('/config/otg-network', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),

  status: () => request<OtgNetworkStatus>('/otg/network/status'),

  interfaces: () => request<NetworkInterfaceInfo[]>('/devices/network', {}, { toastOnError: false }),
}

export const uacApi = {
  get: () => request<{enabled: boolean; sample_rate: number; channels: number}>('/config/uac'),

  update: (config: {enabled: boolean; sample_rate: number; channels: number}) =>
    request('/config/uac', {
      method: 'PATCH',
      body: JSON.stringify(config),
    }),
}

export const otgConfigApi = {
  update: (config: OtgConfigUpdate) =>
    request<OtgConfigResponse>('/config/otg', {
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
