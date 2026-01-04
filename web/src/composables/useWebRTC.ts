// WebRTC composable for H264 video streaming
// Provides low-latency video via WebRTC with DataChannel for HID

import { ref, onUnmounted, computed, type Ref } from 'vue'
import { webrtcApi } from '@/api'
import { generateUUID } from '@/lib/utils'
import {
  type HidKeyboardEvent,
  type HidMouseEvent,
  encodeKeyboardEvent,
  encodeMouseEvent,
} from '@/types/hid'

export type { HidKeyboardEvent, HidMouseEvent }

export type WebRTCState = 'disconnected' | 'connecting' | 'connected' | 'failed'

// ICE candidate type: host=P2P local, srflx=P2P STUN, relay=TURN relay
export type IceCandidateType = 'host' | 'srflx' | 'prflx' | 'relay' | 'unknown'

export interface WebRTCStats {
  bytesReceived: number
  packetsReceived: number
  packetsLost: number
  framesDecoded: number
  framesDropped: number
  frameWidth: number
  frameHeight: number
  framesPerSecond: number
  jitter: number
  roundTripTime: number
  // ICE connection info
  localCandidateType: IceCandidateType
  remoteCandidateType: IceCandidateType
  transportProtocol: string // 'udp' | 'tcp'
  isRelay: boolean // true if using TURN relay
}

// Cached ICE servers from backend API
let cachedIceServers: RTCIceServer[] | null = null

// Fetch ICE servers from backend API
async function fetchIceServers(): Promise<RTCIceServer[]> {
  try {
    const response = await webrtcApi.getIceServers()
    if (response.ice_servers && response.ice_servers.length > 0) {
      cachedIceServers = response.ice_servers.map(server => ({
        urls: server.urls,
        username: server.username,
        credential: server.credential,
      }))
      console.log('[WebRTC] ICE servers loaded from API:', cachedIceServers.length)
      return cachedIceServers
    }
  } catch (err) {
    console.warn('[WebRTC] Failed to fetch ICE servers from API, using fallback:', err)
  }

  // Fallback: for local connections, use no ICE servers (host candidates only)
  // For remote connections, use Google STUN as fallback
  const isLocalConnection = typeof window !== 'undefined' &&
    (window.location.hostname === 'localhost' ||
     window.location.hostname === '127.0.0.1' ||
     window.location.hostname.startsWith('192.168.') ||
     window.location.hostname.startsWith('10.'))

  if (isLocalConnection) {
    console.log('[WebRTC] Local connection detected, using host candidates only')
    return []
  }

  console.log('[WebRTC] Using fallback STUN servers')
  return [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' },
  ]
}

// Shared instance state
let peerConnection: RTCPeerConnection | null = null
let dataChannel: RTCDataChannel | null = null
let sessionId: string | null = null
let statsInterval: number | null = null
let isConnecting = false // Lock to prevent concurrent connect calls
let pendingIceCandidates: RTCIceCandidate[] = [] // Queue for ICE candidates before sessionId is set
let cachedMediaStream: MediaStream | null = null // Cached MediaStream to avoid recreating

const state = ref<WebRTCState>('disconnected')
const videoTrack = ref<MediaStreamTrack | null>(null)
const audioTrack = ref<MediaStreamTrack | null>(null)
const stats = ref<WebRTCStats>({
  bytesReceived: 0,
  packetsReceived: 0,
  packetsLost: 0,
  framesDecoded: 0,
  framesDropped: 0,
  frameWidth: 0,
  frameHeight: 0,
  framesPerSecond: 0,
  jitter: 0,
  roundTripTime: 0,
  localCandidateType: 'unknown',
  remoteCandidateType: 'unknown',
  transportProtocol: '',
  isRelay: false,
})
const error = ref<string | null>(null)
const dataChannelReady = ref(false)

