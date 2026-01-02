// Shared WebSocket types and utilities
// Used by useWebSocket, useHidWebSocket, and useAudioPlayer

import { ref, type Ref } from 'vue'

/** WebSocket connection state */
export interface WsConnectionState {
  connected: Ref<boolean>
  reconnectAttempts: Ref<number>
  networkError: Ref<boolean>
  networkErrorMessage: Ref<string | null>
}

/** Create a new WebSocket connection state */
export function createWsConnectionState(): WsConnectionState {
  return {
    connected: ref(false),
    reconnectAttempts: ref(0),
    networkError: ref(false),
    networkErrorMessage: ref(null),
  }
}

/** Reset connection state to initial values */
export function resetWsConnectionState(state: WsConnectionState) {
  state.connected.value = false
  state.reconnectAttempts.value = 0
  state.networkError.value = false
  state.networkErrorMessage.value = null
}

/** Build WebSocket URL from current location */
export function buildWsUrl(path: string): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}${path}`
}

/** Default reconnect delay in milliseconds */
export const WS_RECONNECT_DELAY = 3000

/** WebSocket ready states */
export const WS_STATE = {
  CONNECTING: WebSocket.CONNECTING,
  OPEN: WebSocket.OPEN,
  CLOSING: WebSocket.CLOSING,
  CLOSED: WebSocket.CLOSED,
} as const
