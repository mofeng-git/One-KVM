type Status = 'connected' | 'connecting' | 'disconnected' | 'error'

interface HidState {
  available: boolean
  initialized: boolean
  online: boolean
  backend: string
  error?: string | null
  errorCode?: string | null
}

interface InputTransport {
  useWebRtc: boolean
  dataChannelReady: boolean
  rtcConnecting: boolean
  rtcConnected: boolean
  wsConnected: boolean
  wsNetworkError: boolean
  wsHidUnavailable: boolean
}

export function getHidStatus(hid: HidState | null, transport: InputTransport): Status {
  if (hid?.errorCode === 'udc_not_configured') return 'disconnected'
  if (hid?.error) return 'error'
  if (!hid?.available) return 'disconnected'

  // A browser DataChannel can stay open after the controlled computer disconnects.
  // Both the HID backend and the browser input transport must be ready.
  if (!hid.online) {
    return hid.initialized && hid.backend !== 'bluetooth' ? 'connecting' : 'disconnected'
  }

  if (transport.useWebRtc) {
    if (transport.dataChannelReady) return 'connected'
    if (transport.rtcConnecting || transport.rtcConnected) return 'connecting'
  }

  if (transport.wsNetworkError) return 'connecting'
  if (!transport.wsConnected || transport.wsHidUnavailable) return 'disconnected'
  return 'connected'
}