// Create RTCPeerConnection with configuration
function createPeerConnection(iceServers: RTCIceServer[]): RTCPeerConnection {
  const config: RTCConfiguration = {
    iceServers,
    iceCandidatePoolSize: 10,
  }

  const pc = new RTCPeerConnection(config)

  // Handle connection state changes
  pc.onconnectionstatechange = () => {
    switch (pc.connectionState) {
      case 'connecting':
        state.value = 'connecting'
        break
      case 'connected':
        state.value = 'connected'
        error.value = null
        startStatsCollection()
        break
      case 'disconnected':
      case 'closed':
        state.value = 'disconnected'
        stopStatsCollection()
        break
      case 'failed':
        state.value = 'failed'
        error.value = 'Connection failed'
        stopStatsCollection()
        break
    }
  }

  // Handle ICE connection state
  pc.oniceconnectionstatechange = () => {
    // ICE state changes handled silently
  }

  // Handle ICE candidates
  pc.onicecandidate = async (event) => {
    if (!event.candidate) return

    const currentSessionId = sessionId
    if (currentSessionId && pc.connectionState !== 'closed') {
      // Session ready, send immediately
      try {
        await webrtcApi.addIceCandidate(currentSessionId, {
          candidate: event.candidate.candidate,
          sdpMid: event.candidate.sdpMid ?? undefined,
          sdpMLineIndex: event.candidate.sdpMLineIndex ?? undefined,
          usernameFragment: event.candidate.usernameFragment ?? undefined,
        })
      } catch {
        // ICE candidate send failures are non-fatal
      }
    } else if (!currentSessionId) {
      // Queue candidate until sessionId is set
      pendingIceCandidates.push(event.candidate)
    }
  }

  // Handle incoming tracks
  pc.ontrack = (event) => {
    const track = event.track

    if (track.kind === 'video') {
      videoTrack.value = track
    } else if (track.kind === 'audio') {
      audioTrack.value = track
    }
  }

  // Handle data channel from server
  pc.ondatachannel = (event) => {
    setupDataChannel(event.channel)
  }

  return pc
}

// Setup data channel event handlers
function setupDataChannel(channel: RTCDataChannel) {
  dataChannel = channel

  channel.onopen = () => {
    dataChannelReady.value = true
  }

  channel.onclose = () => {
    dataChannelReady.value = false
  }

  channel.onerror = () => {
    // Data channel errors handled silently
  }

  channel.onmessage = () => {
    // Handle incoming messages from server (e.g., LED status)
  }
}

// Create data channel for HID events
function createDataChannel(pc: RTCPeerConnection): RTCDataChannel {
  const channel = pc.createDataChannel('hid', {
    ordered: true,
    maxRetransmits: 3,
  })
  setupDataChannel(channel)
  return channel
}

// Start collecting stats
function startStatsCollection() {
  if (statsInterval) return

  statsInterval = window.setInterval(async () => {
    if (!peerConnection) return

    try {
      const report = await peerConnection.getStats()

      // Collect candidate info
      const candidates: Record<string, { type: IceCandidateType; protocol: string }> = {}
      let selectedPairLocalId = ''
      let selectedPairRemoteId = ''
      let foundActivePair = false

      report.forEach((stat) => {
        // Collect all candidates
        if (stat.type === 'local-candidate' || stat.type === 'remote-candidate') {
          candidates[stat.id] = {
            type: (stat.candidateType as IceCandidateType) || 'unknown',
            protocol: stat.protocol || '',
          }
        }

        // Find the active candidate pair
        // Priority: nominated > succeeded (for Chrome/Firefox compatibility)
        if (stat.type === 'candidate-pair') {
          // Check if this is the nominated/selected pair
          const isActive = stat.nominated === true ||
                          (stat.state === 'succeeded' && stat.selected === true) ||
                          (stat.state === 'in-progress' && !foundActivePair)

          // Also check if this pair has actual data transfer (more reliable indicator)
          const hasData = (stat.bytesReceived > 0 || stat.bytesSent > 0)

          if ((isActive || (stat.state === 'succeeded' && hasData)) && !foundActivePair) {
            stats.value.roundTripTime = stat.currentRoundTripTime || 0
            selectedPairLocalId = stat.localCandidateId || ''
            selectedPairRemoteId = stat.remoteCandidateId || ''
            if (stat.nominated === true || stat.selected === true) {
              foundActivePair = true
            }
          }
        }

        // Update video stats
        if (stat.type === 'inbound-rtp' && stat.kind === 'video') {
          stats.value.bytesReceived = stat.bytesReceived || 0
          stats.value.packetsReceived = stat.packetsReceived || 0
          stats.value.packetsLost = stat.packetsLost || 0
          stats.value.framesDecoded = stat.framesDecoded || 0
          stats.value.framesDropped = stat.framesDropped || 0
          stats.value.frameWidth = stat.frameWidth || 0
          stats.value.frameHeight = stat.frameHeight || 0
          stats.value.framesPerSecond = stat.framesPerSecond || 0
          stats.value.jitter = stat.jitter || 0
        }
      })

      // Update ICE connection info from selected pair
      const localCandidate = selectedPairLocalId ? candidates[selectedPairLocalId] : undefined
      const remoteCandidate = selectedPairRemoteId ? candidates[selectedPairRemoteId] : undefined

      if (localCandidate) {
        stats.value.localCandidateType = localCandidate.type
        stats.value.transportProtocol = localCandidate.protocol
      }
      if (remoteCandidate) {
        stats.value.remoteCandidateType = remoteCandidate.type
      }

      // Check if using TURN relay
      // TURN relay is when either local or remote candidate is 'relay' type
      stats.value.isRelay = stats.value.localCandidateType === 'relay' || stats.value.remoteCandidateType === 'relay'
    } catch {
      // Stats collection errors are non-fatal
    }
  }, 1000)
}

