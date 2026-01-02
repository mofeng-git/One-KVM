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
  onDeviceInfo?: (data: any) => void
  onAudioStateChanged?: (data: { streaming: boolean; device: string | null }) => void
}

export function useConsoleEvents(handlers: ConsoleEventHandlers) {
  const { t } = useI18n()
  const systemStore = useSystemStore()
  const { on, off, connect } = useWebSocket()
  const unifiedAudio = getUnifiedAudio()

  // HID event handlers
  function handleHidStateChanged(_data: unknown) {
    // Empty handler to prevent warning - HID state handled via device_info
  }

  function handleHidDeviceLost(data: { backend: string; device?: string; reason: string; error_code: string }) {
    const temporaryErrors = ['eagain', 'eagain_retry']
    if (temporaryErrors.includes(data.error_code)) return

    if (systemStore.hid) {
      systemStore.hid.initialized = false
    }
    toast.error(t('hid.deviceLost'), {
      description: t('hid.deviceLostDesc', { backend: data.backend, reason: data.reason }),
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
    if (systemStore.hid) {
      systemStore.hid.initialized = true
    }
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
  }

  function handleStreamReconnecting(data: { device: string; attempt: number }) {
    if (data.attempt === 1 || data.attempt % 5 === 0) {
      toast.info(t('console.deviceRecovering'), {
        description: t('console.deviceRecoveringDesc', { attempt: data.attempt }),
        duration: 3000,
      })
    }
  }

  function handleStreamRecovered(_data: { device: string }) {
    if (systemStore.stream) {
      systemStore.stream.online = true
    }
    toast.success(t('console.deviceRecovered'), {
      description: t('console.deviceRecoveredDesc'),
      duration: 3000,
    })
  }

  function handleStreamStateChanged(data: { state: string }) {
    if (data.state === 'error') {
      // Handled by video stream composable
    }
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
    on('stream.config_changing', handlers.onStreamConfigChanging || (() => {}))
    on('stream.config_applied', handlers.onStreamConfigApplied || (() => {}))
    on('stream.stats_update', handlers.onStreamStatsUpdate || (() => {}))
    on('stream.mode_changed', handlers.onStreamModeChanged || (() => {}))
    on('stream.state_changed', handleStreamStateChanged)
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
    on('system.device_info', handlers.onDeviceInfo || (() => {}))

    // Connect WebSocket
    connect()
  }

  // Unsubscribe from all events
  function unsubscribe() {
    off('hid.state_changed', handleHidStateChanged)
    off('hid.device_lost', handleHidDeviceLost)
    off('hid.reconnecting', handleHidReconnecting)
    off('hid.recovered', handleHidRecovered)

    off('stream.config_changing', handlers.onStreamConfigChanging || (() => {}))
    off('stream.config_applied', handlers.onStreamConfigApplied || (() => {}))
    off('stream.stats_update', handlers.onStreamStatsUpdate || (() => {}))
    off('stream.mode_changed', handlers.onStreamModeChanged || (() => {}))
    off('stream.state_changed', handleStreamStateChanged)
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

    off('system.device_info', handlers.onDeviceInfo || (() => {}))
  }

  return {
    subscribe,
    unsubscribe,
  }
}
