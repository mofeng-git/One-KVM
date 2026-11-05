/** Build WebSocket URL from current location */
export function buildWsUrl(path: string): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}${path}`
}

/** Default reconnect delay in milliseconds */
export const WS_RECONNECT_DELAY = 3000

export type StreamKind = 'video' | 'audio'

export interface StreamDeviceLostEventData {
  kind: StreamKind
  device: string
  reason: string
}

export type StreamState =
  | 'uninitialized'
  | 'ready'
  | 'streaming'
  | 'no_signal'
  | 'device_lost'
  | 'device_busy'
  | 'error'

export type StreamSignalReason =
  | 'no_signal'
  | 'no_cable'
  | 'no_sync'
  | 'out_of_range'
  | 'recovering'
  | 'device_lost'
  | 'config_changing'
  | 'mode_switching'
  | 'audio_device_lost'
  | 'audio_reconnecting'
  | 'uvc_usb_error'
  | 'uvc_capture_stall'

export interface StreamStateChangedEventData {
  kind?: StreamKind
  state: StreamState
  device?: string | null
  reason?: StreamSignalReason | null
  next_retry_ms?: number | null
}
