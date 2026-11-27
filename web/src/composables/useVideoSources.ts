import { onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { videoSourcesApi, type VideoSource } from '@/api/videoSources'
import { ApiError } from '@/api/request'
import { useAuthStore } from '@/stores/auth'
import { useWebSocket } from '@/composables/useWebSocket'

// One collection and selection for Console and the video configuration popover.
const sources = ref<VideoSource[]>([])
const primarySourceId = ref('')
const selectionKey = `nativeVideoSource:${location.host}`
const activeSourceId = ref(localStorage.getItem(selectionKey) || '')
watch(activeSourceId, id => { try { localStorage.setItem(selectionKey, id) } catch { /* Optional preference. */ } })
const discoveryError = ref('')
const authExpired = ref(false)
let consumers = 0
let controller: AbortController | undefined
let pending: Promise<void> | undefined
let refreshQueued = false
let timer: ReturnType<typeof setTimeout> | undefined
const changed = () => { void refreshSources() }

export function expireVideoSourcesAuth() {
  authExpired.value = true
  controller?.abort()
  clearTimeout(timer)
  const auth = useAuthStore()
  auth.isAuthenticated = false
  auth.user = null
}

async function refreshSources() {
  if (pending) { refreshQueued = true; return pending }
  if (authExpired.value || !useAuthStore().isAuthenticated) return
  pending = (async () => {
    do {
      refreshQueued = false
      const current = new AbortController()
      controller = current
      const timeout = setTimeout(() => current.abort(), 5000)
      try {
        const result = await videoSourcesApi.list(current.signal)
        if (current.signal.aborted || authExpired.value || !useAuthStore().isAuthenticated) return
        sources.value = result.sources
        primarySourceId.value = result.primary_source_id
          || result.sources.find(source => source.primary)?.id || result.sources[0]?.id || ''
        if (!result.sources.some(source => source.id === activeSourceId.value && source.enabled)) {
          activeSourceId.value = result.sources.find(source => source.id === primarySourceId.value && source.enabled)?.id
            || result.sources.find(source => source.enabled)?.id || ''
        }
        discoveryError.value = ''
      } catch (error) {
        if (error instanceof ApiError && error.status === 401) expireVideoSourcesAuth()
        else if (!current.signal.aborted) discoveryError.value = error instanceof Error ? error.message : String(error)
      } finally {
        clearTimeout(timeout)
      }
    } while (refreshQueued && consumers > 0 && !authExpired.value && useAuthStore().isAuthenticated)
  })().finally(() => { pending = undefined })
  return pending
}

function scheduleRefresh() {
  clearTimeout(timer)
  if (!consumers || authExpired.value || !useAuthStore().isAuthenticated) return
  timer = setTimeout(async () => {
    await refreshSources()
    scheduleRefresh()
  }, 10000)
}

export function useVideoSources() {
  const ws = useWebSocket()
  const auth = useAuthStore()
  let active = false
  function start() {
    if (active) return
    active = true
    consumers++
    if (auth.isAuthenticated) authExpired.value = false
    if (consumers === 1) {
      window.addEventListener('videoSourcesChanged', changed)
      ws.on('video.sources_changed', changed)
      void refreshSources()
      scheduleRefresh()
    }
  }
  function stop() {
    if (!active) return
    active = false
    if (--consumers === 0) {
      clearTimeout(timer)
      controller?.abort()
      window.removeEventListener('videoSourcesChanged', changed)
      ws.off('video.sources_changed', changed)
    }
  }
  onMounted(start)
  onActivated(start)
  onDeactivated(stop)
  onUnmounted(stop)
  watch(() => auth.isAuthenticated, authenticated => {
    if (!authenticated) { controller?.abort(); clearTimeout(timer) }
    else if (active) { authExpired.value = false; changed(); scheduleRefresh() }
  })
  return { sources, primarySourceId, activeSourceId, refreshSources, discoveryError, authExpired }
}
