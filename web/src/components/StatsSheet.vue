<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, nextTick, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { WebRTCStats } from '@/composables/useWebRTC'

const { t } = useI18n()

const props = defineProps<{
  open: boolean
  videoMode: 'mjpeg' | 'h264' | 'h265' | 'vp8' | 'vp9'
  // MJPEG stats
  mjpegFps?: number
  wsLatency?: number
  // WebRTC stats
  webrtcStats?: WebRTCStats
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

// Chart containers
const stabilityChartRef = ref<HTMLDivElement | null>(null)
const delayChartRef = ref<HTMLDivElement | null>(null)
const packetLossChartRef = ref<HTMLDivElement | null>(null)
const fpsChartRef = ref<HTMLDivElement | null>(null)

// Chart instances
let stabilityChart: uPlot | null = null
let delayChart: uPlot | null = null
let packetLossChart: uPlot | null = null
let fpsChart: uPlot | null = null

// Data history (last 120 seconds)
const MAX_POINTS = 120
const timestamps = ref<number[]>([])
const jitterHistory = ref<number[]>([])
const delayHistory = ref<number[]>([])
const packetLossHistory = ref<number[]>([])
const fpsHistory = ref<number[]>([])
const bitrateHistory = ref<number[]>([])

// For delta calculations
let lastBytesReceived = 0
let lastPacketsLost = 0
let lastTimestamp = 0

// Is WebRTC mode
const isWebRTC = computed(() => props.videoMode !== 'mjpeg')

// Format time for axis
function formatTime(ts: number): string {
  const date = new Date(ts * 1000)
  return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}

// Chart theme colors
const chartColors = {
  line: '#3b82f6',
  fill: 'rgba(59, 130, 246, 0.1)',
  grid: 'rgba(148, 163, 184, 0.1)',
  axis: '#64748b',
  text: '#94a3b8',
}

// Chart options factory
function createChartOptions(
  container: HTMLElement,
  _yLabel: string,
  yFormatter: (v: number) => string
): uPlot.Options {
  const width = container.clientWidth || 300

  return {
    width,
    height: 100,
    cursor: {
      show: true,
      x: true,
      y: false,
      drag: { x: false, y: false },
    },
    legend: { show: false },
    scales: {
      x: { time: false },
      y: { auto: true, range: (_u, min, max) => [Math.max(0, min - 1), max + 1] },
    },
    axes: [
      {
        show: true,
        stroke: chartColors.axis,
        grid: { show: false },
        ticks: { show: false },
        gap: 4,
        size: 20,
        values: (_, splits) => splits.map(v => formatTime(v)),
        font: '10px system-ui',
      },
      {
        show: true,
        side: 1, // Right side
        stroke: chartColors.axis,
        size: 55,
        gap: 8,
        grid: { stroke: chartColors.grid, width: 1 },
        values: (_, splits) => splits.map(v => yFormatter(v)),
        font: '10px system-ui',
      },
    ],
    series: [
      {},
      {
        stroke: chartColors.line,
        width: 1.5,
        fill: chartColors.fill,
        paths: uPlot.paths.spline?.() || undefined,
      },
    ],
  }
}

// Tooltip state for each chart
const activeTooltip = ref<{
  chartId: string
  time: string
  value: string
  unit: string
  left: number
  top: number
  visible: boolean
}>({
  chartId: '',
  time: '',
  value: '',
  unit: '',
  left: 0,
  top: 0,
  visible: false,
})

function createTooltipPlugin(chartId: string, unit: string): uPlot.Plugin {
  return {
    hooks: {
      setCursor: [
        (u) => {
          const idx = u.cursor.idx
          if (idx !== null && idx !== undefined && u.cursor.left !== undefined && u.cursor.top !== undefined) {
            const ts = u.data[0]?.[idx]
            const val = u.data[1]?.[idx]
            if (ts !== undefined && ts !== null && val !== undefined && val !== null) {
              const date = new Date(ts * 1000)
              activeTooltip.value = {
                chartId,
                time: date.toLocaleTimeString('zh-CN'),
                value: val.toFixed(1),
                unit,
                left: u.cursor.left,
                top: u.cursor.top,
                visible: true,
              }
            }
          }
        },
      ],
      ready: [
        (u) => {
          const over = u.over
          over.addEventListener('mouseleave', () => {
            if (activeTooltip.value.chartId === chartId) {
              activeTooltip.value.visible = false
            }
          })
        },
      ],
    },
  }
}

// Initialize charts
function initCharts() {
  if (!props.open) return

  nextTick(() => {
    // Initialize timestamps if empty
    if (timestamps.value.length === 0) {
      const now = Date.now() / 1000
      for (let i = MAX_POINTS - 1; i >= 0; i--) {
        timestamps.value.push(now - i)
      }
      jitterHistory.value = new Array(MAX_POINTS).fill(0)
      delayHistory.value = new Array(MAX_POINTS).fill(0)
      packetLossHistory.value = new Array(MAX_POINTS).fill(0)
      fpsHistory.value = new Array(MAX_POINTS).fill(0)
      bitrateHistory.value = new Array(MAX_POINTS).fill(0)
    }

    // Network Stability (Jitter) Chart
    if (stabilityChartRef.value && !stabilityChart) {
      const opts = createChartOptions(stabilityChartRef.value, 'ms', (v) => `${v.toFixed(0)} ms`)
      opts.plugins = [createTooltipPlugin('stability', 'ms')]
      stabilityChart = new uPlot(
        opts,
        [timestamps.value, jitterHistory.value],
        stabilityChartRef.value
      )
    }

    // Playback Delay Chart
    if (delayChartRef.value && !delayChart) {
      const opts = createChartOptions(delayChartRef.value, 'ms', (v) => `${v.toFixed(0)} ms`)
      opts.plugins = [createTooltipPlugin('delay', 'ms')]
      delayChart = new uPlot(
        opts,
        [timestamps.value, delayHistory.value],
        delayChartRef.value
      )
    }

    // Packet Loss Chart
    if (packetLossChartRef.value && !packetLossChart) {
      const opts = createChartOptions(packetLossChartRef.value, '', (v) => `${v.toFixed(0)} 个`)
      opts.plugins = [createTooltipPlugin('packetLoss', '个')]
      packetLossChart = new uPlot(
        opts,
        [timestamps.value, packetLossHistory.value],
        packetLossChartRef.value
      )
    }

    // FPS Chart
    if (fpsChartRef.value && !fpsChart) {
      const opts = createChartOptions(fpsChartRef.value, 'fps', (v) => `${v.toFixed(0)} fps`)
      opts.plugins = [createTooltipPlugin('fps', 'fps')]
      fpsChart = new uPlot(
        opts,
        [timestamps.value, fpsHistory.value],
        fpsChartRef.value
      )
    }
  })
}

// Destroy charts
function destroyCharts() {
  stabilityChart?.destroy()
  stabilityChart = null
  delayChart?.destroy()
  delayChart = null
  packetLossChart?.destroy()
  packetLossChart = null
  fpsChart?.destroy()
  fpsChart = null
}

// Add data point
function addDataPoint() {
  const now = Date.now() / 1000

  // Shift timestamps
  timestamps.value.push(now)
  if (timestamps.value.length > MAX_POINTS) {
    timestamps.value.shift()
  }

  if (isWebRTC.value && props.webrtcStats) {
    // Jitter in ms
    const jitter = (props.webrtcStats.jitter || 0) * 1000
    jitterHistory.value.push(jitter)

    // RTT (round trip time) as delay in ms
    const rtt = (props.webrtcStats.roundTripTime || 0) * 1000
    delayHistory.value.push(rtt)

    // Packet loss delta
    const currentLost = props.webrtcStats.packetsLost || 0
    const lostDelta = lastPacketsLost > 0 ? Math.max(0, currentLost - lastPacketsLost) : 0
    lastPacketsLost = currentLost
    packetLossHistory.value.push(lostDelta)

    // FPS
    fpsHistory.value.push(props.webrtcStats.framesPerSecond || 0)

    // Calculate bitrate
    const currentBytes = props.webrtcStats.bytesReceived || 0
    const currentTime = Date.now()
    if (lastTimestamp > 0 && currentBytes > lastBytesReceived) {
      const timeDiff = (currentTime - lastTimestamp) / 1000
      const bytesDiff = currentBytes - lastBytesReceived
      const bitrate = (bytesDiff * 8) / (timeDiff * 1000000)
      bitrateHistory.value.push(Math.round(bitrate * 100) / 100)
    } else {
      bitrateHistory.value.push(bitrateHistory.value[bitrateHistory.value.length - 1] || 0)
    }
    lastBytesReceived = currentBytes
    lastTimestamp = currentTime
  } else {
    // MJPEG mode
    jitterHistory.value.push(0)
    delayHistory.value.push(props.wsLatency || 0)
    packetLossHistory.value.push(0)
    fpsHistory.value.push(props.mjpegFps || 0)
    bitrateHistory.value.push(0)
  }

  // Trim arrays
  if (jitterHistory.value.length > MAX_POINTS) jitterHistory.value.shift()
  if (delayHistory.value.length > MAX_POINTS) delayHistory.value.shift()
  if (packetLossHistory.value.length > MAX_POINTS) packetLossHistory.value.shift()
  if (fpsHistory.value.length > MAX_POINTS) fpsHistory.value.shift()
  if (bitrateHistory.value.length > MAX_POINTS) bitrateHistory.value.shift()

  // Update charts
  updateCharts()
}

// Update charts with new data
function updateCharts() {
  stabilityChart?.setData([timestamps.value, jitterHistory.value])
  delayChart?.setData([timestamps.value, delayHistory.value])
  packetLossChart?.setData([timestamps.value, packetLossHistory.value])
  fpsChart?.setData([timestamps.value, fpsHistory.value])
}

// Data collection interval
let dataInterval: number | null = null

function startDataCollection() {
  if (dataInterval) return
  dataInterval = window.setInterval(addDataPoint, 1000)
}

function stopDataCollection() {
  if (dataInterval) {
    clearInterval(dataInterval)
    dataInterval = null
  }
}

// Format candidate type for display
function formatCandidateType(type: string): string {
  const typeMap: Record<string, string> = {
    host: 'Host (Local)',
    srflx: 'STUN (NAT)',
    prflx: 'Peer Reflexive',
    relay: 'TURN Relay',
    unknown: '-',
  }
  return typeMap[type] || type
}

// Current stats for header display
const currentStats = computed(() => {
  if (isWebRTC.value && props.webrtcStats) {
    const lastBitrate = bitrateHistory.value[bitrateHistory.value.length - 1]
    const bitrate = lastBitrate !== undefined ? lastBitrate : 0
    return {
      jitter: Math.round((props.webrtcStats.jitter || 0) * 1000 * 10) / 10,
      delay: Math.round((props.webrtcStats.roundTripTime || 0) * 1000),
      fps: props.webrtcStats.framesPerSecond || 0,
      resolution: props.webrtcStats.frameWidth && props.webrtcStats.frameHeight
        ? `${props.webrtcStats.frameWidth}x${props.webrtcStats.frameHeight}`
        : '-',
      bitrate: bitrate.toFixed(2),
      packetsLost: props.webrtcStats.packetsLost || 0,
      // ICE connection info
      isRelay: props.webrtcStats.isRelay || false,
      transport: (props.webrtcStats.transportProtocol || '-').toUpperCase(),
      localType: formatCandidateType(props.webrtcStats.localCandidateType || 'unknown'),
      remoteType: formatCandidateType(props.webrtcStats.remoteCandidateType || 'unknown'),
    }
  }
  return {
    jitter: 0,
    delay: props.wsLatency || 0,
    fps: props.mjpegFps || 0,
    resolution: '-',
    bitrate: '0',
    packetsLost: 0,
    isRelay: false,
    transport: '-',
    localType: '-',
    remoteType: '-',
  }
})

// Watch open state
watch(() => props.open, (isOpen) => {
  if (isOpen) {
    // Reset data
    timestamps.value = []
    jitterHistory.value = []
    delayHistory.value = []
    packetLossHistory.value = []
    fpsHistory.value = []
    bitrateHistory.value = []
    lastBytesReceived = 0
    lastPacketsLost = 0
    lastTimestamp = 0

    setTimeout(() => {
      initCharts()
      startDataCollection()
    }, 150)
  } else {
    stopDataCollection()
    destroyCharts()
  }
})

// Resize handler
function handleResize() {
  if (!props.open) return
  destroyCharts()
  setTimeout(initCharts, 50)
}

onMounted(() => {
  window.addEventListener('resize', handleResize)
  if (props.open) {
    initCharts()
    startDataCollection()
  }
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
  stopDataCollection()
  destroyCharts()
})
</script>

<template>
  <Sheet :open="props.open" @update:open="emit('update:open', $event)">
    <SheetContent
      side="right"
      class="w-[400px] sm:w-[440px] p-0 border-l border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950"
    >
      <!-- Header -->
      <SheetHeader class="px-6 py-3 border-b border-slate-200 dark:border-slate-800">
        <div class="flex items-center gap-2">
          <SheetTitle class="text-base">{{ t('stats.title') }}</SheetTitle>
          <span class="text-xs px-2 py-0.5 rounded bg-slate-100 dark:bg-slate-800 text-muted-foreground">
            {{ isWebRTC ? 'WebRTC' : 'MJPEG' }}
          </span>
        </div>
      </SheetHeader>

      <ScrollArea class="h-[calc(100vh-60px)]">
        <div class="px-6 py-4 space-y-6">
          <!-- Video Section Header -->
          <div>
            <h3 class="text-sm font-medium">{{ t('stats.video') }}</h3>
            <p class="text-xs text-muted-foreground mt-0.5">
              {{ t('stats.videoDesc') }}
            </p>
          </div>

          <!-- Network Stability (Jitter) -->
          <div class="space-y-2" v-if="isWebRTC">
            <div class="flex items-center justify-between">
              <h4 class="text-sm font-medium">{{ t('stats.stability') }}</h4>
            </div>
            <p class="text-xs text-muted-foreground">
              {{ t('stats.stabilityDesc') }}
            </p>
            <div class="relative">
              <div
                ref="stabilityChartRef"
                class="w-full rounded-lg bg-slate-50 dark:bg-slate-900/50 p-2"
              />
              <div
                v-if="activeTooltip.visible && activeTooltip.chartId === 'stability'"
                class="chart-tooltip"
                :style="{ left: `${activeTooltip.left + 60}px`, top: `${activeTooltip.top - 40}px` }"
              >
                <div class="text-xs font-medium">{{ activeTooltip.time }}</div>
                <div class="text-xs text-blue-500">{{ activeTooltip.value }} {{ activeTooltip.unit }}</div>
              </div>
            </div>
          </div>

          <!-- Playback Delay -->
          <div class="space-y-2" v-if="isWebRTC">
            <div class="flex items-center justify-between">
              <h4 class="text-sm font-medium">{{ t('stats.delay') }}</h4>
              <span class="text-xs text-muted-foreground">
                {{ currentStats.delay }} ms
              </span>
            </div>
            <p class="text-xs text-muted-foreground">
              {{ t('stats.delayDesc') }}
            </p>
            <div class="relative">
              <div
                ref="delayChartRef"
                class="w-full rounded-lg bg-slate-50 dark:bg-slate-900/50 p-2"
              />
              <div
                v-if="activeTooltip.visible && activeTooltip.chartId === 'delay'"
                class="chart-tooltip"
                :style="{ left: `${activeTooltip.left + 60}px`, top: `${activeTooltip.top - 40}px` }"
              >
                <div class="text-xs font-medium">{{ activeTooltip.time }}</div>
                <div class="text-xs text-blue-500">{{ activeTooltip.value }} {{ activeTooltip.unit }}</div>
              </div>
            </div>
          </div>

          <!-- Packet Loss -->
          <div class="space-y-2" v-if="isWebRTC">
            <div class="flex items-center justify-between">
              <h4 class="text-sm font-medium">{{ t('stats.packetLoss') }}</h4>
              <span class="text-xs text-muted-foreground">
                {{ currentStats.packetsLost }} {{ t('stats.total') }}
              </span>
            </div>
            <p class="text-xs text-muted-foreground">
              {{ t('stats.packetLossDesc') }}
            </p>
            <div class="relative">
              <div
                ref="packetLossChartRef"
                class="w-full rounded-lg bg-slate-50 dark:bg-slate-900/50 p-2"
              />
              <div
                v-if="activeTooltip.visible && activeTooltip.chartId === 'packetLoss'"
                class="chart-tooltip"
                :style="{ left: `${activeTooltip.left + 60}px`, top: `${activeTooltip.top - 40}px` }"
              >
                <div class="text-xs font-medium">{{ activeTooltip.time }}</div>
                <div class="text-xs text-blue-500">{{ activeTooltip.value }} {{ activeTooltip.unit }}</div>
              </div>
            </div>
          </div>

          <!-- FPS -->
          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <h4 class="text-sm font-medium">{{ t('stats.frameRate') }}</h4>
              <span class="text-xs text-muted-foreground">
                {{ currentStats.fps }} fps
              </span>
            </div>
            <p class="text-xs text-muted-foreground">
              {{ t('stats.frameRateDesc') }}
            </p>
            <div class="relative">
              <div
                ref="fpsChartRef"
                class="w-full rounded-lg bg-slate-50 dark:bg-slate-900/50 p-2"
              />
              <div
                v-if="activeTooltip.visible && activeTooltip.chartId === 'fps'"
                class="chart-tooltip"
                :style="{ left: `${activeTooltip.left + 60}px`, top: `${activeTooltip.top - 40}px` }"
              >
                <div class="text-xs font-medium">{{ activeTooltip.time }}</div>
                <div class="text-xs text-blue-500">{{ activeTooltip.value }} {{ activeTooltip.unit }}</div>
              </div>
            </div>
          </div>

          <!-- Additional Stats -->
          <div class="space-y-3 pt-2 border-t border-slate-200 dark:border-slate-800" v-if="isWebRTC">
            <h4 class="text-sm font-medium">{{ t('stats.additional') }}</h4>
            <div class="grid grid-cols-2 gap-3">
              <div class="rounded-lg bg-slate-50 dark:bg-slate-900/50 p-3">
                <p class="text-xs text-muted-foreground">{{ t('stats.resolution') }}</p>
                <p class="text-sm font-medium mt-1">{{ currentStats.resolution }}</p>
              </div>
              <div class="rounded-lg bg-slate-50 dark:bg-slate-900/50 p-3">
                <p class="text-xs text-muted-foreground">{{ t('stats.bitrate') }}</p>
                <p class="text-sm font-medium mt-1">{{ currentStats.bitrate }} Mbps</p>
              </div>
            </div>

            <!-- Connection Info -->
            <h4 class="text-sm font-medium pt-2">{{ t('stats.connection') }}</h4>
            <div class="grid grid-cols-2 gap-3">
              <div class="rounded-lg bg-slate-50 dark:bg-slate-900/50 p-3">
                <p class="text-xs text-muted-foreground">{{ t('stats.connectionType') }}</p>
                <p class="text-sm font-medium mt-1 flex items-center gap-1.5">
                  <span
                    :class="[
                      'inline-block w-2 h-2 rounded-full',
                      currentStats.isRelay ? 'bg-amber-500' : 'bg-green-500'
                    ]"
                  />
                  {{ currentStats.isRelay ? t('stats.relay') : t('stats.p2p') }}
                </p>
              </div>
              <div class="rounded-lg bg-slate-50 dark:bg-slate-900/50 p-3">
                <p class="text-xs text-muted-foreground">{{ t('stats.transport') }}</p>
                <p class="text-sm font-medium mt-1">{{ currentStats.transport }}</p>
              </div>
              <div class="rounded-lg bg-slate-50 dark:bg-slate-900/50 p-3">
                <p class="text-xs text-muted-foreground">{{ t('stats.localCandidate') }}</p>
                <p class="text-sm font-medium mt-1">{{ currentStats.localType }}</p>
              </div>
              <div class="rounded-lg bg-slate-50 dark:bg-slate-900/50 p-3">
                <p class="text-xs text-muted-foreground">{{ t('stats.remoteCandidate') }}</p>
                <p class="text-sm font-medium mt-1">{{ currentStats.remoteType }}</p>
              </div>
            </div>
          </div>
        </div>
      </ScrollArea>
    </SheetContent>
  </Sheet>
</template>

<style>
/* Override uPlot styles for dark mode */
.dark .u-wrap {
  background: transparent !important;
}

.dark .u-over {
  background: transparent !important;
}

/* Chart cursor line */
.u-cursor-x {
  border-right: 1px dashed #64748b !important;
}

.u-cursor-y {
  display: none !important;
}

/* Chart tooltip */
.chart-tooltip {
  position: absolute;
  z-index: 50;
  pointer-events: none;
  padding: 6px 10px;
  border-radius: 6px;
  background: rgba(15, 23, 42, 0.9);
  color: white;
  box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  white-space: nowrap;
  transform: translateX(-50%);
}

.dark .chart-tooltip {
  background: rgba(30, 41, 59, 0.95);
  border: 1px solid rgba(71, 85, 105, 0.5);
}
</style>
