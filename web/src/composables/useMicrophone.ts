import { computed, ref } from 'vue'
import {
  microphoneSupportError,
  normalizeMicrophoneError,
  UacMicrophoneSession,
  type UacMicrophoneErrorCode,
  type UacTargetState,
} from '@/lib/uac-microphone'

export type MicrophoneTransferState = 'idle' | 'starting' | 'streaming' | 'stopping' | 'error'

export interface MicrophoneInputDevice {
  deviceId: string
  label: string
}

const WS_ENDPOINT = `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/api/ws/uac-audio`

export function useMicrophone() {
  const state = ref<MicrophoneTransferState>('idle')
  const targetState = ref<UacTargetState>('idle')
  const errorCode = ref<UacMicrophoneErrorCode | null>(microphoneSupportError())
  const inputDevices = ref<MicrophoneInputDevice[]>([])
  const selectedDeviceId = ref('')
  const permissionGranted = ref(false)
  const loadingDevices = ref(false)
  let session: UacMicrophoneSession | null = null

  const active = computed(() => state.value === 'streaming')
  const busy = computed(() => state.value === 'starting' || state.value === 'stopping')

  async function refreshInputDevices(requestPermission = false) {
    const supportError = microphoneSupportError()
    if (supportError) {
      errorCode.value = supportError
      return false
    }

    loadingDevices.value = true
    let permissionStream: MediaStream | null = null
    try {
      if (requestPermission) {
        permissionStream = await navigator.mediaDevices.getUserMedia({ audio: true })
        permissionGranted.value = true
        if (!selectedDeviceId.value) {
          selectedDeviceId.value = permissionStream.getAudioTracks()[0]?.getSettings().deviceId ?? ''
        }
      }

      const availableDevices = (await navigator.mediaDevices.enumerateDevices())
        .filter(device => device.kind === 'audioinput' && device.deviceId)
      permissionGranted.value = requestPermission
        || active.value
        || availableDevices.some(device => device.label)
      const devices = availableDevices
        .map((device, index) => ({
          deviceId: device.deviceId,
          label: device.label || `Microphone ${index + 1}`,
        }))

      inputDevices.value = devices
      if (!devices.some(device => device.deviceId === selectedDeviceId.value)) {
        selectedDeviceId.value = devices[0]?.deviceId ?? ''
      }
      errorCode.value = null
      return true
    } catch (error) {
      const microphoneError = normalizeMicrophoneError(error)
      errorCode.value = microphoneError.code
      return false
    } finally {
      permissionStream?.getTracks().forEach(track => track.stop())
      loadingDevices.value = false
    }
  }

  async function start() {
    if (busy.value || active.value) return
    const supportError = microphoneSupportError()
    if (supportError) {
      errorCode.value = supportError
      state.value = 'error'
      return
    }

    state.value = 'starting'
    targetState.value = 'waiting'
    errorCode.value = null

    const nextSession = new UacMicrophoneSession(
      WS_ENDPOINT,
      error => {
        if (session !== nextSession) return
        session = null
        targetState.value = 'idle'
        errorCode.value = error.code
        state.value = 'error'
      },
      nextState => {
        if (session !== nextSession) return
        targetState.value = nextState
      },
      selectedDeviceId.value,
    )
    session = nextSession

    try {
      await nextSession.start()
      if (session !== nextSession) {
        await nextSession.stop()
        return
      }
      permissionGranted.value = true
      selectedDeviceId.value = nextSession.activeInputDeviceId || selectedDeviceId.value
      state.value = 'streaming'
      void refreshInputDevices()
    } catch (error) {
      if (session !== nextSession) return
      session = null
      targetState.value = 'idle'
      const microphoneError = normalizeMicrophoneError(error)
      errorCode.value = microphoneError.code
      state.value = 'error'
    }
  }

  async function stop() {
    if (state.value === 'idle' || state.value === 'stopping') return
    state.value = 'stopping'
    const current = session
    session = null
    await current?.stop()
    targetState.value = 'idle'
    errorCode.value = microphoneSupportError()
    state.value = 'idle'
  }

  async function toggle() {
    if (active.value || state.value === 'starting') await stop()
    else await start()
  }

  async function selectInputDevice(deviceId: string) {
    if (deviceId === selectedDeviceId.value) return
    const wasActive = active.value
    if (wasActive) await stop()
    selectedDeviceId.value = deviceId
    if (wasActive) await start()
  }

  return {
    state,
    targetState,
    active,
    busy,
    errorCode,
    inputDevices,
    selectedDeviceId,
    permissionGranted,
    loadingDevices,
    refreshInputDevices,
    selectInputDevice,
    start,
    stop,
    toggle,
  }
}

let instance: ReturnType<typeof useMicrophone> | null = null

export function getMicrophone() {
  if (!instance) instance = useMicrophone()
  return instance
}
