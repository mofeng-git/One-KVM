import { ref, onUnmounted } from 'vue'
import { buildWsUrl } from '@/types/websocket'
import type { ComputerUseScreenshot, ComputerUseSession, ComputerUseAction } from '@/api'

export type ComputerUseServerMessage =
  | { type: 'session_updated'; session: ComputerUseSession }
  | { type: 'screenshot_requested'; request_id: string }
  | { type: 'screenshot_captured'; screenshot: ComputerUseScreenshot }
  | { type: 'step_started'; step: number }
  | { type: 'actions_executed'; actions: ComputerUseAction[] }
  | { type: 'error'; message: string }

export function useComputerUseSocket(options: {
  onMessage: (message: ComputerUseServerMessage) => void
  onScreenshotRequested: (requestId: string) => Promise<ComputerUseScreenshot | null>
}) {
  const connected = ref(false)
  const error = ref<string | null>(null)
  const clientId = crypto.randomUUID()
  let ws: WebSocket | null = null
  let connectPromise: Promise<void> | null = null

  function connect(): Promise<void> {
    if (ws && ws.readyState === WebSocket.OPEN) return Promise.resolve()
    if (connectPromise) return connectPromise

    ws = new WebSocket(buildWsUrl(`/api/ws/computer-use?client_id=${encodeURIComponent(clientId)}`))

    connectPromise = new Promise((resolve, reject) => {
      if (!ws) {
        reject(new Error('Computer use WebSocket failed'))
        return
      }

      ws.onopen = () => {
        connected.value = true
        error.value = null
        connectPromise = null
        resolve()
      }

      ws.onerror = () => {
        error.value = 'Computer use WebSocket failed'
        connectPromise = null
        reject(new Error(error.value))
      }
    })

    ws.onclose = () => {
      connected.value = false
      connectPromise = null
    }

    ws.onmessage = async (event) => {
      try {
        const message = JSON.parse(event.data) as ComputerUseServerMessage
        options.onMessage(message)
        if (message.type === 'screenshot_requested') {
          const screenshot = await options.onScreenshotRequested(message.request_id)
          if (screenshot && ws?.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({
              type: 'screenshot_result',
              request_id: message.request_id,
              screenshot,
            }))
          }
        }
      } catch (err) {
        console.error('[ComputerUse] Failed to handle WS message:', err)
      }
    }

    return connectPromise
  }

  function disconnect() {
    ws?.close()
    ws = null
    connected.value = false
    connectPromise = null
  }

  onUnmounted(disconnect)

  return {
    connected,
    error,
    clientId,
    connect,
    disconnect,
  }
}
