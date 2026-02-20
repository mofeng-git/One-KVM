// Console WebSocket events composable - handles all WebSocket event subscriptions
// Extracted from ConsoleView.vue for better separation of concerns

import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { useSystemStore } from '@/stores/system'
import { useWebSocket } from '@/composables/useWebSocket'
import { getUnifiedAudio } from '@/composables/useUnifiedAudio'

export interface ConsoleEventHandlers {
  onStreamConfigChanging?: (data: { reason?: string }) => void
  onStreamConfigApplied?: (data: { device: string; resolution: [number, number]; fps: number }) => void
  onStreamStatsUpdate?: (data: { clients?: number; clients_stat?: Record<string, { fps: number }> }) => void
  onStreamModeChanged?: (data: { mode: string; previous_mode: string }) => void
  onStreamModeSwitching?: (data: { transition_id: string; to_mode: string; from_mode: string }) => void
  onStreamModeReady?: (data: { transition_id: string; mode: string }) => void
  onWebRTCReady?: (data: { codec: string; hardware: boolean; transition_id?: string }) => void
  onStreamStateChanged?: (data: { state: string; device?: string | null }) => void
  onStreamDeviceLost?: (data: { device: string; reason: string }) => void
  onStreamReconnecting?: (data: { device: string; attempt: number }) => void
  onStreamRecovered?: (data: { device: string }) => void
  onDeviceInfo?: (data: any) => void
  onAudioStateChanged?: (data: { streaming: boolean; device: string | null }) => void
}

