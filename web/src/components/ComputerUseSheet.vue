<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Bot,
  BrainCircuit,
  ChevronDown,
  ChevronRight,
  Image,
  KeyRound,
  SendHorizontal,
  Settings,
  Square,
  Trash2,
  Wifi,
  WifiOff,
  X,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { computerUseApi, type ComputerUseAction, type ComputerUseConfig, type ComputerUseSession } from '@/api'
import type { ComputerUseTimelineItem } from '@/types/computerUseTimeline'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

const props = defineProps<{
  open: boolean
  connected: boolean
  wsError: string | null
  session: ComputerUseSession | null
  timeline: ComputerUseTimelineItem[]
}>()

const { t } = useI18n()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'start', prompt: string): void
  (e: 'stop'): void
  (e: 'clear'): void
}>()

const config = ref<ComputerUseConfig | null>(null)
const prompt = ref('')
const apiKey = ref('')
const savingConfig = ref(false)
const starting = ref(false)
const settingsOpen = ref(false)
const messagesRef = ref<HTMLDivElement | null>(null)
const reasoningOverrides = ref<Record<string, boolean>>({})

const defaultModel = computed({
  get: () => config.value?.model ?? 'gpt-5.5',
  set: (value: string) => {
    if (config.value) config.value.model = value
  },
})
const defaultBaseUrl = computed({
  get: () => config.value?.base_url ?? 'https://api.openai.com/v1/responses',
  set: (value: string) => {
    if (config.value) config.value.base_url = value
  },
})

const status = computed(() => props.session?.status ?? 'idle')
const isRunning = computed(() => ['waiting_screenshot', 'thinking', 'executing'].includes(status.value))
const canStart = computed(() => (
  !!config.value?.enabled
  && !!config.value?.api_key_configured
  && prompt.value.trim().length > 0
  && !isRunning.value
))

const statusLabel = computed(() => {
  switch (status.value) {
    case 'waiting_screenshot': return t('computerUse.status.waitingScreenshot')
    case 'thinking': return t('computerUse.status.thinking')
    case 'executing': return t('computerUse.status.executing')
    case 'completed': return t('computerUse.status.completed')
    case 'failed': return t('computerUse.status.failed')
    case 'stopped': return t('computerUse.status.stopped')
    default: return t('computerUse.status.idle')
  }
})

const apiKeyPlaceholder = computed(() => {
  if (!config.value?.api_key_configured) return t('computerUse.settings.apiKey')
  const source = config.value.api_key_source
  const sourceLabel = source === 'env'
    ? t('computerUse.settings.sourceEnv')
    : source === 'config'
      ? t('computerUse.settings.sourceConfig')
      : t('computerUse.settings.sourceNone')
  return t('computerUse.settings.configured', { source: sourceLabel })
})

async function loadConfig() {
  try {
    config.value = await computerUseApi.config()
  } catch (err) {
    toast.error(t('computerUse.errors.configLoadFailed'), {
      description: err instanceof Error ? err.message : undefined,
    })
  }
}

async function saveConfig() {
  if (!config.value) return
  savingConfig.value = true
  try {
    config.value = await computerUseApi.updateConfig({
      enabled: config.value.enabled,
      base_url: config.value.base_url,
      model: config.value.model,
      api_key: apiKey.value.trim() || undefined,
    })
    apiKey.value = ''
    settingsOpen.value = false
    toast.success(t('computerUse.success.configSaved'))
  } finally {
    savingConfig.value = false
  }
}

async function clearApiKey() {
  savingConfig.value = true
  try {
    config.value = await computerUseApi.updateConfig({ clear_api_key: true })
    apiKey.value = ''
    toast.success(t('computerUse.success.apiKeyCleared'))
  } finally {
    savingConfig.value = false
  }
}

function start() {
  if (!canStart.value) return
  const text = prompt.value.trim()
  starting.value = true
  emit('start', text)
  prompt.value = ''
  starting.value = false
}

function formatAction(action: ComputerUseAction): string {
  switch (action.type) {
    case 'click':
      return t('computerUse.actions.click', {
        x: action.x,
        y: action.y,
        button: t(`computerUse.mouseButtons.${action.button ?? 'left'}`),
      })
    case 'double_click':
      return t('computerUse.actions.doubleClick', {
        x: action.x,
        y: action.y,
        button: t(`computerUse.mouseButtons.${action.button ?? 'left'}`),
      })
    case 'move':
      return t('computerUse.actions.move', { x: action.x, y: action.y })
    case 'drag':
      return t('computerUse.actions.drag', { count: action.path.length })
    case 'scroll':
      return t('computerUse.actions.scroll', {
        x: action.x,
        y: action.y,
        dx: action.dx ?? 0,
        dy: action.dy ?? 0,
      })
    case 'type':
      return t('computerUse.actions.typeAscii', { count: action.text.length })
    case 'keypress':
      return t('computerUse.actions.keypress', { keys: action.keys.join('+') })
    case 'wait':
      return t('computerUse.actions.wait', { ms: action.ms })
    case 'screenshot':
      return t('computerUse.actions.screenshot')
  }
}