// Stop collecting stats
function stopStatsCollection() {
  if (statsInterval) {
    clearInterval(statsInterval)
    statsInterval = null
  }
}

// Send queued ICE candidates after sessionId is set
async function flushPendingIceCandidates() {
  if (!sessionId || pendingIceCandidates.length === 0) return

  const candidates = [...pendingIceCandidates]
  pendingIceCandidates = []

  for (const candidate of candidates) {
    try {
      await webrtcApi.addIceCandidate(sessionId, {
        candidate: candidate.candidate,
        sdpMid: candidate.sdpMid ?? undefined,
        sdpMLineIndex: candidate.sdpMLineIndex ?? undefined,
        usernameFragment: candidate.usernameFragment ?? undefined,
      })
    } catch {
      // ICE candidate send failures are non-fatal
    }
  }
}

// Connect to WebRTC server
async function connect(): Promise<boolean> {
  // Prevent concurrent connection attempts
  if (isConnecting) {
    return false
  }

  if (peerConnection && state.value === 'connected') {
    return true
  }

  isConnecting = true

  // Clean up any existing connection first
  if (peerConnection || sessionId) {
    await disconnect()
  }

  // Clear pending ICE candidates from previous attempt
  pendingIceCandidates = []

  try {
    state.value = 'connecting'
    error.value = null

    // Fetch ICE servers from backend API
    const iceServers = await fetchIceServers()

    // Create peer connection with fetched ICE servers
    peerConnection = createPeerConnection(iceServers)

    // Create data channel before offer (for HID)
    createDataChannel(peerConnection)

    // Add transceiver for receiving video
    peerConnection.addTransceiver('video', { direction: 'recvonly' })
    peerConnection.addTransceiver('audio', { direction: 'recvonly' })

    // Create offer
    const offer = await peerConnection.createOffer()
    await peerConnection.setLocalDescription(offer)

    // Send offer to server and get answer
    const response = await webrtcApi.offer(offer.sdp!, generateUUID())
    sessionId = response.session_id

    // Send any ICE candidates that were queued while waiting for sessionId
    await flushPendingIceCandidates()

    // Set remote description (answer)
    const answer: RTCSessionDescriptionInit = {
      type: 'answer',
      sdp: response.sdp,
    }
    await peerConnection.setRemoteDescription(answer)

    // Add any ICE candidates from the response
    if (response.ice_candidates && response.ice_candidates.length > 0) {
      for (const candidateObj of response.ice_candidates) {
        try {
          const iceCandidate: RTCIceCandidateInit = {
            candidate: candidateObj.candidate,
            sdpMid: candidateObj.sdpMid ?? '0',
            sdpMLineIndex: candidateObj.sdpMLineIndex ?? 0,
          }
          await peerConnection.addIceCandidate(iceCandidate)
        } catch {
          // ICE candidate add failures are non-fatal
        }
      }
    }

    // 等待连接真正建立（最多等待 15 秒）
    // 直接检查 peerConnection.connectionState 而不是 reactive state
    // 因为 TypeScript 不知道 state 会被 onconnectionstatechange 回调异步修改
    const connectionTimeout = 15000
    const pollInterval = 100
    let waited = 0

    while (waited < connectionTimeout && peerConnection) {
      const pcState = peerConnection.connectionState
      if (pcState === 'connected') {
        isConnecting = false
        return true
      }
      if (pcState === 'failed' || pcState === 'closed') {
        throw new Error('Connection failed during ICE negotiation')
      }
      await new Promise(resolve => setTimeout(resolve, pollInterval))
      waited += pollInterval
    }

    // 超时
    throw new Error('Connection timeout waiting for ICE negotiation')
  } catch (err) {
    state.value = 'failed'
    error.value = err instanceof Error ? err.message : 'Connection failed'
    isConnecting = false
    disconnect()
    return false
  }
}

