import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type ComputedRef,
  type Ref,
} from 'vue'
import { configApi, type VideoDevice, type VideoInputStatus, type VideoResolution } from '@/api'
import { useWebSocket } from '@/composables/useWebSocket'

interface VideoSelection {
  device: Ref<string>
  format: Ref<string>
  resolution: Ref<string>
  fps: Ref<number | null>
}

interface Options {
  devices: Ref<VideoDevice[]>
  selection: VideoSelection
  active: ComputedRef<boolean> | Ref<boolean>
  listenForStreamEvents?: boolean
  preferredFormat?: (device: VideoDevice) => string | undefined
}

export function useVideoDeviceConfiguration(options: Options) {
  const selectedDevice = computed(() =>
    options.devices.value.find(device => device.path === options.selection.device.value),
  )
  const isSourceFollowing = computed(() =>
    selectedDevice.value?.control_mode === 'source_following',
  )
  const inputStatus = computed(() => selectedDevice.value?.input_status ?? null)
  const availableFormats = computed(() => selectedDevice.value?.formats ?? [])
  const availableResolutions = computed(() => {
    const resolutions = availableFormats.value.find(
      format => format.format === options.selection.format.value,
    )?.resolutions ?? []
    const merged = new Map<string, VideoResolution>()
    for (const resolution of resolutions) {
      const key = `${resolution.width}x${resolution.height}`
      const current = merged.get(key)
      if (!current) {
        merged.set(key, { ...resolution, fps: [...resolution.fps] })
      } else {
        current.fps = [...new Set([...current.fps, ...resolution.fps])].sort((a, b) => b - a)
      }
    }
    return [...merged.values()].sort((a, b) => b.width * b.height - a.width * a.height)
  })
  const availableFps = computed(() => {
    const resolution = availableResolutions.value.find(
      item => `${item.width}x${item.height}` === options.selection.resolution.value,
    )
    return resolution?.fps ?? []
  })

  let requestGeneration = 0
  let pollTimer: ReturnType<typeof setInterval> | null = null
  const refreshingInputStatus = ref(false)

  function replaceInputStatus(path: string, status: VideoInputStatus) {
    const device = options.devices.value.find(item => item.path === path)
    if (!device) return
    device.input_status = status
    device.has_signal = status.state === 'locked'
  }

  async function refreshInputStatus() {
    const path = options.selection.device.value
    if (!path || !isSourceFollowing.value || refreshingInputStatus.value) return
    const generation = ++requestGeneration
    refreshingInputStatus.value = true
    try {
      const status = await configApi.getVideoInputStatus(path)
      if (generation !== requestGeneration || path !== options.selection.device.value) return
      replaceInputStatus(path, status)
    } catch {
      if (generation !== requestGeneration || path !== options.selection.device.value) return
      replaceInputStatus(path, {
        state: 'unavailable',
        format: null,
        width: null,
        height: null,
        fps: null,
      })
    } finally {
      if (generation === requestGeneration) refreshingInputStatus.value = false
    }
  }

  function stopPolling() {
    requestGeneration++
    refreshingInputStatus.value = false
    if (pollTimer) clearInterval(pollTimer)
    pollTimer = null
  }

  function syncPolling() {
    stopPolling()
    if (!options.active.value || document.hidden || !isSourceFollowing.value) return
    void refreshInputStatus()
    pollTimer = setInterval(() => void refreshInputStatus(), 2_000)
  }

  function chooseResolution() {
    if (isSourceFollowing.value) return
    const current = options.selection.resolution.value
    if (availableResolutions.value.some(item => `${item.width}x${item.height}` === current)) return
    const preferred = availableResolutions.value.find(item => item.width === 1920 && item.height === 1080)
      ?? availableResolutions.value.find(item => item.width === 1280 && item.height === 720)
      ?? availableResolutions.value[0]
    options.selection.resolution.value = preferred ? `${preferred.width}x${preferred.height}` : ''
  }

  function chooseFps() {
    if (isSourceFollowing.value) return
    const current = options.selection.fps.value
    if (current !== null && availableFps.value.includes(current)) return
    options.selection.fps.value = availableFps.value.includes(30) ? 30 : availableFps.value[0] ?? null
  }

  watch(() => options.selection.device.value, () => {
    requestGeneration++
    if (isSourceFollowing.value) {
      options.selection.format.value = ''
      options.selection.resolution.value = ''
      options.selection.fps.value = null
    } else if (selectedDevice.value) {
      const valid = availableFormats.value.some(item => item.format === options.selection.format.value)
      if (!valid) {
        options.selection.format.value = options.preferredFormat?.(selectedDevice.value)
          ?? availableFormats.value[0]?.format
          ?? ''
      }
    }
    syncPolling()
  })
  watch(() => options.selection.format.value, chooseResolution)
  watch(() => options.selection.resolution.value, chooseFps)
  watch([() => options.active.value, isSourceFollowing], syncPolling)

  const { on, off, connect } = useWebSocket()
  const refreshFromStreamEvent = () => {
    if (options.active.value && isSourceFollowing.value) void refreshInputStatus()
  }
  const streamEvents = ['stream.config_applied', 'stream.state_changed', 'stream.recovered']

  function handleVisibilityChange() {
    syncPolling()
  }

  onMounted(() => {
    document.addEventListener('visibilitychange', handleVisibilityChange)
    if (options.listenForStreamEvents) {
      for (const event of streamEvents) on(event, refreshFromStreamEvent)
      connect()
    }
    syncPolling()
  })

  onBeforeUnmount(() => {
    stopPolling()
    document.removeEventListener('visibilitychange', handleVisibilityChange)
    if (options.listenForStreamEvents) {
      for (const event of streamEvents) off(event, refreshFromStreamEvent)
    }
  })

  return {
    selectedDevice,
    isSourceFollowing,
    inputStatus,
    availableFormats,
    availableResolutions,
    availableFps,
    refreshInputStatus,
    refreshingInputStatus,
  }
}
