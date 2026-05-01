import { ref, onUnmounted } from 'vue'
import {
  type HidKeyboardEvent,
  type HidMouseEvent,
  type HidConsumerEvent,
  encodeKeyboardEvent,
  encodeMouseEvent,
  encodeConsumerEvent,
  RESP_OK,
  RESP_ERR_HID_UNAVAILABLE,
  RESP_ERR_INVALID_MESSAGE,
} from '@/types/hid'
import { buildWsUrl, WS_RECONNECT_DELAY } from '@/types/websocket'

export type { HidKeyboardEvent, HidMouseEvent, HidConsumerEvent }

let wsInstance: WebSocket | null = null
const connected = ref(false)
const reconnectAttempts = ref(0)
const networkError = ref(false)
const networkErrorMessage = ref<string | null>(null)
let reconnectTimeout: number | null = null
const hidUnavailable = ref(false)

let connectionPromise: Promise<boolean> | null = null
let connectionResolved = false

function connect(): Promise<boolean> {
  if (wsInstance && wsInstance.readyState === WebSocket.OPEN && connectionResolved) {
    return Promise.resolve(true)
  }

  if (connectionPromise && !connectionResolved) {
    return connectionPromise
  }

  connectionResolved = false
  connectionPromise = new Promise((resolve) => {
    networkError.value = false
    networkErrorMessage.value = null
    hidUnavailable.value = false

    const url = buildWsUrl('/api/ws/hid')

    try {
      wsInstance = new WebSocket(url)
      wsInstance.binaryType = 'arraybuffer'

      wsInstance.onopen = () => {
        connected.value = true
        networkError.value = false
        reconnectAttempts.value = 0
      }

      wsInstance.onmessage = (e) => {
        if (e.data instanceof ArrayBuffer) {
          const view = new DataView(e.data)
          if (view.byteLength >= 1) {
            const code = view.getUint8(0)
            if (code === RESP_OK) {
              hidUnavailable.value = false
              networkError.value = false
              connectionResolved = true
              resolve(true)
            } else if (code === RESP_ERR_HID_UNAVAILABLE) {
              // HID is not available, mark it and don't trigger reconnection
              hidUnavailable.value = true
              networkError.value = false
              connectionResolved = true
              resolve(true)
            } else if (code === RESP_ERR_INVALID_MESSAGE) {
              console.warn('[HID] Server rejected message as invalid')
            }
          }
        }
      }

      wsInstance.onclose = () => {
        connected.value = false
        connectionResolved = false
        connectionPromise = null

        // Don't auto-reconnect if HID is unavailable
        if (hidUnavailable.value) {
          resolve(false)
          return
        }

        // Auto-reconnect with infinite retry for network errors
        networkError.value = true
        networkErrorMessage.value = 'HID WebSocket disconnected'
        reconnectAttempts.value++
        reconnectTimeout = window.setTimeout(() => connect(), WS_RECONNECT_DELAY)
      }

      wsInstance.onerror = () => {
        networkError.value = true
        networkErrorMessage.value = 'Network connection failed'
        connectionResolved = false
        connectionPromise = null
        resolve(false)
      }
    } catch (err) {
      console.error('[HID] Failed to create connection:', err)
      connectionResolved = false
      connectionPromise = null
      resolve(false)
    }
  })

  return connectionPromise
}

function disconnect() {
  if (reconnectTimeout !== null) {
    clearTimeout(reconnectTimeout)
    reconnectTimeout = null
  }

  if (wsInstance) {
    wsInstance.close()
    wsInstance = null
    connected.value = false
    networkError.value = false
  }

  connectionPromise = null
  connectionResolved = false
}

function sendKeyboard(event: HidKeyboardEvent): Promise<void> {
  return new Promise((resolve, reject) => {
    if (!wsInstance || wsInstance.readyState !== WebSocket.OPEN) {
      reject(new Error('WebSocket not connected'))
      return
    }

    try {
      wsInstance.send(encodeKeyboardEvent(event))
      resolve()
    } catch (err) {
      reject(err)
    }
  })
}

function _sendMouseInternal(event: HidMouseEvent): Promise<void> {
  return new Promise((resolve, reject) => {
    if (!wsInstance || wsInstance.readyState !== WebSocket.OPEN) {
      reject(new Error('WebSocket not connected'))
      return
    }

    try {
      wsInstance.send(encodeMouseEvent(event))
      resolve()
    } catch (err) {
      reject(err)
    }
  })
}

function sendMouse(event: HidMouseEvent): Promise<void> {
  return _sendMouseInternal(event)
}

function sendConsumer(event: HidConsumerEvent): Promise<void> {
  return new Promise((resolve, reject) => {
    if (!wsInstance || wsInstance.readyState !== WebSocket.OPEN) {
      reject(new Error('WebSocket not connected'))
      return
    }

    try {
      wsInstance.send(encodeConsumerEvent(event))
      resolve()
    } catch (err) {
      reject(err)
    }
  })
}

export function useHidWebSocket() {
  onUnmounted(() => {
  })

  return {
    connected,
    reconnectAttempts,
    networkError,
    networkErrorMessage,
    hidUnavailable,
    connect,
    disconnect,
    sendKeyboard,
    sendMouse,
    sendConsumer,
  }
}

if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    disconnect()
  })
}
