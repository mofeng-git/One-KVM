// WebSocket composable for real-time event streaming
//
// Usage:
//   const { connected, on, off } = useWebSocket()
//   on('stream.state_changed', (data) => { ... })

import { ref } from 'vue'
import { buildWsUrl, WS_RECONNECT_DELAY } from '@/types/websocket'

export interface WsEvent {
  event: string
  data: any
}

type EventHandler = (data: any) => void

let wsInstance: WebSocket | null = null
let handlers = new Map<string, EventHandler[]>()
const connected = ref(false)
const reconnectAttempts = ref(0)
const networkError = ref(false)
const networkErrorMessage = ref<string | null>(null)

function connect() {
  if (wsInstance && wsInstance.readyState === WebSocket.OPEN) {
    return
  }

  const url = buildWsUrl('/api/ws')

  try {
    wsInstance = new WebSocket(url)

    wsInstance.onopen = () => {
      connected.value = true
      networkError.value = false
      networkErrorMessage.value = null
      reconnectAttempts.value = 0

      // Subscribe to all events by default
      subscribe(['*'])
    }

    wsInstance.onmessage = (e) => {
      try {
        const event: WsEvent = JSON.parse(e.data)

        if (event.event === 'error') {
          console.error('[WebSocket] Server error:', event.data?.message)
        } else {
          handleEvent(event)
        }
      } catch (err) {
        console.error('[WebSocket] Failed to parse message:', err)
      }
    }

    wsInstance.onclose = () => {
      connected.value = false
      networkError.value = true

      // Auto-reconnect with infinite retry
      reconnectAttempts.value++
      setTimeout(connect, WS_RECONNECT_DELAY)
    }

    wsInstance.onerror = () => {
      networkError.value = true
      networkErrorMessage.value = 'Network connection failed'
    }
  } catch (err) {
    console.error('[WebSocket] Failed to create connection:', err)
  }
}

function disconnect() {
  if (wsInstance) {
    wsInstance.close()
    wsInstance = null
  }
}

function subscribe(topics: string[]) {
  if (wsInstance && wsInstance.readyState === WebSocket.OPEN) {
    wsInstance.send(JSON.stringify({
      type: 'subscribe',
      payload: { topics }
    }))
  }
}

function on(event: string, handler: EventHandler) {
  if (!handlers.has(event)) {
    handlers.set(event, [])
  }
  handlers.get(event)!.push(handler)
}

function off(event: string, handler: EventHandler) {
  const eventHandlers = handlers.get(event)
  if (eventHandlers) {
    const index = eventHandlers.indexOf(handler)
    if (index > -1) {
      eventHandlers.splice(index, 1)
    }
  }
}

function handleEvent(payload: WsEvent) {
  const eventName = payload.event
  const eventHandlers = handlers.get(eventName)

  if (eventHandlers) {
    eventHandlers.forEach(handler => {
      try {
        handler(payload.data)
      } catch (err) {
        console.error(`[WebSocket] Error in handler for ${eventName}:`, err)
      }
    })
  }
  // Silently ignore events without handlers
}

export function useWebSocket() {
  // Connection is now triggered manually by components after registering handlers

  return {
    connected,
    reconnectAttempts,
    networkError,
    networkErrorMessage,
    on,
    off,
    subscribe,
    connect,
    disconnect,
  }
}

// Global lifecycle - disconnect when page unloads
if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    disconnect()
  })
}
