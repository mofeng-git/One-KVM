// Unified Audio Manager
// Manages audio playback across different video modes (MJPEG/WebSocket and H264/WebRTC)
// Provides a single interface for volume control and audio source switching

import { ref, watch, type Ref } from 'vue'
import { getAudioPlayer } from './useAudioPlayer'

export type AudioMode = 'ws' | 'webrtc'

export interface UnifiedAudioState {
  audioMode: Ref<AudioMode>
  volume: Ref<number>
  muted: Ref<boolean>
  connected: Ref<boolean>
  playing: Ref<boolean>
  error: Ref<string | null>
}

export function useUnifiedAudio() {
  // === State ===
  const audioMode = ref<AudioMode>('ws')
  const volume = ref(0) // 0-1, default muted (browser autoplay policy)
  const muted = ref(false)
  const connected = ref(false)
  const playing = ref(false)
  const error = ref<string | null>(null)

  // === Internal References ===
  const wsPlayer = getAudioPlayer()
  let webrtcVideoElement: HTMLVideoElement | null = null

  // === Methods ===

  /**
   * Set the WebRTC video element reference
   * This is needed to control WebRTC audio volume
   */
  function setWebRTCElement(el: HTMLVideoElement | null) {
    // Only update if element is provided (don't clear on null to preserve reference)
    if (el) {
      webrtcVideoElement = el
      // Sync current volume to video element
      el.volume = volume.value
      // Mute if volume is 0 or explicitly muted
      const shouldMute = muted.value || volume.value === 0
      el.muted = shouldMute
    }
  }

  /**
   * Switch audio mode between WebSocket and WebRTC
   * Automatically handles connection state
   */
  async function switchMode(mode: AudioMode) {
    if (mode === audioMode.value) return

    const wasConnected = connected.value
    const wasPlaying = playing.value

    // Disconnect old mode
    if (audioMode.value === 'ws') {
      wsPlayer.disconnect()
    }
    // WebRTC audio doesn't need manual disconnect, handled by video element

    audioMode.value = mode

    // If was connected/playing and volume > 0, auto-connect new mode
    if ((wasConnected || wasPlaying) && volume.value > 0) {
      await connect()
    }

    // Update connection state
    updateConnectionState()
  }

  /**
   * Set volume (0-1)
   * Applies to both WS and WebRTC audio
   */
  function setVolume(v: number) {
    const newVolume = Math.max(0, Math.min(1, v))
    volume.value = newVolume

    // Sync to WS player
    wsPlayer.setVolume(newVolume)

    // Sync to WebRTC video element
    if (webrtcVideoElement) {
      const shouldMute = muted.value || newVolume === 0
      webrtcVideoElement.volume = newVolume
      webrtcVideoElement.muted = shouldMute
    }
  }

  /**
   * Set muted state
   */
  function setMuted(m: boolean) {
    muted.value = m

    // WS player: control via volume (no separate mute)
    if (audioMode.value === 'ws') {
      wsPlayer.setVolume(m ? 0 : volume.value)
    }

    // WebRTC video element
    if (webrtcVideoElement) {
      webrtcVideoElement.muted = m || volume.value === 0
    }
  }

  /**
   * Toggle muted state
   */
  function toggleMute() {
    setMuted(!muted.value)
  }

  /**
   * Connect audio based on current mode
   */
  async function connect() {
    error.value = null

    if (audioMode.value === 'ws') {
      try {
        await wsPlayer.connect()
        connected.value = wsPlayer.connected.value
        playing.value = wsPlayer.playing.value
      } catch (e) {
        error.value = e instanceof Error ? e.message : 'WS audio connect failed'
      }
    } else {
      // WebRTC audio is automatically connected via video track
      // Just ensure video element is not muted (if volume > 0)
      if (webrtcVideoElement) {
        webrtcVideoElement.muted = muted.value || volume.value === 0
        connected.value = true
        playing.value = !webrtcVideoElement.muted
      }
    }
  }

  /**
   * Disconnect audio
   */
  function disconnect() {
    if (audioMode.value === 'ws') {
      wsPlayer.disconnect()
    }

    // WebRTC audio: mute but don't disconnect (follows video element)
    if (webrtcVideoElement) {
      webrtcVideoElement.muted = true
    }

    connected.value = false
    playing.value = false
  }

  /**
   * Update connection state based on current mode
   */
  function updateConnectionState() {
    if (audioMode.value === 'ws') {
      connected.value = wsPlayer.connected.value
      playing.value = wsPlayer.playing.value
    } else {
      // WebRTC mode: check if video element has audio and is not muted
      connected.value = webrtcVideoElement !== null
      playing.value = webrtcVideoElement !== null && !webrtcVideoElement.muted
    }
  }

  // Watch WS player state changes
  watch(() => wsPlayer.connected.value, (newConnected) => {
    if (audioMode.value === 'ws') {
      connected.value = newConnected
    }
  })

  watch(() => wsPlayer.playing.value, (newPlaying) => {
    if (audioMode.value === 'ws') {
      playing.value = newPlaying
    }
  })

  watch(() => wsPlayer.error.value, (newError) => {
    if (audioMode.value === 'ws') {
      error.value = newError
    }
  })

  return {
    // State
    audioMode,
    volume,
    muted,
    connected,
    playing,
    error,

    // Methods
    setWebRTCElement,
    switchMode,
    setVolume,
    setMuted,
    toggleMute,
    connect,
    disconnect,
  }
}

// Singleton instance
let instance: ReturnType<typeof useUnifiedAudio> | null = null

/**
 * Get the singleton unified audio manager instance
 */
export function getUnifiedAudio() {
  if (!instance) {
    instance = useUnifiedAudio()
  }
  return instance
}
