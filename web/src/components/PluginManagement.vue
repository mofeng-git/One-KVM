<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown, FileArchive, Upload } from 'lucide-vue-next'
import { pluginsApi, type Plugin } from '@/api/plugins'
import { usePluginStore } from '@/stores/plugins'
import { pluginText } from '@/lib/pluginNavigation'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

const { t, locale } = useI18n()
const store = usePluginStore()
const uninstalling = ref(false)
const busy = ref(false)
const showUrl = ref(false)
const url = ref('')
const upload = ref<File | null>(null)
const input = ref<HTMLInputElement>()

async function uninstall(plugin: Plugin) {
  const { id, name } = plugin.manifest
  if (!window.confirm(t('plugins.confirmRemove', { name: pluginText(name, locale.value) }))) return
  uninstalling.value = true
  try {
    await pluginsApi.uninstall(id)
    store.remove(id)
  } catch {
    // The request client displays the API error.
  } finally {
    uninstalling.value = false
  }
}

async function install(method: 'file' | 'url') {
  busy.value = true
  try {
    if (method === 'file' && upload.value) {
      store.upsert(await pluginsApi.upload(upload.value))
      upload.value = null
      if (input.value) input.value.value = ''
    } else if (method === 'url' && url.value.trim()) {
      store.upsert(await pluginsApi.install(url.value.trim()))
      url.value = ''
    }
    await store.refresh()
  } catch {
    // The request client displays the API error.
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Card>
    <CardHeader class="settings-card-heading">
      <CardTitle>{{ t('plugins.install') }}</CardTitle>
    </CardHeader>
    <CardContent class="space-y-4">
      <form @submit.prevent="install('file')">
        <input ref="input" type="file" accept=".onepkg" hidden :disabled="busy"
          @change="upload = ($event.target as HTMLInputElement).files?.[0] ?? null" />
        <div v-if="!upload" class="flex flex-wrap items-center gap-x-3 gap-y-2">
          <Button type="button" :disabled="busy" @click="input?.click()">
            <Upload aria-hidden="true" />{{ t('plugins.choosePackage') }}
          </Button>
          <span class="text-sm text-muted-foreground">{{ t('plugins.packageFormat') }}</span>
        </div>
        <div v-else class="flex flex-wrap items-center gap-2 rounded-md border bg-muted/30 p-2.5">
          <div class="flex min-w-36 flex-1 items-center gap-2 px-1 text-sm" role="status">
            <FileArchive class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            <span class="truncate" :title="upload.name">{{ upload.name }}</span>
          </div>
          <Button type="button" variant="outline" size="sm" :disabled="busy" @click="input?.click()">
            {{ t('plugins.changePackage') }}
          </Button>
          <Button type="submit" size="sm" :disabled="busy">{{ t('plugins.installPackage') }}</Button>
        </div>
      </form>
      <div class="space-y-2 border-t pt-3">
        <Button type="button" variant="ghost" size="sm" class="-ml-2" :aria-expanded="showUrl"
          aria-controls="plugin-install-url" :disabled="busy" @click="showUrl = !showUrl">
          {{ t('plugins.installUrl') }}
          <ChevronDown :class="{ 'rotate-180': showUrl }" aria-hidden="true" />
        </Button>
        <form v-show="showUrl" id="plugin-install-url" class="flex flex-wrap items-center gap-2"
          @submit.prevent="install('url')">
          <Input v-model="url" type="url" :aria-label="t('plugins.packageUrl')"
            :placeholder="t('plugins.packageUrl')" class="min-w-0 flex-1" :disabled="busy" required />
          <Button type="submit" variant="outline" :disabled="busy || !url.trim()">
            {{ t('plugins.installPackage') }}
          </Button>
        </form>
      </div>
      <p class="text-sm text-muted-foreground">{{ t('plugins.trustedPackages') }}</p>
    </CardContent>
  </Card>
  <p v-if="store.failed" class="text-sm text-destructive" role="status">
    {{ t('plugins.loadFailed') }}
  </p>
  <p v-else-if="store.loaded && !store.installed.length" class="text-sm text-muted-foreground">
    {{ t('plugins.empty') }}
  </p>
  <Card v-if="store.installed.length">
    <CardHeader class="settings-card-heading">
      <CardTitle>{{ t('plugins.installed') }}</CardTitle>
    </CardHeader>
    <CardContent>
      <ul class="divide-y">
        <li v-for="plugin in store.installed" :key="plugin.manifest.id"
          class="flex flex-wrap items-center justify-between gap-3 py-4 first:pt-0 last:pb-0">
          <div class="min-w-0 space-y-1">
            <p class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
              <span class="font-medium">{{ pluginText(plugin.manifest.name, locale) }}</span>
              <span class="text-xs text-muted-foreground">{{ plugin.manifest.version }}</span>
              <span class="break-words text-xs text-muted-foreground">{{ plugin.manifest.author }}</span>
            </p>
            <p v-if="pluginText(plugin.manifest.description, locale)" class="text-sm text-muted-foreground">
              {{ pluginText(plugin.manifest.description, locale) }}
            </p>
          </div>
          <Button type="button" variant="outline" size="sm" class="text-destructive"
            :disabled="uninstalling" @click="uninstall(plugin)">{{ t('plugins.uninstall') }}</Button>
        </li>
      </ul>
    </CardContent>
  </Card>
</template>
