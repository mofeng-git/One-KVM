<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { RotateCw } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { buildWsUrl } from '@/types/websocket'
import '@xterm/xterm/css/xterm.css'

type ConnectionState = 'connecting' | 'connected' | 'closed' | 'error'

const { t } = useI18n()

const container = ref<HTMLDivElement | null>(null)
const connectionState = ref<ConnectionState>('connecting')
const statusMessage = ref('')

let terminal: Terminal | null = null
let fitAddon: FitAddon | null = null
let socket: WebSocket | null = null
let resizeObserver: ResizeObserver | null = null
let resizeFrame: number | null = null
let disposed = false

function sendCommand(command: Record<string, unknown>) {
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify(command))
  }
}

function sendResize() {
  const term = terminal
  if (!term) return
  sendCommand({ type: 'resize', cols: term.cols, rows: term.rows })
}

function writeStatusLine(message: string) {
  terminal?.write(`\r\n\x1b[33m${message}\x1b[0m\r\n`)
}

function handleControlFrame(raw: string) {
  let message: { type?: string; code?: number | null; message?: string }
  try {
    message = JSON.parse(raw)
  } catch {
    return
  }

  if (message.type === 'exit') {
    connectionState.value = 'closed'
    statusMessage.value =
      typeof message.code === 'number'
        ? t('extensions.ttyd.exitStatus', { code: message.code })
        : t('extensions.ttyd.disconnected')
    writeStatusLine(statusMessage.value)
  } else if (message.type === 'error') {
    connectionState.value = 'error'
    statusMessage.value = message.message || t('extensions.ttyd.connectionFailed')
    writeStatusLine(statusMessage.value)
  }
}

function handleBinaryFrame(data: ArrayBuffer) {
  terminal?.write(new Uint8Array(data))
}

function connect() {
  const term = terminal
  if (!term || disposed) return

  socket?.close()
  connectionState.value = 'connecting'
  statusMessage.value = ''

  const url = new URL(buildWsUrl('/api/terminal/shell'))
  url.searchParams.set('cols', String(term.cols))
  url.searchParams.set('rows', String(term.rows))

  let ws: WebSocket
  try {
    ws = new WebSocket(url.toString())
  } catch {
    connectionState.value = 'error'
    statusMessage.value = t('extensions.ttyd.connectionFailed')
    return
  }

  ws.binaryType = 'arraybuffer'
  socket = ws

  ws.onopen = () => {
    if (disposed || socket !== ws) {
      ws.close()
      return
    }
    connectionState.value = 'connected'
    statusMessage.value = ''
    sendResize()
    term.focus()
  }

  ws.onmessage = (event) => {
    if (socket !== ws) return
    if (typeof event.data === 'string') {
      handleControlFrame(event.data)
    } else if (event.data instanceof ArrayBuffer) {
      handleBinaryFrame(event.data)
    }
  }

  ws.onerror = () => {
    if (disposed || socket !== ws) return
    connectionState.value = 'error'
    statusMessage.value = t('extensions.ttyd.connectionFailed')
  }

  ws.onclose = () => {
    if (socket !== ws) return
    socket = null
    if (disposed) return
    if (connectionState.value === 'connected' || connectionState.value === 'connecting') {
      connectionState.value = 'closed'
      statusMessage.value = t('extensions.ttyd.disconnected')
      writeStatusLine(statusMessage.value)
    }
  }
}

function reconnect() {
  connect()
}

function fit() {
  if (!fitAddon || !container.value) return
  if (container.value.clientWidth === 0 || container.value.clientHeight === 0) return
  try {
    fitAddon.fit()
  } catch {
    // The container may be mid-layout; the next resize tick will retry.
  }
}

function scheduleFit() {
  if (resizeFrame !== null) cancelAnimationFrame(resizeFrame)
  resizeFrame = requestAnimationFrame(() => {
    resizeFrame = null
    fit()
  })
}

async function copySelection() {
  const selection = terminal?.getSelection()
  if (!selection) return false
  try {
    await navigator.clipboard.writeText(selection)
  } catch {
    // Clipboard access can be blocked outside secure contexts.
  }
  return true
}

onMounted(async () => {
  const host = container.value
  if (!host) return

  const term = new Terminal({
    cursorBlink: true,
    cursorStyle: 'bar',
    fontFamily:
      'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
    fontSize: 13,
    lineHeight: 1.2,
    scrollback: 5000,
    theme: {
      background: '#0b0e14',
      foreground: '#d7dce5',
      cursor: '#7dd3fc',
      cursorAccent: '#0b0e14',
      selectionBackground: '#274060',
      black: '#1b1f27',
      red: '#f87171',
      green: '#86efac',
      yellow: '#fbbf24',
      blue: '#60a5fa',
      magenta: '#c084fc',
      cyan: '#67e8f9',
      white: '#d7dce5',
      brightBlack: '#5b6577',
      brightRed: '#fca5a5',
      brightGreen: '#bbf7d0',
      brightYellow: '#fde68a',
      brightBlue: '#93c5fd',
      brightMagenta: '#d8b4fe',
      brightCyan: '#a5f3fc',
      brightWhite: '#f8fafc',
    },
  })
  terminal = term

  const addon = new FitAddon()
  fitAddon = addon
  term.loadAddon(addon)

  term.onData((data) => sendCommand({ type: 'input', data }))
  term.onResize(() => sendResize())
  term.attachCustomKeyEventHandler((event) => {
    if (
      event.type === 'keydown' &&
      event.ctrlKey &&
      event.shiftKey &&
      event.key.toLowerCase() === 'c'
    ) {
      void copySelection()
      return false
    }
    return true
  })

  term.open(host)

  resizeObserver = new ResizeObserver(scheduleFit)
  resizeObserver.observe(host)

  await nextTick()
  fit()
  connect()
})

onBeforeUnmount(() => {
  disposed = true
  if (resizeFrame !== null) {
    cancelAnimationFrame(resizeFrame)
    resizeFrame = null
  }
  resizeObserver?.disconnect()
  resizeObserver = null
  socket?.close()
  socket = null
  terminal?.dispose()
  terminal = null
  fitAddon = null
})
</script>

<template>
  <div class="relative h-full w-full overflow-hidden bg-[#0b0e14]">
    <div ref="container" class="h-full w-full" />

    <div
      v-if="connectionState === 'connecting'"
      class="pointer-events-none absolute inset-0 flex items-center justify-center bg-[#0b0e14]/80 text-xs text-slate-400"
    >
      {{ t('extensions.ttyd.connecting') }}
    </div>

    <div
      v-else-if="connectionState !== 'connected'"
      class="absolute inset-x-0 bottom-0 flex items-center justify-between gap-3 border-t border-white/10 bg-[#11151d]/95 px-3 py-2 text-xs text-slate-300"
    >
      <span class="min-w-0 truncate">{{ statusMessage }}</span>
      <Button variant="ghost" size="sm" class="h-7 shrink-0 gap-1.5 text-xs text-slate-200 hover:bg-white/10" @click="reconnect">
        <RotateCw class="size-3.5" />
        {{ t('extensions.ttyd.reconnect') }}
      </Button>
    </div>
  </div>
</template>
