import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { useSystemStore, type DeviceInfoEvent } from '@/stores/system'
import type { StreamDeviceLostEventData, StreamStateChangedEventData } from '@/types/websocket'
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
  onStreamStateChanged?: (data: StreamStateChangedEventData) => void
  onStreamDeviceLost?: (data: StreamDeviceLostEventData) => void
  onStreamReconnecting?: (data: { device: string; attempt: number }) => void
  onStreamRecovered?: (data: { device: string }) => void
  onDeviceInfo?: (data: DeviceInfoEvent) => void
}

export function useConsoleEvents(handlers: ConsoleEventHandlers) {
  const { t } = useI18n()
  const systemStore = useSystemStore()
  const { on, off, connect } = useWebSocket()
  let subscribed = false

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

  function handleStreamRecovered(data: { device: string }) {
    if (systemStore.stream) {
      systemStore.stream.online = true
    }
    handlers.onStreamRecovered?.(data)
  }

  // Keep registration and cleanup together so every subscription is removed.
  const subscriptions = [
    ['stream.config_changing', handlers.onStreamConfigChanging],
    ['stream.config_applied', handlers.onStreamConfigApplied],
    ['stream.stats_update', handlers.onStreamStatsUpdate],
    ['stream.mode_changed', handlers.onStreamModeChanged],
    ['stream.mode_switching', handlers.onStreamModeSwitching],
    ['stream.mode_ready', handlers.onStreamModeReady],
    ['stream.webrtc_ready', handlers.onWebRTCReady],
    ['stream.state_changed', handlers.onStreamStateChanged],
    ['stream.device_lost', handleStreamDeviceLost],
    ['stream.reconnecting', handlers.onStreamReconnecting],
    ['stream.recovered', handleStreamRecovered],
    ['system.device_info', handlers.onDeviceInfo],
  ] as const

  function subscribe() {
    if (subscribed) return
    subscribed = true
    for (const [event, handler] of subscriptions) {
      if (handler) on(event, handler)
    }
    connect()
  }

  function unsubscribe() {
    if (!subscribed) return
    subscribed = false
    for (const [event, handler] of subscriptions) {
      if (handler) off(event, handler)
    }
  }

  return {
    subscribe,
    unsubscribe,
  }
}
