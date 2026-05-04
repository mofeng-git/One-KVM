import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { useSystemStore } from '@/stores/system'
import type { StreamDeviceLostEventData } from '@/types/websocket'
import { useWebSocket } from '@/composables/useWebSocket'
import { isAudioStreamDeviceLostPayload } from '@/lib/streamSignal'

export interface ConsoleEventHandlers {
  onStreamConfigChanging?: (data: { reason?: string }) => void
  onStreamConfigApplied?: (data: { device: string; resolution: [number, number]; fps: number }) => void
  onStreamStatsUpdate?: (data: { clients?: number; clients_stat?: Record<string, { fps: number }> }) => void
  onStreamModeChanged?: (data: { mode: string; previous_mode: string }) => void
  onStreamModeSwitching?: (data: { transition_id: string; to_mode: string; from_mode: string }) => void
  onStreamModeReady?: (data: { transition_id: string; mode: string }) => void
  onWebRTCReady?: (data: { codec: string; hardware: boolean; transition_id?: string }) => void
  onStreamStateChanged?: (data: {
    state: string
    device?: string | null
    reason?: string | null
    next_retry_ms?: number | null
  }) => void
  onStreamDeviceLost?: (data: StreamDeviceLostEventData) => void
  onStreamReconnecting?: (data: { device: string; attempt: number }) => void
  onStreamRecovered?: (data: { device: string }) => void
  onDeviceInfo?: (data: any) => void
}

export function useConsoleEvents(handlers: ConsoleEventHandlers) {
  const { t } = useI18n()
  const systemStore = useSystemStore()
  const { on, off, connect } = useWebSocket()
  const noop = () => {}

  function handleStreamDeviceLost(data: StreamDeviceLostEventData) {
    const audioLost = isAudioStreamDeviceLostPayload(data)
    if (systemStore.stream && !audioLost) {
      systemStore.stream.online = false
    }
    toast.error(t(audioLost ? 'audio.deviceLost' : 'console.deviceLost'), {
      description: t(audioLost ? 'audio.deviceLostDesc' : 'console.deviceLostDesc', {
        device: data.device,
        reason: data.reason,
      }),
      duration: 5000,
    })
    handlers.onStreamDeviceLost?.(data)
  }

  function handleStreamReconnecting(data: { device: string; attempt: number }) {
    handlers.onStreamReconnecting?.(data)
  }

  function handleStreamRecovered(_data: { device: string }) {
    if (systemStore.stream) {
      systemStore.stream.online = true
    }
    handlers.onStreamRecovered?.(_data)
  }

  function handleStreamStateChangedForward(data: { state: string; device?: string | null }) {
    handlers.onStreamStateChanged?.(data)
  }

  function subscribe() {
    on('stream.config_changing', handlers.onStreamConfigChanging ?? noop)
    on('stream.config_applied', handlers.onStreamConfigApplied ?? noop)
    on('stream.stats_update', handlers.onStreamStatsUpdate ?? noop)
    on('stream.mode_changed', handlers.onStreamModeChanged ?? noop)
    on('stream.mode_switching', handlers.onStreamModeSwitching ?? noop)
    on('stream.mode_ready', handlers.onStreamModeReady ?? noop)
    on('stream.webrtc_ready', handlers.onWebRTCReady ?? noop)
    on('stream.state_changed', handleStreamStateChangedForward)
    on('stream.device_lost', handleStreamDeviceLost)
    on('stream.reconnecting', handleStreamReconnecting)
    on('stream.recovered', handleStreamRecovered)

    on('system.device_info', handlers.onDeviceInfo ?? noop)

    connect()
  }

  function unsubscribe() {
    off('stream.config_changing', handlers.onStreamConfigChanging ?? noop)
    off('stream.config_applied', handlers.onStreamConfigApplied ?? noop)
    off('stream.stats_update', handlers.onStreamStatsUpdate ?? noop)
    off('stream.mode_changed', handlers.onStreamModeChanged ?? noop)
    off('stream.mode_switching', handlers.onStreamModeSwitching ?? noop)
    off('stream.mode_ready', handlers.onStreamModeReady ?? noop)
    off('stream.webrtc_ready', handlers.onWebRTCReady ?? noop)
    off('stream.state_changed', handleStreamStateChangedForward)
    off('stream.device_lost', handleStreamDeviceLost)
    off('stream.reconnecting', handleStreamReconnecting)
    off('stream.recovered', handleStreamRecovered)

    off('system.device_info', handlers.onDeviceInfo ?? noop)
  }

  return {
    subscribe,
    unsubscribe,
  }
}
