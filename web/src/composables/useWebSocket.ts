
import { ref } from 'vue'
import { buildWsUrl, WS_RECONNECT_DELAY } from '@/types/websocket'

export interface WsEvent {
  event: string
  data: any
}

type EventHandler = (data: any) => void

let wsInstance: WebSocket | null = null
const handlers = new Map<string, EventHandler[]>()
let subscribedTopics: string[] = []
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null
const connected = ref(false)
const reconnectAttempts = ref(0)
const networkError = ref(false)
const networkErrorMessage = ref<string | null>(null)

function getSubscribedTopics(): string[] {
  return Array.from(handlers.entries())
    .filter(([, eventHandlers]) => eventHandlers.length > 0)
    .map(([event]) => event)
    .sort()
}

function arraysEqual(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index])
}

function syncSubscriptions(force = false) {
  const topics = getSubscribedTopics()

  if (!force && arraysEqual(topics, subscribedTopics)) {
    return
  }

  if (wsInstance && wsInstance.readyState === WebSocket.OPEN) {
    subscribe(topics)
    subscribedTopics = topics
  }
}

function connect() {
  if (wsInstance && (wsInstance.readyState === WebSocket.OPEN || wsInstance.readyState === WebSocket.CONNECTING)) {
    syncSubscriptions()
    return
  }

  if (reconnectTimeout !== null) {
    clearTimeout(reconnectTimeout)
    reconnectTimeout = null
  }

  const url = buildWsUrl('/api/ws')

  try {
    const socket = new WebSocket(url)
    wsInstance = socket

    socket.onopen = () => {
      if (wsInstance !== socket) return
      connected.value = true
      networkError.value = false
      networkErrorMessage.value = null
      reconnectAttempts.value = 0

      syncSubscriptions(true)
    }

    socket.onmessage = (e) => {
      if (wsInstance !== socket) return
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

    socket.onclose = () => {
      if (wsInstance !== socket) return
      wsInstance = null
      connected.value = false
      networkError.value = true

      // Auto-reconnect with infinite retry
      reconnectAttempts.value++
      reconnectTimeout = setTimeout(connect, WS_RECONNECT_DELAY)
    }

    socket.onerror = () => {
      if (wsInstance !== socket) return
      networkError.value = true
      networkErrorMessage.value = 'Network connection failed'
    }
  } catch (err) {
    console.error('[WebSocket] Failed to create connection:', err)
  }
}

function disconnect() {
  if (reconnectTimeout !== null) {
    clearTimeout(reconnectTimeout)
    reconnectTimeout = null
  }
  const socket = wsInstance
  wsInstance = null
  socket?.close()
  connected.value = false
  networkError.value = false
  networkErrorMessage.value = null
  reconnectAttempts.value = 0
  subscribedTopics = []
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
  syncSubscriptions()
}

function off(event: string, handler: EventHandler) {
  const eventHandlers = handlers.get(event)
  if (eventHandlers) {
    const index = eventHandlers.indexOf(handler)
    if (index > -1) {
      eventHandlers.splice(index, 1)
    }
    if (eventHandlers.length === 0) {
      handlers.delete(event)
    }
  }
  syncSubscriptions()
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

if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    disconnect()
  })
}
