// WebSocket HID channel for low-latency keyboard/mouse input (binary protocol)
// Uses the same binary format as WebRTC DataChannel for consistency

import { ref, onUnmounted } from 'vue'

export interface HidKeyboardEvent {
  type: 'keydown' | 'keyup'
  key: number
  modifiers?: {
    ctrl?: boolean
    shift?: boolean
    alt?: boolean
    meta?: boolean
  }
}

export interface HidMouseEvent {
  type: 'move' | 'moveabs' | 'down' | 'up' | 'scroll'
  x?: number
  y?: number
  button?: number // 0=left, 1=middle, 2=right
  scroll?: number
}

// Binary message constants (must match datachannel.rs)
const MSG_KEYBOARD = 0x01
const MSG_MOUSE = 0x02

// Keyboard event types
const KB_EVENT_DOWN = 0x00
const KB_EVENT_UP = 0x01

// Mouse event types
const MS_EVENT_MOVE = 0x00
const MS_EVENT_MOVE_ABS = 0x01
const MS_EVENT_DOWN = 0x02
const MS_EVENT_UP = 0x03
const MS_EVENT_SCROLL = 0x04

// Response codes from server
const RESP_OK = 0x00
const RESP_ERR_HID_UNAVAILABLE = 0x01
const RESP_ERR_INVALID_MESSAGE = 0x02

let wsInstance: WebSocket | null = null
const connected = ref(false)
const reconnectAttempts = ref(0)
const networkError = ref(false)
const networkErrorMessage = ref<string | null>(null)
const RECONNECT_DELAY = 3000
let reconnectTimeout: number | null = null
const hidUnavailable = ref(false) // Track if HID is unavailable to prevent unnecessary reconnects

// Mouse throttle mechanism
let mouseThrottleMs = 10
let lastMouseSendTime = 0
let pendingMouseEvent: HidMouseEvent | null = null
let throttleTimer: number | null = null

// Connection promise to avoid race conditions
let connectionPromise: Promise<boolean> | null = null
let connectionResolved = false

// Encode keyboard event to binary format
function encodeKeyboardEvent(event: HidKeyboardEvent): ArrayBuffer {
  const buffer = new ArrayBuffer(4)
  const view = new DataView(buffer)

  view.setUint8(0, MSG_KEYBOARD)
  view.setUint8(1, event.type === 'keydown' ? KB_EVENT_DOWN : KB_EVENT_UP)
  view.setUint8(2, event.key & 0xff)

  // Build modifiers bitmask
  let modifiers = 0
  if (event.modifiers?.ctrl) modifiers |= 0x01 // Left Ctrl
  if (event.modifiers?.shift) modifiers |= 0x02 // Left Shift
  if (event.modifiers?.alt) modifiers |= 0x04 // Left Alt
  if (event.modifiers?.meta) modifiers |= 0x08 // Left Meta
  view.setUint8(3, modifiers)

  return buffer
}

// Encode mouse event to binary format
function encodeMouseEvent(event: HidMouseEvent): ArrayBuffer {
  const buffer = new ArrayBuffer(7)
  const view = new DataView(buffer)

  view.setUint8(0, MSG_MOUSE)

  // Event type
  let eventType = MS_EVENT_MOVE
  switch (event.type) {
    case 'move':
      eventType = MS_EVENT_MOVE
      break
    case 'moveabs':
      eventType = MS_EVENT_MOVE_ABS
      break
    case 'down':
      eventType = MS_EVENT_DOWN
      break
    case 'up':
      eventType = MS_EVENT_UP
      break
    case 'scroll':
      eventType = MS_EVENT_SCROLL
      break
  }
  view.setUint8(1, eventType)

  // X coordinate (i16 LE)
  view.setInt16(2, event.x ?? 0, true)

  // Y coordinate (i16 LE)
  view.setInt16(4, event.y ?? 0, true)

  // Button or scroll delta
  if (event.type === 'down' || event.type === 'up') {
    view.setUint8(6, event.button ?? 0)
  } else if (event.type === 'scroll') {
    view.setInt8(6, event.scroll ?? 0)
  } else {
    view.setUint8(6, 0)
  }

  return buffer
}

function connect(): Promise<boolean> {
  // If already connected, return immediately
  if (wsInstance && wsInstance.readyState === WebSocket.OPEN && connectionResolved) {
    return Promise.resolve(true)
  }

  // If connection is in progress, return the existing promise
  if (connectionPromise && !connectionResolved) {
    return connectionPromise
  }

  connectionResolved = false
  connectionPromise = new Promise((resolve) => {
    // Reset network error flag when attempting new connection
    networkError.value = false
    networkErrorMessage.value = null
    hidUnavailable.value = false

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const url = `${protocol}//${window.location.host}/api/ws/hid`

    try {
      wsInstance = new WebSocket(url)
      wsInstance.binaryType = 'arraybuffer'

      wsInstance.onopen = () => {
        connected.value = true
        networkError.value = false
        reconnectAttempts.value = 0
      }

      wsInstance.onmessage = (e) => {
        // Handle binary response
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
        reconnectTimeout = window.setTimeout(() => connect(), RECONNECT_DELAY)
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
    // Close the websocket
    wsInstance.close()
    wsInstance = null
    connected.value = false
    networkError.value = false
  }

  // Reset connection state
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

// Set mouse throttle interval (0-1000ms, 0 = no throttle)
export function setMouseThrottle(ms: number) {
  mouseThrottleMs = Math.max(0, Math.min(1000, ms))
}

// Internal function to actually send mouse event
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

// Throttled mouse event sender
function sendMouse(event: HidMouseEvent): Promise<void> {
  return new Promise((resolve, reject) => {
    const now = Date.now()
    const elapsed = now - lastMouseSendTime

    if (elapsed >= mouseThrottleMs) {
      // Send immediately if enough time has passed
      lastMouseSendTime = now
      _sendMouseInternal(event).then(resolve).catch(reject)
    } else {
      // Queue the event and send after throttle period
      pendingMouseEvent = event

      // Clear existing timer
      if (throttleTimer !== null) {
        clearTimeout(throttleTimer)
      }

      // Schedule send after remaining throttle time
      throttleTimer = window.setTimeout(() => {
        if (pendingMouseEvent) {
          lastMouseSendTime = Date.now()
          _sendMouseInternal(pendingMouseEvent)
            .then(resolve)
            .catch(reject)
          pendingMouseEvent = null
        }
      }, mouseThrottleMs - elapsed)
    }
  })
}

export function useHidWebSocket() {
  onUnmounted(() => {
    // Don't disconnect on component unmount - WebSocket is shared
    // Only disconnect when explicitly called or page unloads
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
  }
}

// Global lifecycle - disconnect when page unloads
if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    disconnect()
  })
}
