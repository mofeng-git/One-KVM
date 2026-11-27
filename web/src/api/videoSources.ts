import { request } from './request'

export interface VideoSource {
  id: string
  label: string
  device: string
  format: string
  quality: number
  enabled: boolean
  primary: boolean
  width: number
  height: number
  fps: number
}

export interface VideoSourceStatus {
  state: string
  reason?: string
  /** Actual capture frames, not successful snapshot requests. */
  frames: number
  connected?: boolean | null
}

export const videoSourcesApi = {
  list: (signal?: AbortSignal) => request<{ sources: VideoSource[]; primary_source_id: string }>(
    '/video/sources', { signal }, { toastOnError: false },
  ),
  status: (id: string, signal?: AbortSignal, clientId?: string) => request<VideoSourceStatus>(
    `/video/sources/${encodeURIComponent(id)}/status${clientId ? `?client_id=${encodeURIComponent(clientId)}` : ''}`, { signal }, { toastOnError: false },
  ),
  streamUrl: (id: string, generation: number, clientId: string) =>
    `/api/video/sources/${encodeURIComponent(id)}/stream?generation=${generation}&client_id=${encodeURIComponent(clientId)}`,
  snapshotUrl: (id: string) => `/api/video/sources/${encodeURIComponent(id)}/snapshot`,
}
