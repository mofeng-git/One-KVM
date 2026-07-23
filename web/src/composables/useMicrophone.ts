import { ref } from 'vue'

let instance: ReturnType<typeof useMicrophone> | null = null
export function getMicrophone() {
  if (!instance) instance = useMicrophone()
  return instance
}

const WS_BASE = `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/api/ws/uac-audio`

// Opus: 48kHz stereo, 64kbps → ~8 KB/s vs raw PCM 192 KB/s (24× reduction)
const OPUS_CONFIG: AudioEncoderConfig = {
  codec: 'opus',
  sampleRate: 48000,
  numberOfChannels: 2,
  bitrate: 64000,
}

// 15-byte binary header matching server-side UAC_AUDIO_HEADER_SIZE
function buildHeader(msgType: number, durationMs: number, dataLen: number): Uint8Array {
  const h = new Uint8Array(15)
  const v = new DataView(h.buffer)
  v.setUint8(0, msgType)       // 0x03 = Opus
  v.setUint32(1, 0, true)      // timestamp (unused)
  v.setUint16(5, durationMs, true)
  v.setUint32(7, 0, true)      // sequence
  v.setUint32(11, dataLen, true)
  return h
}

export function useMicrophone() {
  const active = ref(false)
  const error = ref<string | null>(null)
  let ws: WebSocket | null = null
  let stream: MediaStream | null = null
  let encoder: AudioEncoder | null = null
  let running = false

  let frameCount = 0
  let byteCount = 0

  // ── AudioEncoder helper ─────────────────────────────────
  function createEncoder(onOpusFrame: (data: Uint8Array, durMs: number) => void): AudioEncoder {
    const enc = new AudioEncoder({
      output: (chunk: EncodedAudioChunk) => {
        const buf = new Uint8Array(chunk.byteLength)
        chunk.copyTo(buf)
        // Opus frame duration in microseconds → milliseconds
        const durMs = Math.round(chunk.duration! / 1000)
        onOpusFrame(buf, durMs)
      },
      error: (e: Error) => console.error('[mic] encoder error:', e),
    })
    enc.configure(OPUS_CONFIG)
    return enc
  }

  // ── start / stop ────────────────────────────────────────
  async function start() {
    error.value = null
    frameCount = 0
    byteCount = 0
    running = true
    console.log('[mic] starting...')

    try {
      // WebSocket
      ws = new WebSocket(WS_BASE)
      ws.binaryType = 'arraybuffer'
      const wsReady = new Promise<void>((resolve, reject) => {
        ws!.onopen = () => { console.log('[mic] WS opened'); active.value = true; resolve() }
        ws!.onerror = (ev) => { console.error('[mic] WS error:', ev); reject(new Error('WebSocket failed')) }
      })
      ws.onclose = (ev) => {
        console.log('[mic] WS closed: code=%d frames=%d bytes=%d', ev.code, frameCount, byteCount)
        active.value = false
        running = false
      }

      // Microphone
      console.log('[mic] getUserMedia...')
      stream = await navigator.mediaDevices.getUserMedia({
        audio: { sampleRate: 48000, channelCount: 2, echoCancellation: false, noiseSuppression: false }
      })
      // AudioEncoder (WebCodecs) for Opus compression
      encoder = createEncoder((opusData, durMs) => {
        if (!ws || ws.readyState !== WebSocket.OPEN) return
        const header = buildHeader(0x03, durMs, opusData.length)
        const msg = new Uint8Array(15 + opusData.length)
        msg.set(header)
        msg.set(opusData, 15)
        ws.send(msg)
        frameCount++
        byteCount += msg.byteLength
        if (frameCount % 50 === 0) {
          console.debug('[mic] frame #%d: opus=%dB dur=%dms',
            frameCount, opusData.length, durMs)
        }
      })

      await wsReady

      // ScriptProcessor → S16LE PCM → AudioData → AudioEncoder → Opus
      const audioCtx = new AudioContext({ sampleRate: 48000 })
      const source = audioCtx.createMediaStreamSource(stream)
      const processor = audioCtx.createScriptProcessor(4096, 2, 2)
      source.connect(processor)
      processor.connect(audioCtx.destination)

      processor.onaudioprocess = (e: AudioProcessingEvent) => {
        if (!running || !encoder || encoder.state !== 'configured') return
        if (!e.inputBuffer) return
        const buf = e.inputBuffer as any
        const left = buf.getChannelData(0) as Float32Array
        const right = buf.getChannelData(1) as Float32Array
        const samples = left.length

        // Float32 → S16LE interleaved
        const pcm = new Int16Array(samples * 2)
        for (let i = 0; i < samples; i++) {
          pcm[i * 2]     = Math.max(-32768, Math.min(32767, Math.round((left[i] ?? 0) * 32767)))
          pcm[i * 2 + 1] = Math.max(-32768, Math.min(32767, Math.round((right[i] ?? 0) * 32767)))
        }

        try {
          const audioData = new AudioData({
            format: 's16',
            sampleRate: 48000,
            numberOfFrames: samples,
            numberOfChannels: 2,
            timestamp: 0,
            data: pcm.buffer,
          })
          encoder.encode(audioData)
          audioData.close()
        } catch (e) {
          console.warn('[mic] AudioData/encode error:', e)
        }
      }
    } catch (e) {
      console.error('[mic] start error:', e)
      error.value = e instanceof Error ? e.message : 'Failed to start microphone'
      stop()
    }
  }

  function stop() {
    console.log('[mic] stop: frames=%d bytes=%d', frameCount, byteCount)
    running = false
    if (encoder) { encoder.close(); encoder = null }
    if (stream) { stream.getTracks().forEach(t => t.stop()); stream = null }
    if (ws) { ws.close(); ws = null }
    active.value = false
  }

  function toggle() {
    if (active.value) { stop() } else { start() }
  }

  return { active, error, start, stop, toggle }
}
