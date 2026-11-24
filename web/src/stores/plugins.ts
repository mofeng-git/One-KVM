import { ref } from 'vue'
import { defineStore } from 'pinia'
import { pluginsApi, type Plugin } from '@/api/plugins'

export const usePluginStore = defineStore('plugins', () => {
  const installed = ref<Plugin[]>([])
  const loaded = ref(false)
  const failed = ref(false)
  let revision = 0

  async function refresh() {
    const request = ++revision
    try {
      const plugins = await pluginsApi.list()
      if (request === revision) {
        installed.value = plugins
        loaded.value = true
        failed.value = false
      }
    } catch (error) {
      if (request === revision) failed.value = true
      throw error
    }
  }

  function upsert(plugin: Plugin) {
    revision++
    const index = installed.value.findIndex(item => item.manifest.id === plugin.manifest.id)
    if (index < 0) installed.value.push(plugin)
    else installed.value[index] = plugin
  }

  function remove(id: string) {
    revision++
    installed.value = installed.value.filter(plugin => plugin.manifest.id !== id)
  }

  return { installed, loaded, failed, refresh, upsert, remove }
})
