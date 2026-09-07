import { ref, watch, onMounted, onUnmounted, onDeactivated, onActivated, type Ref } from 'vue'
import type { hidApi } from '@/api'
import { request } from '@/api/request'
import type { BluetoothStatus } from '@/types/bluetooth'
export function useHidConnection(active: Ref<boolean>, backend: Ref<string | undefined>) {
  const status = ref<Awaited<ReturnType<typeof hidApi.status>> | null>(null)
  const bluetooth = ref<BluetoothStatus | null>(null)
  const error = ref('')
  let timer: ReturnType<typeof setTimeout> | undefined
  let controller: AbortController | undefined
  let generation = 0, inFlight = false, mounted = false, deactivated = false
  function stop() { generation++; clearTimeout(timer); controller?.abort(); status.value = null; bluetooth.value = null }
  async function refresh() {
    if (!mounted || deactivated || !active.value || document.hidden || inFlight) return
    inFlight = true
    const own = generation
    controller = new AbortController()
    const signal = controller.signal
    const timeout = setTimeout(() => controller?.abort(), 5000)
    try {
      const [hidResult, btResult] = await Promise.allSettled([
        request<Awaited<ReturnType<typeof hidApi.status>>>('/hid/status', { signal }, { toastOnError: false }),
        backend.value === 'bluetooth' ? request<BluetoothStatus>('/hid/bluetooth', { signal }, { toastOnError: false }) : Promise.resolve(null),
      ])
      if (own !== generation) return
      if (hidResult.status === 'rejected') throw hidResult.reason
      if (btResult.status === 'rejected') throw btResult.reason
      const hid = hidResult.value, bt = btResult.value
      status.value = hid.backend === backend.value ? hid : null
      bluetooth.value = hid.backend === backend.value ? bt : null
      error.value = ''
    } catch (e) {
      if (own !== generation) return
      status.value = null; bluetooth.value = null
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      clearTimeout(timeout)
      inFlight = false
      if (mounted && !deactivated && active.value && !document.hidden) timer = setTimeout(refresh, 2000)
    }
  }
  function restart() { stop(); error.value = ''; void refresh() }
  watch([active, backend], restart)
  onMounted(() => { mounted = true; document.addEventListener('visibilitychange', restart); restart() })
  onActivated(() => { deactivated = false; restart() })
  onDeactivated(() => { deactivated = true; stop() })
  onUnmounted(() => { mounted = false; stop(); document.removeEventListener('visibilitychange', restart) })
  return { status, bluetooth, error, restart }
}
