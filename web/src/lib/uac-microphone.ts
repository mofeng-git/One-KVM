const SAMPLE_RATE = 48_000
const CHANNELS = 2
const OPUS_BITRATE = 64_000
// Keep capture delivery close to Opus' 20 ms packet cadence instead of
// releasing several packets in an 85 ms burst.
const PROCESSOR_BUFFER_SIZE = 1024
const SOCKET_OPEN_TIMEOUT_MS = 8_000
const MAX_SOCKET_BUFFER_BYTES = 256 * 1024
const MAX_ENCODER_QUEUE_SIZE = 4

const OPUS_CONFIG: AudioEncoderConfig = {
  codec: 'opus',
  sampleRate: SAMPLE_RATE,
  numberOfChannels: CHANNELS,
  bitrate: OPUS_BITRATE,
}

export type UacMicrophoneErrorCode =
  | 'secure-context-required'
  | 'unsupported'
  | 'permission-denied'
  | 'device-unavailable'
  | 'connection-failed'
  | 'encoder-failed'
  | 'unknown'

export type UacTargetState = 'idle' | 'waiting' | 'active' | 'stalled'

export class UacMicrophoneError extends Error {
  constructor(
    public readonly code: UacMicrophoneErrorCode,
    message: string,
  ) {
    super(message)
    this.name = 'UacMicrophoneError'
  }
}

export function microphoneSupportError(): UacMicrophoneErrorCode | null {
  if (!window.isSecureContext) return 'secure-context-required'
  if (!navigator.mediaDevices?.getUserMedia || typeof AudioEncoder === 'undefined' || typeof AudioData === 'undefined') {
    return 'unsupported'
  }
  return null
}

function buildAudioMessage(chunk: EncodedAudioChunk): Uint8Array {
  const message = new Uint8Array(15 + chunk.byteLength)
  const header = new DataView(message.buffer, 0, 15)
  header.setUint8(0, 0x03)
  header.setUint32(1, Math.round(chunk.timestamp / 1000), true)
  header.setUint16(5, Math.round((chunk.duration ?? 0) / 1000), true)
  header.setUint32(7, 0, true)
  header.setUint32(11, chunk.byteLength, true)
  chunk.copyTo(message.subarray(15))
  return message
}

function isUacTargetState(value: unknown): value is UacTargetState {
  return value === 'idle' || value === 'waiting' || value === 'active' || value === 'stalled'
}

function openSocket(
  url: string,
  onCreated: (socket: WebSocket) => void,
  onTargetState: (state: UacTargetState) => void,
): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(url)
    onCreated(socket)
    socket.binaryType = 'arraybuffer'
    socket.onmessage = event => {
      if (typeof event.data !== 'string') return
      try {
        const message = JSON.parse(event.data) as { type?: unknown, state?: unknown }
        if (message.type === 'uac_status' && isUacTargetState(message.state)) {
          onTargetState(message.state)
        }
      } catch {
        // Ignore control messages from incompatible server versions.
      }
    }
    let settled = false

    const timeout = window.setTimeout(() => {
      if (settled) return
      settled = true
      socket.close()
      reject(new UacMicrophoneError('connection-failed', 'WebSocket connection timed out'))
    }, SOCKET_OPEN_TIMEOUT_MS)

    socket.onopen = () => {
      if (settled) return
      settled = true
      window.clearTimeout(timeout)
      resolve(socket)
    }
    socket.onerror = () => {
      if (settled) return
      settled = true
      window.clearTimeout(timeout)
      reject(new UacMicrophoneError('connection-failed', 'WebSocket connection failed'))
    }
    socket.onclose = () => {
      if (settled) return
      settled = true
      window.clearTimeout(timeout)
      reject(new UacMicrophoneError('connection-failed', 'WebSocket was rejected'))
    }
  })
}

export function normalizeMicrophoneError(error: unknown): UacMicrophoneError {
  if (error instanceof UacMicrophoneError) return error
  if (error instanceof DOMException) {
    if (error.name === 'NotAllowedError' || error.name === 'SecurityError') {
      return new UacMicrophoneError('permission-denied', error.message)
    }
    if (error.name === 'NotFoundError' || error.name === 'OverconstrainedError' || error.name === 'NotReadableError') {
      return new UacMicrophoneError('device-unavailable', error.message)
    }
  }
  return new UacMicrophoneError(
    'unknown',
    error instanceof Error ? error.message : String(error),
  )
}

export class UacMicrophoneSession {
  private socket: WebSocket | null = null
  private stream: MediaStream | null = null
  private encoder: AudioEncoder | null = null
  private context: AudioContext | null = null
  private source: MediaStreamAudioSourceNode | null = null
  private processor: ScriptProcessorNode | null = null
  private sink: MediaStreamAudioDestinationNode | null = null
  private stopping = false
  private stopped = false
  private stopPromise: Promise<void> | null = null
  private encodedFrames = 0
  private inputFrames = 0

  constructor(
    private readonly endpoint: string,
    private readonly onUnexpectedEnd: (error: UacMicrophoneError) => void,
    private readonly onTargetState: (state: UacTargetState) => void,
    private readonly inputDeviceId = '',
  ) {}

  get activeInputDeviceId(): string {
    return this.stream?.getAudioTracks()[0]?.getSettings().deviceId ?? ''
  }

