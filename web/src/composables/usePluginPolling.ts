import { onMounted, onUnmounted } from 'vue'
import { usePluginStore } from '@/stores/plugins'

export function usePluginPolling() {
  const store = usePluginStore()
  let timer: ReturnType<typeof setInterval> | undefined

  function refresh() {
    void store.refresh().catch(() => {})
  }

  onMounted(() => {
    refresh()
    timer = setInterval(refresh, 2000)
  })
  onUnmounted(() => clearInterval(timer))
  return store
}
