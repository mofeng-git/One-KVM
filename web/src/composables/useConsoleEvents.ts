import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { useSystemStore } from '@/stores/system'
import { useWebSocket } from '@/composables/useWebSocket'

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
    /** Optional fine-grained diagnostic tag (e.g. `no_cable`, `out_of_range`, `recovering`). */
    reason?: string | null
    /** Optional countdown (ms) until the next backend self-recovery attempt. */
    next_retry_ms?: number | null
  }) => void
  onStreamDeviceLost?: (data: { device: string; reason: string }) => void
  onStreamReconnecting?: (data: { device: string; attempt: number }) => void
  onStreamRecovered?: (data: { device: string }) => void
  onDeviceInfo?: (data: any) => void
}

export function useConsoleEvents(handlers: ConsoleEventHandlers) {
  const { t } = useI18n()
  const systemStore = useSystemStore()
  const { on, off, connect } = useWebSocket()
  const noop = () => {}

  function handleStreamDeviceLost(data: { device: string; reason: string }) {
    if (systemStore.stream) {
      systemStore.stream.online = false
    }
    toast.error(t('console.deviceLost'), {
      description: t('console.deviceLostDesc', { device: data.device, reason: data.reason }),
      duration: 5000,
    })
    handlers.onStreamDeviceLost?.(data)
  }

  function handleStreamReconnecting(data: { device: string; attempt: number }) {
    if (data.attempt === 1 || data.attempt % 5 === 0) {
      toast.info(t('console.deviceRecovering'), {
        description: t('console.deviceRecoveringDesc', { attempt: data.attempt }),
        duration: 3000,
      })
    }
    handlers.onStreamReconnecting?.(data)
  }

  function handleStreamRecovered(_data: { device: string }) {
    if (systemStore.stream) {
      systemStore.stream.online = true
    }
    toast.success(t('console.deviceRecovered'), {
      description: t('console.deviceRecoveredDesc'),
      duration: 3000,
    })
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
