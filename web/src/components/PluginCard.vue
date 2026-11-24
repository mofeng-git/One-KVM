<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight, Play, RefreshCw, Save, Square } from 'lucide-vue-next'
import { pluginsApi, type ConfigValue, type Plugin, type PluginField, type PluginStatusField } from '@/api/plugins'
import { usePluginStore } from '@/stores/plugins'
import { pluginText } from '@/lib/pluginNavigation'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import CardTitle from '@/components/SettingsCardTitle.vue'

const props = defineProps<{ plugin: Plugin; compact?: boolean }>()
const { t, locale } = useI18n()
const formId = useId()
const form = ref<HTMLFormElement>()
const store = usePluginStore()
const draft = ref<Record<string, ConfigValue>>({})
const autostart = ref(false)
const busy = ref(false)
const logsOpen = ref(false)
const logs = ref<string[] | null>(null)
const revealed = ref<Record<string, boolean>>({})
const id = computed(() => props.plugin.manifest.id)
const snapshot = computed(() => JSON.stringify(props.plugin.config))

watch(snapshot, () => {
  draft.value = { ...props.plugin.config }
}, { immediate: true })
watch(() => props.plugin.autostart, value => { autostart.value = value }, { immediate: true })

const fields = computed(() => props.plugin.manifest.config_schema.filter(field =>
  !field.when || JSON.stringify(draft.value[field.when.key]) === JSON.stringify(field.when.equals),
))
const dirty = computed(() => autostart.value !== props.plugin.autostart || Object.keys(draft.value).some(key =>
  JSON.stringify(draft.value[key]) !== JSON.stringify(props.plugin.config[key]),
))
const statusFields = computed(() => props.plugin.manifest.status_fields.filter(field =>
  props.plugin.runtime.phase === 'running' || field.available_offline,
))
const files = computed(() => {
  const values = props.plugin.runtime.fields.files
  if (!props.plugin.manifest.files || !Array.isArray(values)) return []
  return values.filter((file): file is { name: string; display_name?: string } =>
    typeof file?.name === 'string' && (!file.display_name || typeof file.display_name === 'string'),
  )
})

function textValue(field: PluginField): string {
  const value = draft.value[field.key]
  if (Array.isArray(value)) return value.join('\n')
  return value == null ? '' : String(value)
}

function editText(field: PluginField, value: string) {
  if (field.kind === 'string_list') {
    draft.value[field.key] = value.split('\n')
  } else if (field.kind === 'integer' && value !== '') {
    draft.value[field.key] = Number(value)
  } else {
    draft.value[field.key] = value
  }
}

function formatStatus(field: PluginStatusField, value: unknown) {
  if (typeof value === 'string') {
    const label = pluginText(field.value_labels?.[value], locale.value)
    if (label) return label
  }
  if (field.format === 'percent' && typeof value === 'number') {
    return new Intl.NumberFormat(locale.value, { style: 'percent' }).format(value)
  }
  if (typeof value === 'object' && value !== null) return JSON.stringify(value, null, 2)
  if (typeof value === 'boolean') return t(value ? 'plugins.yes' : 'plugins.no')
  return value == null ? '—' : String(value)
}

async function run(operation: () => Promise<void>) {
  busy.value = true
  try {
    await operation()
  } catch {
    // The request client displays the API error.
  } finally {
    busy.value = false
  }
}

function configure(enabled?: boolean) {
  const config = Object.fromEntries(Object.entries(draft.value).map(([key, value]) => [
    key, Array.isArray(value) ? value.map(item => item.trim()).filter(Boolean) : value,
  ]))
  return run(async () => {
    const plugin = await pluginsApi.configure(id.value, { enabled, autostart: autostart.value, config })
    store.upsert(plugin)
    draft.value = { ...plugin.config }
    autostart.value = plugin.autostart
  })
}

function start() {
  if (form.value?.reportValidity()) return configure(true)
}

function stop() {
  return run(async () => store.upsert(await pluginsApi.configure(id.value, { enabled: false })))
}

function action(action: Plugin['manifest']['actions'][number]) {
  if (action.confirm && !window.confirm(t('plugins.confirmAction', { action: pluginText(action.label, locale.value) }))) return
  return run(async () => {
    await pluginsApi.action(id.value, action.id)
    await store.refresh()
  })
}

function canRunAction(action: Plugin['manifest']['actions'][number]) {
  return props.plugin.runtime.phase === 'running' &&
    (action.when_status ?? []).every(condition =>
      props.plugin.runtime.fields[condition.key] === condition.equals,
    )
}

function removeFile(name: string) {
  if (!window.confirm(t('plugins.confirmFile', { name }))) return
  return run(async () => {
    await pluginsApi.deleteFile(id.value, name)
    await store.refresh()
  })
}

function refreshLogs() {
  return run(async () => { logs.value = await pluginsApi.logs(id.value) })
}
</script>

