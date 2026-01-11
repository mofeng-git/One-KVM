import { ref } from 'vue'

export interface StreamModeSwitchingEvent {
  transition_id: string
  to_mode: string
  from_mode: string
}

export interface StreamModeReadyEvent {
  transition_id: string
  mode: string
}

export interface WebRTCReadyEvent {
  codec: string
  hardware: boolean
  transition_id?: string
}

let singleton: ReturnType<typeof createVideoSession> | null = null

function createVideoSession() {
  const localSwitching = ref(false)
  const backendSwitching = ref(false)
  const activeTransitionId = ref<string | null>(null)
  const expectedTransitionId = ref<string | null>(null)

  let lastUserSwitchAt = 0

  let webrtcReadyWaiter: {
    transitionId: string
    resolve: (ready: boolean) => void
    timer: ReturnType<typeof setTimeout>
  } | null = null

  let modeReadyWaiter: {
    transitionId: string
    resolve: (mode: string | null) => void
    timer: ReturnType<typeof setTimeout>
  } | null = null

  function startLocalSwitch() {
    localSwitching.value = true
  }

  function tryStartLocalSwitch(minIntervalMs = 800): boolean {
    const now = Date.now()
    if (localSwitching.value) return false
    if (now - lastUserSwitchAt < minIntervalMs) return false
    lastUserSwitchAt = now
    localSwitching.value = true
    return true
  }

  function endLocalSwitch() {
    localSwitching.value = false
  }

  function clearWaiters() {
    if (webrtcReadyWaiter) {
      clearTimeout(webrtcReadyWaiter.timer)
      webrtcReadyWaiter.resolve(false)
      webrtcReadyWaiter = null
    }
    if (modeReadyWaiter) {
      clearTimeout(modeReadyWaiter.timer)
      modeReadyWaiter.resolve(null)
      modeReadyWaiter = null
    }
    expectedTransitionId.value = null
  }

  function registerTransition(transitionId: string) {
    expectedTransitionId.value = transitionId
    activeTransitionId.value = transitionId
    backendSwitching.value = true
  }

  function isStaleTransition(transitionId?: string): boolean {
    if (!transitionId) return false
    return expectedTransitionId.value !== null && transitionId !== expectedTransitionId.value
  }

  function waitForWebRTCReady(transitionId: string, timeoutMs = 3000): Promise<boolean> {
    if (webrtcReadyWaiter) {
      clearTimeout(webrtcReadyWaiter.timer)
      webrtcReadyWaiter.resolve(false)
      webrtcReadyWaiter = null
    }

    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        if (webrtcReadyWaiter?.transitionId === transitionId) {
          webrtcReadyWaiter = null
        }
        resolve(false)
      }, timeoutMs)

      webrtcReadyWaiter = {
        transitionId,
        resolve,
        timer,
      }
    })
  }

  function waitForModeReady(transitionId: string, timeoutMs = 5000): Promise<string | null> {
    if (modeReadyWaiter) {
      clearTimeout(modeReadyWaiter.timer)
      modeReadyWaiter.resolve(null)
      modeReadyWaiter = null
    }

    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        if (modeReadyWaiter?.transitionId === transitionId) {
          modeReadyWaiter = null
        }
        resolve(null)
      }, timeoutMs)

      modeReadyWaiter = {
        transitionId,
        resolve,
        timer,
      }
    })
  }

  function onModeSwitching(data: StreamModeSwitchingEvent) {
    if (localSwitching.value && expectedTransitionId.value && data.transition_id !== expectedTransitionId.value) {
      return
    }
    backendSwitching.value = true
    activeTransitionId.value = data.transition_id
    expectedTransitionId.value = data.transition_id
  }

  function onModeReady(data: StreamModeReadyEvent) {
    if (isStaleTransition(data.transition_id)) return

    backendSwitching.value = false
    activeTransitionId.value = null
    expectedTransitionId.value = null

    if (modeReadyWaiter?.transitionId === data.transition_id) {
      clearTimeout(modeReadyWaiter.timer)
      modeReadyWaiter.resolve(data.mode)
      modeReadyWaiter = null
    }
  }

  function onWebRTCReady(data: WebRTCReadyEvent) {
    if (isStaleTransition(data.transition_id)) return
    if (data.transition_id && webrtcReadyWaiter?.transitionId === data.transition_id) {
      clearTimeout(webrtcReadyWaiter.timer)
      webrtcReadyWaiter.resolve(true)
      webrtcReadyWaiter = null
    }
  }

  return {
    localSwitching,
    backendSwitching,
    activeTransitionId,
    expectedTransitionId,
    startLocalSwitch,
    tryStartLocalSwitch,
    endLocalSwitch,
    clearWaiters,
    registerTransition,
    waitForWebRTCReady,
    waitForModeReady,
    onModeSwitching,
    onModeReady,
    onWebRTCReady,
  }
}

export function useVideoSession(): ReturnType<typeof createVideoSession> {
  if (!singleton) {
    singleton = createVideoSession()
  }
  return singleton
}