// Disconnect from WebRTC server
async function disconnect() {
  stopStatsCollection()

  // Clear state FIRST to prevent ICE candidates from being sent
  const oldSessionId = sessionId
  sessionId = null
  isConnecting = false
  pendingIceCandidates = []

  if (dataChannel) {
    dataChannel.close()
    dataChannel = null
    dataChannelReady.value = false
  }

  if (peerConnection) {
    peerConnection.close()
    peerConnection = null
  }

  if (oldSessionId) {
    try {
      await webrtcApi.close(oldSessionId)
    } catch {
      // Ignore close errors
    }
  }

  videoTrack.value = null
  audioTrack.value = null
  cachedMediaStream = null // Clear cached stream on disconnect
  state.value = 'disconnected'
  error.value = null

  // Reset stats
  stats.value = {
    bytesReceived: 0,
    packetsReceived: 0,
    packetsLost: 0,
    framesDecoded: 0,
    framesDropped: 0,
    frameWidth: 0,
    frameHeight: 0,
    framesPerSecond: 0,
    jitter: 0,
    roundTripTime: 0,
    localCandidateType: 'unknown',
    remoteCandidateType: 'unknown',
    transportProtocol: '',
    isRelay: false,
  }
}

// Send keyboard event via DataChannel (binary format)
function sendKeyboard(event: HidKeyboardEvent): boolean {
  if (!dataChannel || dataChannel.readyState !== 'open') {
    return false
  }

  try {
    const buffer = encodeKeyboardEvent(event)
    dataChannel.send(buffer)
    return true
  } catch {
    return false
  }
}

// Send mouse event via DataChannel (binary format)
function sendMouse(event: HidMouseEvent): boolean {
  if (!dataChannel || dataChannel.readyState !== 'open') {
    return false
  }

  try {
    const buffer = encodeMouseEvent(event)
    dataChannel.send(buffer)
    return true
  } catch {
    return false
  }
}

// Get MediaStream for video element (cached to avoid recreating)
function getMediaStream(): MediaStream | null {
  if (!videoTrack.value && !audioTrack.value) {
    return null
  }

  // Reuse cached stream if tracks match
  if (cachedMediaStream) {
    const currentVideoTracks = cachedMediaStream.getVideoTracks()
    const currentAudioTracks = cachedMediaStream.getAudioTracks()

    const videoMatches = videoTrack.value
      ? currentVideoTracks.includes(videoTrack.value)
      : currentVideoTracks.length === 0
    const audioMatches = audioTrack.value
      ? currentAudioTracks.includes(audioTrack.value)
      : currentAudioTracks.length === 0

    if (videoMatches && audioMatches) {
      return cachedMediaStream
    }

    // Tracks changed, update the cached stream
    // Remove old tracks
    currentVideoTracks.forEach(t => cachedMediaStream!.removeTrack(t))
    currentAudioTracks.forEach(t => cachedMediaStream!.removeTrack(t))

    // Add new tracks
    if (videoTrack.value) cachedMediaStream.addTrack(videoTrack.value)
    if (audioTrack.value) cachedMediaStream.addTrack(audioTrack.value)

    return cachedMediaStream
  }

  // Create new cached stream
  cachedMediaStream = new MediaStream()
  if (videoTrack.value) {
    cachedMediaStream.addTrack(videoTrack.value)
  }
  if (audioTrack.value) {
    cachedMediaStream.addTrack(audioTrack.value)
  }
  return cachedMediaStream
}

// Composable export
export function useWebRTC() {
  onUnmounted(() => {
    // Don't disconnect on unmount - keep connection alive
    // Only disconnect when explicitly called
  })

  return {
    // State
    state: state as Ref<WebRTCState>,
    videoTrack,
    audioTrack,
    stats,
    error,
    dataChannelReady,
    sessionId: computed(() => sessionId),

    // Methods
    connect,
    disconnect,
    sendKeyboard,
    sendMouse,
    getMediaStream,

    // Computed
    isConnected: computed(() => state.value === 'connected'),
    isConnecting: computed(() => state.value === 'connecting'),
    hasVideo: computed(() => videoTrack.value !== null),
    hasAudio: computed(() => audioTrack.value !== null),
  }
}

// Cleanup on page unload
if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    disconnect()
  })
}
