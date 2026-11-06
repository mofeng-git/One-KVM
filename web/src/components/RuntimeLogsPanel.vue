<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Loader2, RefreshCw, ScrollText, Send, Trash2 } from 'lucide-vue-next'
import { request } from '@/api/request'
import { buildWsUrl, WS_RECONNECT_DELAY } from '@/types/websocket'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import CardTitle from './SettingsCardTitle.vue'

const MAX_LINES = 1000
const MAX_DESCRIPTION_LENGTH = 2000

const { t } = useI18n()

const lines = ref<string[]>([])
const connected = ref(false)
const autoScroll = ref(true)
const reportOpen = ref(false)
const reportDescription = ref('')
const reportSending = ref(false)
const logViewport = ref<HTMLElement | null>(null)

let socket: WebSocket | null = null
let reconnectTimer: number | null = null
let flushHandle: number | null = null
let disposed = false
const pendingLines: string[] = []

const reportDisabled = computed(
  () => reportSending.value || reportDescription.value.trim().length === 0,
)

function clearLogs() {
  lines.value = []
  pendingLines.length = 0
}

function lineClass(line: string): string {
  if (line.includes(' ERROR ')) return 'text-destructive'
  if (line.includes(' WARN ')) return 'text-amber-600 dark:text-amber-400'
  if (line.includes(' DEBUG ') || line.includes(' TRACE ')) return 'text-muted-foreground'
  return ''
}

function scrollToBottom() {
  if (!autoScroll.value) return
  void nextTick(() => {
    const element = logViewport.value
    if (element) element.scrollTop = element.scrollHeight
  })
}

function flushPendingLines() {
  flushHandle = null
  if (pendingLines.length === 0) return
  const merged = [...lines.value, ...pendingLines.splice(0)]
  lines.value = merged.length > MAX_LINES ? merged.slice(-MAX_LINES) : merged
  scrollToBottom()
}

function enqueueLine(line: string) {
  pendingLines.push(line)
  if (flushHandle !== null) return
  flushHandle = window.requestAnimationFrame(flushPendingLines)
}

function applySnapshot(received: unknown) {
  if (!Array.isArray(received)) return
  pendingLines.length = 0
  lines.value = received
    .filter((line): line is string => typeof line === 'string')
    .slice(-MAX_LINES)
  scrollToBottom()
}

function handleMessage(raw: string) {
  let payload: unknown
  try {
    payload = JSON.parse(raw)
  } catch {
    return
  }
  if (!payload || typeof payload !== 'object') return

  const message = payload as {
    type?: string
    lines?: unknown
    line?: unknown
    count?: unknown
  }

  if (message.type === 'snapshot') {
    applySnapshot(message.lines)
  } else if (message.type === 'line' && typeof message.line === 'string') {
    enqueueLine(message.line)
  } else if (message.type === 'lagged' && typeof message.count === 'number') {
    enqueueLine(t('runtimeLogs.lagged', { count: message.count }))
  }
}

function scheduleReconnect() {
  if (disposed || reconnectTimer !== null) return
  reconnectTimer = window.setTimeout(() => {
    reconnectTimer = null
    connect()
  }, WS_RECONNECT_DELAY)
}

function connect() {
  if (disposed) return
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return
  }

  if (reconnectTimer !== null) {
    window.clearTimeout(reconnectTimer)
    reconnectTimer = null
  }

  try {
    socket = new WebSocket(buildWsUrl('/api/ws/logs'))
  } catch {
    connected.value = false
    scheduleReconnect()
    return
  }

  socket.onopen = () => {
    connected.value = true
  }
  socket.onmessage = (event) => {
    if (typeof event.data === 'string') handleMessage(event.data)
  }
  socket.onerror = () => {
    connected.value = false
  }
  socket.onclose = () => {
    connected.value = false
    scheduleReconnect()
  }
}

function reconnect() {
  socket?.close()
  socket = null
  connect()
}

function openReport() {
  reportDescription.value = ''
  reportOpen.value = true
}