<template>
  <Card :class="compact && 'rounded-none border-0 shadow-none'">
    <CardHeader :class="compact ? 'flex flex-wrap items-start justify-between gap-3' : 'settings-card-heading'">
      <div class="min-w-0 space-y-1">
        <CardTitle :help="pluginText(plugin.manifest.description, locale)">{{ pluginText(plugin.manifest.name, locale) }}</CardTitle>
        <p class="text-sm text-muted-foreground">{{ t(`plugins.${plugin.runtime.phase}`) }}</p>
      </div>
      <div class="settings-card-actions plugin-card-actions ml-auto">
        <div class="settings-service-actions">
          <Button type="button" variant="outline" size="sm" :disabled="busy || plugin.enabled" @click="start">
            <Play class="size-4" aria-hidden="true" />
            {{ t('extensions.start') }}
          </Button>
          <Button type="button" variant="outline" size="sm" :disabled="busy || !plugin.enabled" @click="stop">
            <Square class="size-4" aria-hidden="true" />
            {{ t('extensions.stop') }}
          </Button>
        </div>
        <div class="settings-save-action">
          <Button :form="formId" type="submit" size="sm" :disabled="busy || !dirty">
            <Save class="size-4" aria-hidden="true" />
            {{ t('common.save') }}
          </Button>
        </div>
      </div>
    </CardHeader>
    <form :id="formId" ref="form" @submit.prevent="configure()">
      <CardContent class="space-y-4">
        <p v-if="plugin.runtime.error" class="break-words text-sm text-destructive" role="status">
          <span>{{ t('plugins.errorDetail') }}</span> {{ plugin.runtime.error }}
        </p>
        <div class="settings-toggle flex items-center justify-between gap-2">
          <label class="text-sm font-medium" :for="`${formId}-autostart`">{{ t('extensions.autoStart') }}</label>
          <Switch :id="`${formId}-autostart`" v-model="autostart" :disabled="busy" />
        </div>
        <div v-for="field in fields" :key="field.key" class="grid gap-2 sm:grid-cols-[minmax(9rem,1fr)_minmax(0,2fr)] sm:items-center">
          <label class="text-sm font-medium" :for="`${formId}-${field.key}`">{{ pluginText(field.label, locale) }}</label>
          <Switch v-if="field.kind === 'boolean'" :id="`${formId}-${field.key}`"
            :model-value="draft[field.key] === true" :disabled="busy"
            @update:model-value="draft[field.key] = $event" />
          <select v-else-if="field.kind === 'select'" :id="`${formId}-${field.key}`"
            class="min-h-9 rounded-md border bg-background px-3 text-sm" :disabled="busy"
            :value="draft[field.key]" @change="editText(field, ($event.target as HTMLSelectElement).value)">
            <option v-for="choice in field.choices" :key="choice" :value="choice">{{ pluginText(field.choice_labels?.[choice], locale) || choice }}</option>
          </select>
          <Textarea v-else-if="field.kind === 'textarea' || field.kind === 'string_list'"
            :id="`${formId}-${field.key}`" :model-value="textValue(field)" :disabled="busy" rows="4"
            @update:model-value="editText(field, String($event))" />
          <Input v-else :id="`${formId}-${field.key}`" :model-value="textValue(field)"
            :type="field.kind === 'integer' ? 'number' : field.kind" :disabled="busy"
            :min="field.min ?? undefined" :max="field.max ?? undefined" :required="field.kind === 'integer'"
            :autocomplete="field.kind === 'password' ? 'new-password' : undefined"
            @update:model-value="editText(field, String($event))" />
        </div>

        <dl v-if="statusFields.length" class="space-y-2 border-t pt-3 text-sm">
          <div v-for="field in statusFields" :key="field.key"
            class="grid gap-1 sm:grid-cols-[minmax(9rem,1fr)_minmax(0,2fr)]">
            <dt class="text-muted-foreground">{{ pluginText(field.label, locale) }}</dt>
            <dd class="min-w-0">
              <Button v-if="field.secret && plugin.runtime.fields[field.key]" type="button" variant="outline" size="sm"
                @click="revealed[field.key] = !revealed[field.key]">
                {{ t(revealed[field.key] ? 'plugins.hide' : 'plugins.reveal') }}
              </Button>
              <pre v-if="!field.secret || revealed[field.key]" class="whitespace-pre-wrap break-words font-mono text-xs">{{ formatStatus(field, plugin.runtime.fields[field.key]) }}</pre>
            </dd>
          </div>
        </dl>

        <ul v-if="files.length" class="space-y-2 border-t pt-3 text-sm">
          <li v-for="file in files" :key="file.name" class="flex items-center justify-between gap-3">
            <a :href="pluginsApi.downloadUrl(id, file.name)" class="min-w-0 break-words underline" download>{{ file.display_name || file.name }}</a>
            <Button type="button" variant="ghost" size="sm" :disabled="busy" @click="removeFile(file.name)">{{ t('plugins.deleteFile') }}</Button>
          </li>
        </ul>
      </CardContent>

      <CardFooter class="flex flex-wrap gap-2">
        <Button v-for="item in plugin.manifest.actions" :key="item.id" type="button" variant="outline"
          :disabled="busy || !canRunAction(item)" @click="action(item)">{{ pluginText(item.label, locale) }}</Button>
        <Collapsible v-model:open="logsOpen" class="w-full" @update:open="open => open && refreshLogs()">
          <CollapsibleTrigger as-child>
            <Button type="button" variant="ghost" size="sm" class="gap-2 text-muted-foreground">
              <ChevronRight class="size-4 transition-transform" :class="{ 'rotate-90': logsOpen }" aria-hidden="true" />
              {{ t('extensions.viewLogs') }}
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent class="space-y-2 pt-2">
            <pre v-if="logs !== null" class="max-h-48 overflow-auto rounded-md bg-muted p-3 font-mono text-xs">{{ logs.join('\n') || t('extensions.noLogs') }}</pre>
            <Button type="button" variant="ghost" size="sm" :disabled="busy" @click="refreshLogs">
              <RefreshCw class="size-3" aria-hidden="true" />
              {{ t('common.refresh') }}
            </Button>
          </CollapsibleContent>
        </Collapsible>
      </CardFooter>
    </form>
  </Card>
</template>


<style scoped>
.plugin-card-actions {
  justify-content: flex-end;
}
</style>