function reasoningOpen(item: Extract<ComputerUseTimelineItem, { type: 'reasoning' }>) {
  return reasoningOverrides.value[item.id] ?? !item.completed
}

function toggleReasoning(item: Extract<ComputerUseTimelineItem, { type: 'reasoning' }>) {
  reasoningOverrides.value[item.id] = !reasoningOpen(item)
}

function scrollToBottom() {
  nextTick(() => {
    const el = messagesRef.value
    if (el) el.scrollTop = el.scrollHeight
  })
}

watch(() => props.timeline, scrollToBottom, { deep: true })
watch(() => props.open, (open) => {
  if (open) scrollToBottom()
})

onMounted(loadConfig)
</script>

<template>
  <aside
    v-show="open"
    class="absolute inset-y-0 right-0 z-30 h-full min-h-0 w-full border-l bg-background shadow-xl sm:w-[420px] md:relative md:z-auto xl:w-[460px]"
  >
    <div class="flex h-full min-h-0 flex-col">
      <header class="flex h-12 shrink-0 items-center gap-2 border-b px-3">
        <Bot class="h-5 w-5 shrink-0" />
        <div class="min-w-0 flex-1">
          <div class="truncate text-sm font-semibold">{{ t('computerUse.title') }}</div>
          <div
            class="flex min-w-0 items-center gap-1 text-[11px]"
            :class="connected ? 'text-emerald-600 dark:text-emerald-400' : 'text-muted-foreground'"
          >
            <Wifi v-if="connected" class="h-3 w-3 shrink-0" />
            <WifiOff v-else class="h-3 w-3 shrink-0" />
            <span class="truncate">
              {{ connected ? t('computerUse.connection.connected') : (wsError || t('computerUse.connection.disconnected')) }}
            </span>
            <span class="text-muted-foreground">· {{ statusLabel }}</span>
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8"
          :title="t('computerUse.buttons.settings')"
          :aria-label="t('computerUse.buttons.settings')"
          @click="settingsOpen = true"
        >
          <Settings class="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8"
          :title="t('computerUse.buttons.clear')"
          :aria-label="t('computerUse.buttons.clear')"
          :disabled="isRunning || timeline.length === 0"
          @click="emit('clear')"
        >
          <Trash2 class="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8"
          :title="t('computerUse.buttons.close')"
          :aria-label="t('computerUse.buttons.close')"
          @click="emit('update:open', false)"
        >
          <X class="h-4 w-4" />
        </Button>
      </header>

      <div ref="messagesRef" class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        <div v-if="timeline.length === 0" class="flex h-full items-center justify-center text-muted-foreground/45">
          <Bot class="h-10 w-10" />
        </div>

        <div v-else class="space-y-5">
          <template v-for="item in timeline" :key="item.id">
            <div v-if="item.type === 'user'" class="flex justify-end">
              <div class="max-w-[86%] whitespace-pre-wrap break-words rounded-md bg-muted px-3 py-2 text-sm">
                {{ item.text }}
              </div>
            </div>

            <div v-else-if="item.type === 'assistant'" class="whitespace-pre-wrap break-words text-sm leading-6">
              {{ item.text }}
            </div>

            <div v-else-if="item.type === 'reasoning'" class="border-l-2 border-muted pl-3 text-xs text-muted-foreground">
              <button
                type="button"
                class="flex w-full items-center gap-1.5 py-1 text-left font-medium hover:text-foreground"
                @click="toggleReasoning(item)"
              >
                <ChevronDown v-if="reasoningOpen(item)" class="h-3.5 w-3.5 shrink-0" />
                <ChevronRight v-else class="h-3.5 w-3.5 shrink-0" />
                <BrainCircuit class="h-3.5 w-3.5 shrink-0" />
                <span>
                  {{ item.failed
                    ? t('computerUse.reasoning.failed')
                    : (item.completed ? t('computerUse.reasoning.process') : t('computerUse.reasoning.thinking')) }}
                </span>
              </button>
              <div
                v-if="reasoningOpen(item)"
                class="max-h-72 overflow-y-auto whitespace-pre-wrap break-words pb-2 pt-1 leading-5"
              >
                {{ item.text || t('computerUse.reasoning.thinking') }}
              </div>
            </div>

            <div v-else-if="item.type === 'screenshot'" class="overflow-hidden rounded-md border bg-card">
              <div class="flex items-center justify-between border-b px-2.5 py-1.5 text-[11px] text-muted-foreground">
                <span class="inline-flex items-center gap-1.5">
                  <Image class="h-3.5 w-3.5" />{{ t('computerUse.trace.screenshot') }}
                </span>
                <span>{{ item.screenshot.width }}x{{ item.screenshot.height }}</span>
              </div>
              <div
                class="w-full bg-black"
                :style="{ aspectRatio: `${item.screenshot.width} / ${item.screenshot.height}` }"
              >
                <img
                  :src="item.screenshot.data_url"
                  class="h-full w-full object-contain"
                  :alt="t('computerUse.trace.screenshotAlt')"
                />
              </div>
            </div>

            <div v-else-if="item.type === 'actions_executed'" class="border-l-2 border-emerald-500/50 pl-3 text-xs">
              <div class="mb-1 text-[11px] font-medium text-emerald-600 dark:text-emerald-400">
                {{ t('computerUse.trace.executed') }}
              </div>
              <div class="space-y-1 text-muted-foreground">
                <div v-for="(action, index) in item.actions" :key="index" class="break-words">
                  {{ formatAction(action) }}
                </div>
              </div>
            </div>

            <div
              v-else-if="item.type === 'error'"
              class="whitespace-pre-wrap break-words rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs leading-5 text-destructive"
            >
              {{ item.text }}
            </div>

            <div v-else class="text-center text-xs text-muted-foreground">
              {{ item.text }}
            </div>
          </template>
        </div>
      </div>

      <footer class="shrink-0 border-t bg-background p-3">
        <div class="relative">
          <Textarea
            v-model="prompt"
            rows="3"
            class="max-h-40 min-h-20 resize-none pr-12"
            :placeholder="t('computerUse.input.placeholder')"
            :disabled="isRunning"
            @keydown.meta.enter.prevent="start"
            @keydown.ctrl.enter.prevent="start"
          />
          <Button
            v-if="!isRunning"
            size="icon"
            class="absolute bottom-2 right-2 h-8 w-8"
            :title="t('computerUse.buttons.send')"
            :aria-label="t('computerUse.buttons.send')"
            :disabled="!canStart || starting"
            @click="start"
          >
            <SendHorizontal class="h-4 w-4" />
          </Button>
          <Button
            v-else
            size="icon"
            variant="destructive"
            class="absolute bottom-2 right-2 h-8 w-8"
            :title="t('computerUse.buttons.stop')"
            :aria-label="t('computerUse.buttons.stop')"
            @click="emit('stop')"
          >
            <Square class="h-3.5 w-3.5 fill-current" />
          </Button>
        </div>
        <p v-if="config && !config.api_key_configured" class="mt-2 text-xs text-muted-foreground">
          {{ t('computerUse.input.apiKeyRequired') }}
        </p>
      </footer>
    </div>

    <Dialog v-model:open="settingsOpen">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('computerUse.settings.title') }}</DialogTitle>
        </DialogHeader>

        <div class="space-y-4 py-2">
          <div class="flex items-center justify-between gap-4">
            <Label for="cua-enabled">{{ t('computerUse.settings.enableAi') }}</Label>
            <Switch
              id="cua-enabled"
              :model-value="config?.enabled ?? false"
              @update:model-value="(value) => { if (config) config.enabled = value }"
            />
          </div>

          <div class="space-y-1.5">
            <Label for="cua-model">{{ t('computerUse.settings.model') }}</Label>
            <Input id="cua-model" v-model="defaultModel" :disabled="!config" placeholder="gpt-5.5" />
          </div>

          <div class="space-y-1.5">
            <Label for="cua-url">{{ t('computerUse.settings.apiUrl') }}</Label>
            <Input
              id="cua-url"
              v-model="defaultBaseUrl"
              :disabled="!config"
              placeholder="https://api.example.com/v1/responses"
            />
          </div>

          <div class="space-y-1.5">
            <Label for="cua-key" class="flex items-center gap-1.5">
              <KeyRound class="h-3.5 w-3.5" />
              {{ t('computerUse.settings.apiKey') }}
            </Label>
            <Input
              id="cua-key"
              v-model="apiKey"
              type="password"
              autocomplete="off"
              :placeholder="apiKeyPlaceholder"
            />
          </div>
        </div>

        <DialogFooter class="gap-2 sm:justify-between">
          <Button
            variant="outline"
            :disabled="savingConfig || !config?.api_key_configured"
            @click="clearApiKey"
          >
            {{ t('computerUse.buttons.clearKey') }}
          </Button>
          <Button :disabled="savingConfig || !config" @click="saveConfig">
            {{ t('computerUse.buttons.save') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </aside>
</template>
