// Audio player composable - handles WebSocket connection, Opus decoding, and Web Audio API playback

import { ref, watch } from 'vue'
import { OpusDecoder } from 'opus-decoder'

// Binary protocol header format (15 bytes)
// [type:1][timestamp:4][duration:2][sequence:4][length:4][data:...]

export function useAudioPlayer() {
  // State
  const connected = ref(false)
  const playing = ref(false)
  const volume = ref(0) // Default to 0, user must adjust to enable audio (browser autoplay policy)
  const error = ref<string | null>(null)

  // Internal variables
  let ws: WebSocket | null = null
  let audioContext: AudioContext | null = null
  let gainNode: GainNode | null = null
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let decoder: any = null
  let nextPlayTime = 0
  let isConnecting = false // Prevent concurrent connection attempts

  // Initialize decoder
  async function initDecoder() {
    const opusDecoder = new OpusDecoder({
      channels: 2,
      sampleRate: 48000,
    })
    await opusDecoder.ready
    decoder = opusDecoder
  }

  // Initialize audio context
  function initAudioContext() {
    audioContext = new AudioContext({ sampleRate: 48000 })
    gainNode = audioContext.createGain()
    gainNode.connect(audioContext.destination)
    updateVolume()
  }

  // Connect to WebSocket
  async function connect() {
    // Prevent concurrent connection attempts (critical fix for multiple WS connections)
    if (isConnecting) {
      return
    }

    // Check if already connected
    if (ws) {
      if (ws.readyState === WebSocket.OPEN) {
        return
      }
      if (ws.readyState === WebSocket.CONNECTING) {
        return
      }
      // CLOSING or CLOSED - close and reconnect
      ws.close()
      ws = null
    }

    isConnecting = true

    try {
      // Initialize
      if (!decoder) await initDecoder()
      if (!audioContext) initAudioContext()

      // Resume AudioContext (browser autoplay policy)
      if (audioContext?.state === 'suspended') {
        await audioContext.resume()
      }

      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
      const url = `${protocol}//${location.host}/api/ws/audio`

      ws = new WebSocket(url)
      ws.binaryType = 'arraybuffer'

      ws.onopen = () => {
        isConnecting = false
        connected.value = true
        playing.value = true
        error.value = null
        nextPlayTime = audioContext!.currentTime
      }

      ws.onmessage = (event) => {
        if (event.data instanceof ArrayBuffer) {
          handleAudioPacket(event.data)
        }
      }

      ws.onclose = () => {
        isConnecting = false
        connected.value = false
        playing.value = false
      }

      ws.onerror = () => {
        isConnecting = false
        error.value = 'WebSocket connection failed'
      }
    } catch (e) {
      isConnecting = false
      error.value = e instanceof Error ? e.message : 'Failed to initialize audio'
    }
  }

  // Disconnect
  function disconnect() {
    if (ws) {
      ws.close()
      ws = null
    }
    connected.value = false
    playing.value = false
  }

  // Handle audio packet
  function handleAudioPacket(buffer: ArrayBuffer) {
    if (!decoder || !audioContext || !gainNode) {
      return
    }
    if (audioContext.state !== 'running') {
      audioContext.resume()
    }

    try {
      // Parse Opus data (skip 15 bytes header)
      const opusData = new Uint8Array(buffer, 15)

      // Decode Opus -> PCM
      const decoded = decoder.decodeFrame(opusData)
      if (!decoded || !decoded.channelData || decoded.channelData.length === 0) {
        return
      }

      const samplesPerChannel = decoded.samplesDecoded
      const channels = decoded.channelData.length

      // Create audio buffer
      const audioBuffer = audioContext.createBuffer(
        channels,
        samplesPerChannel,
        48000
      )

      // Fill channel data
      for (let ch = 0; ch < channels; ch++) {
        const channelData = audioBuffer.getChannelData(ch)
        const sourceData = decoded.channelData[ch]
        if (sourceData) {
          channelData.set(sourceData)
        }
      }

      // Schedule playback
      const source = audioContext.createBufferSource()
      source.buffer = audioBuffer
      source.connect(gainNode)

      const now = audioContext.currentTime
      const scheduledAhead = nextPlayTime - now

      // Reset if too far behind (audio was paused/lagged)
      if (nextPlayTime < now) {
        nextPlayTime = now + 0.02 // Start 20ms ahead
      }

      // Reset if buffer too large (> 500ms ahead)
      if (scheduledAhead > 0.5) {
        nextPlayTime = now + 0.05 // Keep 50ms buffer
      }

      source.start(nextPlayTime)
      nextPlayTime += audioBuffer.duration
    } catch {
      // Ignore decode errors
    }
  }

  // Update volume
  function updateVolume() {
    if (gainNode) {
      gainNode.gain.value = volume.value
    }
  }

  // Set volume
  function setVolume(v: number) {
    volume.value = Math.max(0, Math.min(1, v))
    updateVolume()
  }

  // Watch volume changes
  watch(volume, updateVolume)

  return {
    // State
    connected,
    playing,
    volume,
    error,
    // Methods
    connect,
    disconnect,
    setVolume,
  }
}

// Singleton export
let instance: ReturnType<typeof useAudioPlayer> | null = null

export function getAudioPlayer() {
  if (!instance) {
    instance = useAudioPlayer()
  }
  return instance
}
