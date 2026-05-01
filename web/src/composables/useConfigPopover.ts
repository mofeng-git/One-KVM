
import { ref, watch, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

export interface UseConfigPopoverOptions {
  /** Reactive open state from props */
  open: Ref<boolean>
  /** Load device list callback */
  loadDevices?: () => Promise<void>
  /** Initialize from current config callback */
  initializeFromCurrent?: () => void
}

export function useConfigPopover(options: UseConfigPopoverOptions) {
  const { t } = useI18n()

  const applying = ref(false)
  const loadingDevices = ref(false)

  watch(options.open, async (isOpen) => {
    if (isOpen) {
      options.initializeFromCurrent?.()
      if (options.loadDevices) {
        loadingDevices.value = true
        try {
          await options.loadDevices()
        } finally {
          loadingDevices.value = false
        }
      }
    }
  })

  async function applyConfig(applyFn: () => Promise<void>) {
    applying.value = true
    try {
      await applyFn()
      toast.success(t('config.applied'))
    } catch (e) {
      console.info('[ConfigPopover] Apply failed:', e)
    } finally {
      applying.value = false
    }
  }

  // Refresh devices
  async function refreshDevices() {
    if (!options.loadDevices) return
    loadingDevices.value = true
    try {
      await options.loadDevices()
    } finally {
      loadingDevices.value = false
    }
  }

  return {
    applying,
    loadingDevices,

    applyConfig,
    refreshDevices,
  }
}