  async start(): Promise<void> {
    const supportError = microphoneSupportError()
    if (supportError) {
      throw new UacMicrophoneError(supportError, 'Required browser audio APIs are unavailable')
    }

    try {
      const support = await AudioEncoder.isConfigSupported(OPUS_CONFIG)
      if (!support.supported) {
        throw new UacMicrophoneError('unsupported', 'The browser does not support Opus encoding')
      }
      if (this.stopping) return

      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          ...(this.inputDeviceId ? { deviceId: { exact: this.inputDeviceId } } : {}),
          sampleRate: SAMPLE_RATE,
          channelCount: CHANNELS,
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        },
      })
      if (this.stopping) {
        stream.getTracks().forEach(track => track.stop())
        return
      }
      this.stream = stream

      const socket = await openSocket(this.endpoint, pendingSocket => {
        this.socket = pendingSocket
      }, this.onTargetState)
      if (this.stopping) {
        socket.close()
        return
      }
      socket.onclose = () => this.handleUnexpectedEnd('UAC audio connection closed')
      socket.onerror = () => this.handleUnexpectedEnd('UAC audio connection failed')

      this.encoder = new AudioEncoder({
        output: chunk => this.sendEncodedChunk(chunk),
        error: error => this.handleUnexpectedEnd(error.message, 'encoder-failed'),
      })
      this.encoder.configure(OPUS_CONFIG)

      this.context = new AudioContext({ sampleRate: SAMPLE_RATE })
      await this.context.resume()
      if (this.stopping) return
      this.source = this.context.createMediaStreamSource(this.stream)
      this.processor = this.context.createScriptProcessor(PROCESSOR_BUFFER_SIZE, CHANNELS, CHANNELS)
      this.sink = this.context.createMediaStreamDestination()
      this.processor.onaudioprocess = event => this.encodeInput(event.inputBuffer)
      this.source.connect(this.processor)
      // A silent MediaStream destination keeps ScriptProcessor active without
      // routing microphone input to the user's speakers.
      this.processor.connect(this.sink)
    } catch (error) {
      await this.stop()
      throw normalizeMicrophoneError(error)
    }
  }

  stop(): Promise<void> {
    if (this.stopPromise) return this.stopPromise
    this.stopPromise = this.stopResources()
    return this.stopPromise
  }

  private encodeInput(input: AudioBuffer) {
    if (this.stopping || this.encoder?.state !== 'configured') return
    if (this.encoder.encodeQueueSize > MAX_ENCODER_QUEUE_SIZE) return

    const frames = input.length
    const left = input.getChannelData(0)
    const right = input.numberOfChannels > 1 ? input.getChannelData(1) : left
    const pcm = new Int16Array(frames * CHANNELS)
    for (let i = 0; i < frames; i += 1) {
      pcm[i * CHANNELS] = Math.round(Math.max(-1, Math.min(1, left[i] ?? 0)) * 32767)
      pcm[i * CHANNELS + 1] = Math.round(Math.max(-1, Math.min(1, right[i] ?? 0)) * 32767)
    }

    const timestamp = Math.round(this.inputFrames * 1_000_000 / SAMPLE_RATE)
    this.inputFrames += frames
    const audioData = new AudioData({
      format: 's16',
      sampleRate: SAMPLE_RATE,
      numberOfFrames: frames,
      numberOfChannels: CHANNELS,
      timestamp,
      data: pcm,
    })
    try {
      this.encoder.encode(audioData)
    } finally {
      audioData.close()
    }
  }

  private sendEncodedChunk(chunk: EncodedAudioChunk) {
    const socket = this.socket
    if (this.stopping || socket?.readyState !== WebSocket.OPEN) return
    if (socket.bufferedAmount >= MAX_SOCKET_BUFFER_BYTES) return

    const message = buildAudioMessage(chunk)
    new DataView(message.buffer).setUint32(7, this.encodedFrames, true)
    this.encodedFrames = (this.encodedFrames + 1) >>> 0
    socket.send(message)
  }

  private handleUnexpectedEnd(message: string, code: UacMicrophoneErrorCode = 'connection-failed') {
    if (this.stopping || this.stopped) return
    void this.stop().finally(() => {
      this.onUnexpectedEnd(new UacMicrophoneError(code, message))
    })
  }

  private async stopResources() {
    this.stopping = true

    if (this.processor) {
      this.processor.onaudioprocess = null
      this.processor.disconnect()
      this.processor = null
    }
    this.source?.disconnect()
    this.source = null
    this.sink?.disconnect()
    this.sink = null

    this.stream?.getTracks().forEach(track => track.stop())
    this.stream = null

    const encoder = this.encoder
    this.encoder = null
    if (encoder && encoder.state !== 'closed') {
      try {
        if (encoder.state === 'configured') await encoder.flush()
      } catch {
        // The encoder may reject flush after a device or socket failure.
      }
      try {
        encoder.close()
      } catch {
        // An asynchronous encoder error may already have closed it.
      }
    }

    const socket = this.socket
    this.socket = null
    if (socket) {
      if (socket.readyState === WebSocket.CONNECTING) {
        // Keep the connection promise handlers installed so closing a pending
        // socket also settles start().
        socket.close()
      } else {
        socket.onclose = null
        socket.onerror = null
        socket.onmessage = null
        if (socket.readyState === WebSocket.OPEN) socket.close()
      }
    }

    const context = this.context
    this.context = null
    if (context && context.state !== 'closed') {
      try {
        await context.close()
      } catch {
        // A context that failed during startup may already be unusable.
      }
    }

    this.stopped = true
  }
}