export function useConsoleEvents(handlers: ConsoleEventHandlers) {
  const { t } = useI18n()
  const systemStore = useSystemStore()
  const { on, off, connect } = useWebSocket()
  const unifiedAudio = getUnifiedAudio()
  const noop = () => {}
  const HID_TOAST_DEDUPE_MS = 30_000
  const hidLastToastAt = new Map<string, number>()

  function hidErrorHint(errorCode?: string, backend?: string): string {
    switch (errorCode) {
      case 'udc_not_configured':
        return t('hid.errorHints.udcNotConfigured')
      case 'enoent':
        return t('hid.errorHints.hidDeviceMissing')
      case 'port_not_found':
      case 'port_not_opened':
        return t('hid.errorHints.portNotFound')
      case 'no_response':
        return t('hid.errorHints.noResponse')
      case 'protocol_error':
      case 'invalid_response':
        return t('hid.errorHints.protocolError')
      case 'health_check_failed':
      case 'health_check_join_failed':
        return t('hid.errorHints.healthCheckFailed')
      case 'eio':
      case 'epipe':
      case 'eshutdown':
        if (backend === 'otg') {
          return t('hid.errorHints.otgIoError')
        }
        if (backend === 'ch9329') {
          return t('hid.errorHints.ch9329IoError')
        }
        return t('hid.errorHints.ioError')
      default:
        return ''
    }
  }

  function formatHidReason(reason: string, errorCode?: string, backend?: string): string {
    const hint = hidErrorHint(errorCode, backend)
    if (!hint) return reason
    return `${reason} (${hint})`
  }

  // HID event handlers
  function handleHidStateChanged(data: {
    backend: string
    initialized: boolean
    error?: string | null
    error_code?: string | null
  }) {
    systemStore.updateHidStateFromEvent({
      backend: data.backend,
      initialized: data.initialized,
      error: data.error ?? null,
      error_code: data.error_code ?? null,
    })
  }

  function handleHidDeviceLost(data: { backend: string; device?: string; reason: string; error_code: string }) {
    const temporaryErrors = ['eagain', 'eagain_retry']
    if (temporaryErrors.includes(data.error_code)) return

    systemStore.updateHidStateFromEvent({
      backend: data.backend,
      initialized: false,
      error: data.reason,
      error_code: data.error_code,
    })

    const dedupeKey = `${data.backend}:${data.error_code}`
    const now = Date.now()
    const last = hidLastToastAt.get(dedupeKey) ?? 0
    if (now - last < HID_TOAST_DEDUPE_MS) {
      return
    }
    hidLastToastAt.set(dedupeKey, now)

    const reason = formatHidReason(data.reason, data.error_code, data.backend)
    toast.error(t('hid.deviceLost'), {
      description: t('hid.deviceLostDesc', { backend: data.backend, reason }),
      duration: 5000,
    })
  }

  function handleHidReconnecting(data: { backend: string; attempt: number }) {
    if (data.attempt === 1 || data.attempt % 5 === 0) {
      toast.info(t('hid.reconnecting'), {
        description: t('hid.reconnectingDesc', { attempt: data.attempt }),
        duration: 3000,
      })
    }
  }

  function handleHidRecovered(data: { backend: string }) {
    systemStore.updateHidStateFromEvent({
      backend: data.backend,
      initialized: true,
      error: null,
      error_code: null,
    })
    toast.success(t('hid.recovered'), {
      description: t('hid.recoveredDesc', { backend: data.backend }),
      duration: 3000,
    })
  }

  // Stream device monitoring handlers
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

  function handleStreamStateChanged(data: { state: string }) {
    if (data.state === 'error') {
      // Handled by video stream composable
    }
  }

  function handleStreamStateChangedForward(data: { state: string; device?: string | null }) {
    handleStreamStateChanged(data)
    handlers.onStreamStateChanged?.(data)
  }

  // Audio device monitoring handlers
  function handleAudioDeviceLost(data: { device?: string; reason: string; error_code: string }) {
    if (systemStore.audio) {
      systemStore.audio.streaming = false
      systemStore.audio.error = data.reason
    }
    toast.error(t('audio.deviceLost'), {
      description: t('audio.deviceLostDesc', { device: data.device || 'default', reason: data.reason }),
      duration: 5000,
    })
  }

  function handleAudioReconnecting(data: { attempt: number }) {
    if (data.attempt === 1 || data.attempt % 5 === 0) {
      toast.info(t('audio.reconnecting'), {
        description: t('audio.reconnectingDesc', { attempt: data.attempt }),
        duration: 3000,
      })
    }
  }

  function handleAudioRecovered(data: { device?: string }) {
    if (systemStore.audio) {
      systemStore.audio.error = null
    }
    toast.success(t('audio.recovered'), {
      description: t('audio.recoveredDesc', { device: data.device || 'default' }),
      duration: 3000,
    })
  }

  async function handleAudioStateChanged(data: { streaming: boolean; device: string | null }) {
    if (!data.streaming) {
      unifiedAudio.disconnect()
      return
    }
    handlers.onAudioStateChanged?.(data)
  }

  // MSD event handlers
  function handleMsdStateChanged(_data: { mode: string; connected: boolean }) {
    systemStore.fetchMsdState().catch(() => null)
  }

  function handleMsdImageMounted(data: { image_id: string; image_name: string; size: number; cdrom: boolean }) {
    toast.success(t('msd.imageMounted', { name: data.image_name }), {
      description: `${(data.size / 1024 / 1024).toFixed(2)} MB - ${data.cdrom ? 'CD-ROM' : 'Disk'}`,
      duration: 3000,
    })
    systemStore.fetchMsdState().catch(() => null)
  }

  function handleMsdImageUnmounted() {
    toast.info(t('msd.imageUnmounted'), {
      duration: 2000,
    })
    systemStore.fetchMsdState().catch(() => null)
  }

  function handleMsdError(data: { reason: string; error_code: string }) {
    if (systemStore.msd) {
      systemStore.msd.error = data.reason
    }
    toast.error(t('msd.error'), {
      description: t('msd.errorDesc', { reason: data.reason }),
      duration: 5000,
    })
  }

  function handleMsdRecovered() {
    if (systemStore.msd) {
      systemStore.msd.error = null
    }
    toast.success(t('msd.recovered'), {
      description: t('msd.recoveredDesc'),
      duration: 3000,
    })
  }

  // Subscribe to all events
  function subscribe() {
    // HID events
    on('hid.state_changed', handleHidStateChanged)
    on('hid.device_lost', handleHidDeviceLost)
    on('hid.reconnecting', handleHidReconnecting)
    on('hid.recovered', handleHidRecovered)

    // Stream events
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

    // Audio events
    on('audio.state_changed', handleAudioStateChanged)
    on('audio.device_lost', handleAudioDeviceLost)
    on('audio.reconnecting', handleAudioReconnecting)
    on('audio.recovered', handleAudioRecovered)

    // MSD events
    on('msd.state_changed', handleMsdStateChanged)
    on('msd.image_mounted', handleMsdImageMounted)
    on('msd.image_unmounted', handleMsdImageUnmounted)
    on('msd.error', handleMsdError)
    on('msd.recovered', handleMsdRecovered)

    // System events
    on('system.device_info', handlers.onDeviceInfo ?? noop)

    // Connect WebSocket
    connect()
  }

  // Unsubscribe from all events
  function unsubscribe() {
    off('hid.state_changed', handleHidStateChanged)
    off('hid.device_lost', handleHidDeviceLost)
    off('hid.reconnecting', handleHidReconnecting)
    off('hid.recovered', handleHidRecovered)

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

    off('audio.state_changed', handleAudioStateChanged)
    off('audio.device_lost', handleAudioDeviceLost)
    off('audio.reconnecting', handleAudioReconnecting)
    off('audio.recovered', handleAudioRecovered)

    off('msd.state_changed', handleMsdStateChanged)
    off('msd.image_mounted', handleMsdImageMounted)
    off('msd.image_unmounted', handleMsdImageUnmounted)
    off('msd.error', handleMsdError)
    off('msd.recovered', handleMsdRecovered)

    off('system.device_info', handlers.onDeviceInfo ?? noop)
  }

  return {
    subscribe,
    unsubscribe,
  }
}