async function submitReport() {
  const description = reportDescription.value.trim()
  if (!description) return

  reportSending.value = true
  try {
    await request<{ success: boolean; status: number }>(
      '/logs/report',
      {
        method: 'POST',
        body: JSON.stringify({ description }),
      },
      { toastOnError: false },
    )
    toast.success(t('runtimeLogs.reportSuccess'), {
      description: t('runtimeLogs.reportSuccessDesc'),
    })
    reportOpen.value = false
    reportDescription.value = ''
  } catch (error) {
    const message = error instanceof Error ? error.message : t('runtimeLogs.reportFailed')
    toast.error(t('runtimeLogs.reportFailed'), { description: message })
  } finally {
    reportSending.value = false
  }
}

watch(autoScroll, (enabled) => {
  if (enabled) scrollToBottom()
})

onMounted(connect)

onBeforeUnmount(() => {
  disposed = true
  if (reconnectTimer !== null) window.clearTimeout(reconnectTimer)
  if (flushHandle !== null) window.cancelAnimationFrame(flushHandle)
  socket?.close()
  socket = null
})
</script>

<template>
  <Card>
    <CardHeader class="settings-card-heading flex flex-row flex-wrap items-center justify-between gap-3">
      <CardTitle :help="t('runtimeLogs.description')">{{ t('runtimeLogs.title') }}</CardTitle>
      <div class="settings-card-actions flex flex-wrap items-center gap-2">
        <Badge :variant="connected ? 'secondary' : 'outline'" class="gap-1">
          <span
            class="size-1.5 rounded-full"
            :class="connected ? 'bg-emerald-500' : 'bg-muted-foreground'"
          />
          {{ connected ? t('runtimeLogs.connected') : t('runtimeLogs.disconnected') }}
        </Badge>
        <Button variant="outline" size="sm" @click="clearLogs">
          <Trash2 class="size-3.5" />
          {{ t('runtimeLogs.clear') }}
        </Button>
        <Button variant="outline" size="sm" @click="reconnect">
          <RefreshCw class="size-3.5" />
          {{ t('runtimeLogs.reconnect') }}
        </Button>
        <Button size="sm" @click="openReport">
          <Send class="size-3.5" />
          {{ t('runtimeLogs.report') }}
        </Button>
      </div>
    </CardHeader>
    <CardContent class="space-y-3">
      <div class="flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
        <span class="flex items-center gap-1.5">
          <ScrollText class="size-3.5" />
          {{ t('runtimeLogs.lineCount', { count: lines.length }) }} ·
          {{ t('runtimeLogs.truncatedHint') }}
        </span>
        <label class="flex items-center gap-2">
          <Switch v-model="autoScroll" />
          {{ t('runtimeLogs.autoScroll') }}
        </label>
      </div>

      <div
        ref="logViewport"
        class="h-[55vh] max-h-[560px] min-h-[240px] overflow-auto rounded-md border bg-muted/40 p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all"
        role="log"
        aria-live="polite"
      >
        <p v-if="lines.length === 0" class="text-muted-foreground">
          {{ connected ? t('runtimeLogs.empty') : t('runtimeLogs.disconnectedHint') }}
        </p>
        <div v-for="(line, index) in lines" :key="index" :class="lineClass(line)">
          {{ line }}
        </div>
      </div>

      <p class="text-xs text-muted-foreground">{{ t('runtimeLogs.reportHint') }}</p>
    </CardContent>
  </Card>

  <Dialog v-model:open="reportOpen">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>{{ t('runtimeLogs.reportTitle') }}</DialogTitle>
      </DialogHeader>
      <div class="space-y-2">
        <Label for="runtime-log-report-description">
          {{ t('runtimeLogs.reportDescription') }}
        </Label>
        <Textarea
          id="runtime-log-report-description"
          v-model="reportDescription"
          :maxlength="MAX_DESCRIPTION_LENGTH"
          :placeholder="t('runtimeLogs.reportPlaceholder')"
          rows="5"
        />
        <p class="text-xs text-muted-foreground">
          {{ t('runtimeLogs.reportUploadHint') }}
        </p>
      </div>
      <DialogFooter>
        <Button variant="outline" :disabled="reportSending" @click="reportOpen = false">
          {{ t('runtimeLogs.reportCancel') }}
        </Button>
        <Button :disabled="reportDisabled" @click="submitReport">
          <Loader2 v-if="reportSending" class="size-4 animate-spin" />
          {{ reportSending ? t('runtimeLogs.reportSending') : t('runtimeLogs.reportSubmit') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
