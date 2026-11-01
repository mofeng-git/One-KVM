import { ref } from 'vue'

export interface AppearanceSettings {
  loading_style?: string | null
  loading_background?: string | null
  no_signal_image?: string | null
}

const settings = ref<AppearanceSettings>({})
let loaded = false

/** 外观设置（设备端保存，所有浏览器共享） */
export function useAppearance() {
  async function load(force = false) {
    if (loaded && !force) return
    try {
      const res = await fetch('/api/appearance', { credentials: 'include' })
      if (res.ok) {
        settings.value = await res.json()
        loaded = true
      }
    } catch {
      // 忽略：拿不到就用默认外观
    }
  }

  return { settings, load }
}
