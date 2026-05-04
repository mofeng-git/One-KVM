import { ref, onUnmounted, computed, type Ref } from 'vue'
import { webrtcApi, type IceCandidate } from '@/api'
import {
  type HidKeyboardEvent,
  type HidMouseEvent,
  encodeKeyboardEvent,
  encodeMouseEvent,
} from '@/types/hid'
import { videoDebugLog } from '@/lib/debugLog'

export type { HidKeyboardEvent, HidMouseEvent }

export type WebRTCState = 'disconnected' | 'connecting' | 'connected' | 'failed'
export type WebRTCConnectStage =
  | 'idle'
  | 'fetching_ice_servers'
  | 'creating_peer_connection'
  | 'creating_data_channel'
  | 'creating_offer'
  | 'waiting_server_answer'
  | 'setting_remote_description'
  | 'applying_ice_candidates'
  | 'waiting_connection'
  | 'connected'
  | 'disconnected'
  | 'failed'

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
  localCandidateType: IceCandidateType
  remoteCandidateType: IceCandidateType
  transportProtocol: string
  isRelay: boolean
}

let cachedIceServers: RTCIceServer[] | null = null

async function fetchIceServers(): Promise<RTCIceServer[]> {
  try {
    const response = await webrtcApi.getIceServers()
    if (response.mdns_mode) {
      allowMdnsHostCandidates = response.mdns_mode !== 'disabled'
    } else if (response.ice_servers) {
      allowMdnsHostCandidates = response.ice_servers.length === 0
    }

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

  const isLocalConnection = typeof window !== 'undefined' &&
    (window.location.hostname === 'localhost' ||
     window.location.hostname === '127.0.0.1' ||
     window.location.hostname.startsWith('192.168.') ||
     window.location.hostname.startsWith('10.'))

  if (isLocalConnection) {
    allowMdnsHostCandidates = false
    console.log('[WebRTC] Local connection detected, using host candidates only')
    return []
  }

  console.log('[WebRTC] Using fallback STUN servers')
  return [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' },
  ]
}

let peerConnection: RTCPeerConnection | null = null
let dataChannel: RTCDataChannel | null = null
let sessionId: string | null = null
const sessionIdRef = ref<string | null>(null)
let statsInterval: number | null = null
let isConnecting = false
let connectInFlight: Promise<boolean> | null = null
let pendingIceCandidates: RTCIceCandidate[] = []
let seenRemoteCandidates = new Set<string>()
let cachedMediaStream: MediaStream | null = null

let allowMdnsHostCandidates = false

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
const connectStage = ref<WebRTCConnectStage>('idle')

function setConnectStage(stage: WebRTCConnectStage, details?: unknown) {
  connectStage.value = stage
  videoDebugLog(`WebRTC stage -> ${stage}`, details)
}

function getIceCandidatePoolSize(): number {
  if (typeof window === 'undefined') return 0
  const icePoolParam = new URLSearchParams(window.location.search).get('ice_pool')
  if (icePoolParam === null) return 0
  return Math.max(0, Number.parseInt(icePoolParam, 10) || 0)
}

function summarizeIceCandidate(candidate: RTCIceCandidate | IceCandidate | RTCIceCandidateInit | null) {
  if (!candidate) return null
  const candidateLine = candidate.candidate ?? ''
  const parts = candidateLine.trim().split(/\s+/)
  const typIndex = parts.indexOf('typ')
  const raddrIndex = parts.indexOf('raddr')
  const rportIndex = parts.indexOf('rport')

  return {
    type: typIndex >= 0 ? parts[typIndex + 1] : 'unknown',
    protocol: parts[2] ?? '',
    address: parts[4] ?? '',
    port: parts[5] ?? '',
    relatedAddress: raddrIndex >= 0 ? parts[raddrIndex + 1] : undefined,
    relatedPort: rportIndex >= 0 ? parts[rportIndex + 1] : undefined,
    sdpMid: candidate.sdpMid ?? undefined,
    sdpMLineIndex: candidate.sdpMLineIndex ?? undefined,
    usernameFragment: candidate.usernameFragment ?? undefined,
    raw: candidateLine,
  }
}

function createPeerConnection(iceServers: RTCIceServer[]): RTCPeerConnection {
  const config: RTCConfiguration = {
    iceServers,
    iceCandidatePoolSize: getIceCandidatePoolSize(),
  }

  const pc = new RTCPeerConnection(config)

  pc.onconnectionstatechange = () => {
    switch (pc.connectionState) {
      case 'connecting':
        state.value = 'connecting'
        break
      case 'connected':
        state.value = 'connected'
        setConnectStage('connected')
        error.value = null
        startStatsCollection()
        break
      case 'disconnected':
      case 'closed':
        state.value = 'disconnected'
        setConnectStage('disconnected')
        stopStatsCollection()
        break
      case 'failed':
        state.value = 'failed'
        setConnectStage('failed')
        error.value = 'Connection failed'
        stopStatsCollection()
        break
    }
  }

  pc.onicecandidate = async (event) => {
    if (!event.candidate) {
      return
    }
    if (shouldSkipLocalCandidate(event.candidate)) {
      return
    }

    const currentSessionId = sessionId
    if (currentSessionId && pc.connectionState !== 'closed') {
      try {
        await webrtcApi.addIceCandidate(currentSessionId, {
          candidate: event.candidate.candidate,
          sdpMid: event.candidate.sdpMid ?? undefined,
          sdpMLineIndex: event.candidate.sdpMLineIndex ?? undefined,
          usernameFragment: event.candidate.usernameFragment ?? undefined,
        })
      } catch (err) {
        videoDebugLog('Failed to send local ICE candidate', {
          sessionId: currentSessionId,
          candidate: summarizeIceCandidate(event.candidate),
          error: err,
        })
      }
    } else if (!currentSessionId) {
      pendingIceCandidates.push(event.candidate)
    }
  }

  pc.ontrack = (event) => {
    const track = event.track

    if (track.kind === 'video') {
      videoTrack.value = track
    } else if (track.kind === 'audio') {
      audioTrack.value = track
    }
  }

  pc.ondatachannel = (event) => {
    setupDataChannel(event.channel)
  }

  return pc
}

function setupDataChannel(channel: RTCDataChannel) {
  dataChannel = channel

  channel.onopen = () => {
    dataChannelReady.value = true
  }

  channel.onclose = () => {
    dataChannelReady.value = false
  }

  channel.onerror = (event) => {
    videoDebugLog('WebRTC data channel error', {
      label: channel.label,
      readyState: channel.readyState,
      event,
      sessionId,
    })
  }

  channel.onmessage = () => {
  }
}

function createDataChannel(pc: RTCPeerConnection): RTCDataChannel {
  const channel = pc.createDataChannel('hid', {
    ordered: true,
    maxRetransmits: 3,
  })
  setupDataChannel(channel)
  return channel
}

function shouldSkipLocalCandidate(candidate: RTCIceCandidate): boolean {
  if (allowMdnsHostCandidates) return false
  const value = candidate.candidate || ''
  return value.includes(' typ host') && value.includes('.local')
}

async function addRemoteIceCandidate(candidate: IceCandidate): Promise<boolean> {
  if (!peerConnection) return false
  if (!candidate.candidate) return false
  if (seenRemoteCandidates.has(candidate.candidate)) {
    return false
  }
  seenRemoteCandidates.add(candidate.candidate)

  const iceCandidate: RTCIceCandidateInit = {
    candidate: candidate.candidate,
    sdpMid: candidate.sdpMid ?? undefined,
    sdpMLineIndex: candidate.sdpMLineIndex ?? undefined,
    usernameFragment: candidate.usernameFragment ?? undefined,
  }

  try {
    await peerConnection.addIceCandidate(iceCandidate)
    return true
  } catch (err) {
    videoDebugLog('Failed to apply remote ICE candidate', {
      sessionId,
      candidate: summarizeIceCandidate(candidate),
      error: err,
    })
    return false
  }
}

function startStatsCollection() {
  if (statsInterval) return

  statsInterval = window.setInterval(async () => {
    if (!peerConnection) return

    try {
      const report = await peerConnection.getStats()

      const candidates: Record<string, { type: IceCandidateType; protocol: string }> = {}
      let selectedPairLocalId = ''
      let selectedPairRemoteId = ''
      let foundActivePair = false

      report.forEach((stat) => {
        if (stat.type === 'local-candidate' || stat.type === 'remote-candidate') {
          candidates[stat.id] = {
            type: (stat.candidateType as IceCandidateType) || 'unknown',
            protocol: stat.protocol || '',
          }
        }

        if (stat.type === 'candidate-pair') {
          const isActive = stat.nominated === true ||
                          (stat.state === 'succeeded' && stat.selected === true) ||
                          (stat.state === 'in-progress' && !foundActivePair)

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

      const localCandidate = selectedPairLocalId ? candidates[selectedPairLocalId] : undefined
      const remoteCandidate = selectedPairRemoteId ? candidates[selectedPairRemoteId] : undefined

      if (localCandidate) {
        stats.value.localCandidateType = localCandidate.type
        stats.value.transportProtocol = localCandidate.protocol
      }
      if (remoteCandidate) {
        stats.value.remoteCandidateType = remoteCandidate.type
      }

      stats.value.isRelay = stats.value.localCandidateType === 'relay' || stats.value.remoteCandidateType === 'relay'
    } catch {
    }
  }, 1000)
}

function stopStatsCollection() {
  if (statsInterval) {
    clearInterval(statsInterval)
    statsInterval = null
  }
}

async function flushPendingIceCandidates() {
  if (!sessionId || pendingIceCandidates.length === 0) return

  const currentSessionId = sessionId
  const candidates = [...pendingIceCandidates]
  pendingIceCandidates = []

  const sendTasks = candidates.map(async (candidate) => {
    if (shouldSkipLocalCandidate(candidate)) {
      return
    }
    try {
      await webrtcApi.addIceCandidate(currentSessionId, {
        candidate: candidate.candidate,
        sdpMid: candidate.sdpMid ?? undefined,
        sdpMLineIndex: candidate.sdpMLineIndex ?? undefined,
        usernameFragment: candidate.usernameFragment ?? undefined,
      })
    } catch (err) {
      videoDebugLog('Failed to send queued local ICE candidate', {
        sessionId: currentSessionId,
        candidate: summarizeIceCandidate(candidate),
        error: err,
      })
    }
  })

  await Promise.allSettled(sendTasks)
}

async function connect(): Promise<boolean> {
  if (connectInFlight) {
    return connectInFlight
  }

  connectInFlight = (async () => {
    if (isConnecting) {
      return state.value === 'connected'
    }

    if (peerConnection && state.value === 'connected') {
      return true
    }

    isConnecting = true

    if (peerConnection || sessionId) {
      await disconnect()
    }

    pendingIceCandidates = []
    seenRemoteCandidates.clear()

    try {
      state.value = 'connecting'
      error.value = null
      setConnectStage('fetching_ice_servers')

      const iceServers = await fetchIceServers()
      setConnectStage('creating_peer_connection', { iceServerCount: iceServers.length })

      peerConnection = createPeerConnection(iceServers)

      setConnectStage('creating_data_channel')
      createDataChannel(peerConnection)

      peerConnection.addTransceiver('video', { direction: 'recvonly' })
      peerConnection.addTransceiver('audio', { direction: 'recvonly' })
      setConnectStage('creating_offer')

      const offer = await peerConnection.createOffer()
      await peerConnection.setLocalDescription(offer)
      setConnectStage('waiting_server_answer')

      // Do not pass client_id here: each connect creates a fresh session.
      const response = await webrtcApi.offer(offer.sdp!)
      sessionId = response.session_id
      sessionIdRef.value = response.session_id

      const answer: RTCSessionDescriptionInit = {
        type: 'answer',
        sdp: response.sdp,
      }
      setConnectStage('setting_remote_description', { sessionId })
      await peerConnection.setRemoteDescription(answer)

      setConnectStage('applying_ice_candidates', {
        sessionId,
        answerCandidates: response.ice_candidates?.length ?? 0,
      })
      let appliedAnswerCandidates = 0
      for (const candidate of response.ice_candidates ?? []) {
        if (await addRemoteIceCandidate(candidate)) {
          appliedAnswerCandidates += 1
        }
      }
      try {
        await peerConnection.addIceCandidate(null)
      } catch (err) {
        videoDebugLog('Failed to apply remote ICE end-of-candidates from answer response', {
          sessionId,
          appliedAnswerCandidates,
          error: err,
        })
      }

      void flushPendingIceCandidates()

      const connectionTimeout = 5000
      const iceConnectedTimeout = 12000
      const pollInterval = 100
      let waited = 0
      setConnectStage('waiting_connection', {
        sessionId,
        connectionTimeout,
        iceConnectedTimeout,
        pollInterval,
      })

      while (peerConnection) {
        const pcState = peerConnection.connectionState
        const iceState = peerConnection.iceConnectionState
        const timeoutForState = iceState === 'connected' || iceState === 'completed'
          ? iceConnectedTimeout
          : connectionTimeout
        if (waited >= timeoutForState) break

        if (pcState === 'connected') {
          setConnectStage('connected', { sessionId, waited })
          isConnecting = false
          return true
        }
        if (pcState === 'failed' || pcState === 'closed') {
          throw new Error('Connection failed during ICE negotiation')
        }
        await new Promise(resolve => setTimeout(resolve, pollInterval))
        waited += pollInterval
      }

      videoDebugLog('WebRTC connect timed out waiting for ICE/DTLS', {
        sessionId,
        waited,
        connectionState: peerConnection?.connectionState,
        iceConnectionState: peerConnection?.iceConnectionState,
        iceGatheringState: peerConnection?.iceGatheringState,
        signalingState: peerConnection?.signalingState,
      })
      throw new Error('Connection timeout waiting for ICE negotiation')
    } catch (err) {
      state.value = 'failed'
      setConnectStage('failed', {
        sessionId,
        error: err,
        connectionState: peerConnection?.connectionState,
        iceConnectionState: peerConnection?.iceConnectionState,
        iceGatheringState: peerConnection?.iceGatheringState,
        signalingState: peerConnection?.signalingState,
      })
      error.value = err instanceof Error ? err.message : 'Connection failed'
      isConnecting = false
      await disconnect()
      return false
    }
  })()

  try {
    return await connectInFlight
  } finally {
    connectInFlight = null
  }
}

async function disconnect() {
  stopStatsCollection()

  // Clear state FIRST to prevent ICE candidates from being sent
  const oldSessionId = sessionId
  sessionId = null
  sessionIdRef.value = null
  isConnecting = false
  pendingIceCandidates = []
  seenRemoteCandidates.clear()

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
    } catch (err) {
      videoDebugLog('Failed to close backend WebRTC session', {
        sessionId: oldSessionId,
        error: err,
      })
    }
  }

  videoTrack.value = null
  audioTrack.value = null
  cachedMediaStream = null
  state.value = 'disconnected'
  setConnectStage('disconnected', { previousSessionId: oldSessionId })
  error.value = null

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

function getMediaStream(): MediaStream | null {
  if (!videoTrack.value && !audioTrack.value) {
    return null
  }

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

    currentVideoTracks.forEach(t => cachedMediaStream!.removeTrack(t))
    currentAudioTracks.forEach(t => cachedMediaStream!.removeTrack(t))

    if (videoTrack.value) cachedMediaStream.addTrack(videoTrack.value)
    if (audioTrack.value) cachedMediaStream.addTrack(audioTrack.value)

    return cachedMediaStream
  }

  cachedMediaStream = new MediaStream()
  if (videoTrack.value) {
    cachedMediaStream.addTrack(videoTrack.value)
  }
  if (audioTrack.value) {
    cachedMediaStream.addTrack(audioTrack.value)
  }
  return cachedMediaStream
}

export function useWebRTC() {
  onUnmounted(() => {
  })

  return {
    state: state as Ref<WebRTCState>,
    videoTrack,
    audioTrack,
    stats,
    error,
    dataChannelReady,
    connectStage,
    sessionId: sessionIdRef,

    connect,
    disconnect,
    sendKeyboard,
    sendMouse,
    getMediaStream,

    isConnected: computed(() => state.value === 'connected'),
    isConnecting: computed(() => state.value === 'connecting'),
    hasVideo: computed(() => videoTrack.value !== null),
    hasAudio: computed(() => audioTrack.value !== null),
  }
}

if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', () => {
    disconnect()
  })
}
