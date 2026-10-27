<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Copy, Save, Loader2 } from 'lucide-vue-next'
import { mcpApi, type McpConfig } from '@/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import CardTitle from './SettingsCardTitle.vue'
import { useClipboard } from '@/composables/useClipboard'

const clipboard = useClipboard()
const { t } = useI18n()
const config = ref<McpConfig | null>(null)
const enabled = ref(false)
const rate = ref(5)
const apiKey = ref('')
const busy = ref(false)
const endpoint = computed(
  () => new URL(config.value?.endpoint ?? '/mcp', window.location.origin).href,
)

function apply(value: McpConfig) {
  config.value = value
  enabled.value = value.enabled
  rate.value = value.requests_per_second
  apiKey.value = value.api_key ?? ''
}
async function load() {
  busy.value = true
  config.value = null
  try {
    apply(await mcpApi.config())
  } catch (error) {
    toast.error(error instanceof Error ? error.message : t('mcp.loadFailed'))
  } finally {
    busy.value = false
  }
}
async function save(regenerate = false) {
  if (!Number.isInteger(rate.value) || rate.value < 1 || rate.value > 100) {
    toast.error(t('mcp.invalidRate'))
    return
  }
  busy.value = true
  try {
    apply(
      await mcpApi.updateConfig({
        enabled: enabled.value,
        requests_per_second: rate.value,
        regenerate_api_key: regenerate,
      }),
    )
    toast.success(t('mcp.saved'))
  } catch (error) {
    toast.error(error instanceof Error ? error.message : t('mcp.saveFailed'))
  } finally {
    busy.value = false
  }
}
async function copy(value: string) {
  if (await clipboard.copy(value)) toast.success(t('mcp.copied'))
  else toast.error(t('mcp.copyFailed'))
}
onMounted(load)
</script>

<template>
  <Card>
    <CardHeader class="settings-card-heading">
      <CardTitle :help="t('mcp.description') + ' ' + t('mcp.connectHint')">{{
        t('mcp.title')
      }}</CardTitle>
      <div class="settings-card-actions">
        <Button :disabled="busy || !config" @click="save()"
          ><Loader2 v-if="busy" class="size-4 animate-spin" /><Save v-else class="size-4" />{{
            t('mcp.save')
          }}</Button
        >
      </div>
    </CardHeader>
    <CardContent class="space-y-5">
      <div class="settings-toggle">
        <Label for="mcp-enabled">{{ t('mcp.enabled') }}</Label>
        <Switch id="mcp-enabled" v-model="enabled" :disabled="busy || !config" />
      </div>
      <div class="settings-field space-y-2">
        <Label for="mcp-rate">{{ t('settingsUi.mcpRate') }}</Label>
        <Input
          id="mcp-rate"
          :aria-description="t('mcp.rate')"
          placeholder="1–100"
          v-model.number="rate"
          type="number"
          min="1"
          max="100"
          step="1"
          :disabled="busy || !config"
        />
      </div>
      <div class="settings-field space-y-2">
        <Label>{{ t('mcp.endpoint') }}</Label>
        <div class="flex items-center gap-2">
          <code
            id="mcp-endpoint"
            class="min-w-0 flex-1 break-all rounded bg-muted px-3 py-2 font-mono text-sm"
            >{{ endpoint }}</code
          >
          <Button
            variant="ghost"
            size="icon"
            :aria-label="t('mcp.copyEndpoint')"
            @click="copy(endpoint)"
            ><Copy class="size-4"
          /></Button>
        </div>
      </div>
      <div class="settings-field space-y-2">
        <Label>API Key</Label>
        <div v-if="apiKey" class="flex items-center gap-2">
          <code
            id="mcp-key"
            class="min-w-0 flex-1 break-all rounded bg-muted px-3 py-2 font-mono text-sm"
            >{{ apiKey }}</code
          >
          <Button variant="ghost" size="icon" :aria-label="t('mcp.copyKey')" @click="copy(apiKey)"
            ><Copy class="size-4"
          /></Button>
        </div>
        <p v-if="!apiKey" class="text-sm text-muted-foreground">
          {{ config?.api_key_configured ? t('mcp.keyConfigured') : t('mcp.keyMissing') }}
        </p>
        <Button variant="outline" :disabled="busy || !config" @click="save(true)">{{
          t('mcp.regenerate')
        }}</Button>
      </div>

      <div class="flex justify-end gap-2">
        <Button v-if="!config && !busy" variant="outline" @click="load">{{
          t('mcp.retry')
        }}</Button>
      </div>
    </CardContent>
  </Card>
</template>
