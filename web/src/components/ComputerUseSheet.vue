<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { Bot, ChevronDown, Image, KeyRound, Play, Square } from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { computerUseApi, type ComputerUseAction, type ComputerUseConfig, type ComputerUseSession } from '@/api'
import type { ComputerUseTimelineItem } from '@/types/computerUseTimeline'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

const props = defineProps<{
  open: boolean
  connected: boolean
  wsError: string | null
  session: ComputerUseSession | null
  timeline: ComputerUseTimelineItem[]
}>()

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
const activeTab = ref('chat')
const messagesRef = ref<HTMLDivElement | null>(null)

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
const defaultMaxSteps = computed({
  get: () => String(config.value?.max_steps ?? 30),
  set: (value: string) => {
    if (config.value) config.value.max_steps = Number(value) || 30
  },
})
const defaultTimeoutSeconds = computed({
  get: () => String(config.value?.timeout_seconds ?? 600),
  set: (value: string) => {
    if (config.value) config.value.timeout_seconds = Number(value) || 600
  },
})

const status = computed(() => props.session?.status ?? 'idle')
const isRunning = computed(() => ['waiting_screenshot', 'thinking', 'executing'].includes(status.value))
const canStart = computed(() => !!config.value?.enabled && !!config.value?.api_key_configured && prompt.value.trim().length > 0 && !isRunning.value)
const showWelcome = computed(() => props.timeline.length === 0 && !props.session?.last_error && !props.session?.final_message)

const statusLabel = computed(() => {
  switch (status.value) {
    case 'waiting_screenshot': return '截屏中'
    case 'thinking': return '思考中'
    case 'executing': return '执行中'
    case 'completed': return '已完成'
    case 'failed': return '失败'
    case 'stopped': return '已停止'
    default: return '空闲'
  }
})

async function loadConfig() {
  config.value = await computerUseApi.config()
}

async function saveConfig() {
  savingConfig.value = true
  try {
    config.value = await computerUseApi.updateConfig({
      enabled: config.value?.enabled ?? true,
      base_url: config.value?.base_url || 'https://api.openai.com/v1/responses',
      model: config.value?.model || 'gpt-5.5',
      max_steps: config.value?.max_steps || 30,
      timeout_seconds: config.value?.timeout_seconds || 600,
      openai_api_key: apiKey.value.trim() || undefined,
    })
    apiKey.value = ''
    toast.success('Computer Use 配置已保存')
  } finally {
    savingConfig.value = false
  }
}

async function clearApiKey() {
  savingConfig.value = true
  try {
    config.value = await computerUseApi.updateConfig({
      clear_openai_api_key: true,
    })
    apiKey.value = ''
    toast.success('OpenAI API Key 已清除')
  } finally {
    savingConfig.value = false
  }
}

async function start() {
  if (!canStart.value) return
  const text = prompt.value.trim()
  starting.value = true
  try {
    emit('start', text)
    prompt.value = ''
  } finally {
    starting.value = false
  }
}

function formatAction(action: ComputerUseAction): string {
  switch (action.type) {
    case 'click':
      return `点击 (${action.x}, ${action.y}) ${action.button ?? 'left'}`
    case 'double_click':
      return `双击 (${action.x}, ${action.y}) ${action.button ?? 'left'}`
    case 'move':
      return `移动到 (${action.x}, ${action.y})`
    case 'drag':
      return `拖拽 ${action.path.length} 个点`
    case 'scroll':
      return `滚动 (${action.x}, ${action.y}) dx=${action.dx ?? 0} dy=${action.dy ?? 0}`
    case 'type':
      return `输入 ${action.text.length} 字符`
    case 'keypress':
      return `按键 ${action.keys.join('+')}`
    case 'wait':
      return `等待 ${action.ms}ms`
    case 'screenshot':
      return '请求截图'
  }
}

function scrollToBottom() {
  nextTick(() => {
    const el = messagesRef.value
    if (!el) return
    el.scrollTop = el.scrollHeight
  })
}

watch(() => props.timeline.length, scrollToBottom)
watch(() => props.open, (open) => {
  if (open) scrollToBottom()
})

onMounted(loadConfig)
</script>

<template>
  <aside
    v-show="open"
    class="absolute inset-y-0 right-0 z-30 h-full min-h-0 w-[min(100%,420px)] border-l bg-background/98 shadow-xl backdrop-blur md:relative md:z-auto md:w-[420px] xl:w-[460px]"
  >
    <div class="flex h-full min-h-0 flex-col">
      <div class="flex h-12 shrink-0 items-center justify-between border-b px-3">
        <div class="flex min-w-0 items-center gap-2">
          <Bot class="h-5 w-5 shrink-0" />
          <div class="min-w-0">
            <div class="truncate text-sm font-semibold">Computer Use</div>
            <div class="truncate text-[11px] text-muted-foreground">
              WebSocket {{ connected ? '已连接' : '未连接' }}
              <span v-if="wsError"> · {{ wsError }}</span>
            </div>
          </div>
        </div>
        <div class="flex items-center gap-1.5">
          <Badge :variant="status === 'failed' ? 'destructive' : 'secondary'">
            {{ statusLabel }}
          </Badge>
          <Button variant="ghost" size="icon" class="h-8 w-8" @click="emit('update:open', false)">
            <ChevronDown class="h-4 w-4 rotate-90" />
          </Button>
        </div>
      </div>

      <Tabs v-model="activeTab" class="flex min-h-0 flex-1 flex-col">
        <div class="px-3 py-2">
          <TabsList class="grid w-full grid-cols-2">
            <TabsTrigger value="chat">对话</TabsTrigger>
            <TabsTrigger value="settings">设置</TabsTrigger>
          </TabsList>
        </div>

        <TabsContent value="chat" class="m-0 flex min-h-0 flex-1 flex-col data-[state=inactive]:hidden">
          <div ref="messagesRef" class="min-h-0 flex-1 space-y-3 overflow-y-auto p-3">
            <div v-if="showWelcome" class="rounded-md border border-dashed p-4 text-center text-xs text-muted-foreground">
              发送任务后，这里会显示对话、截图和坐标操作。
            </div>

            <template v-for="item in timeline" :key="item.id">
              <div v-if="item.type === 'user'" class="flex justify-end">
                <div class="max-w-[86%] rounded-md bg-primary px-3 py-2 text-sm text-primary-foreground">
                  {{ item.text }}
                </div>
              </div>

              <div v-else-if="item.type === 'assistant'" class="flex justify-start">
                <div class="max-w-[86%] rounded-md border bg-muted/50 px-3 py-2 text-sm">
                  {{ item.text }}
                </div>
              </div>

              <div v-else-if="item.type === 'screenshot'" class="rounded-md border bg-card p-2">
                <div class="mb-2 flex items-center justify-between text-xs text-muted-foreground">
                  <span class="inline-flex items-center gap-1.5"><Image class="h-3.5 w-3.5" />截图</span>
                  <span>{{ item.screenshot.width }}x{{ item.screenshot.height }}</span>
                </div>
                <div
                  class="w-full overflow-hidden rounded-sm bg-black"
                  :style="{ aspectRatio: `${item.screenshot.width} / ${item.screenshot.height}` }"
                >
                  <img :src="item.screenshot.data_url" class="h-full w-full object-cover" alt="Computer Use screenshot" />
                </div>
              </div>

              <div v-else-if="item.type === 'actions_executed'" class="rounded-md border border-success/35 bg-success/10 p-2 text-success">
                <div class="mb-2 text-xs font-medium">已执行</div>
                <div class="space-y-1">
                  <div v-for="(action, index) in item.actions" :key="index" class="rounded-sm bg-background/60 px-2 py-1.5 text-xs">
                    {{ formatAction(action) }}
                  </div>
                </div>
              </div>

              <div v-else-if="item.type === 'error'" class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {{ item.text }}
              </div>

              <div v-else class="text-center text-xs text-muted-foreground">
                {{ item.text }}
              </div>
            </template>
          </div>

          <div class="shrink-0 border-t p-3">
            <Textarea
              v-model="prompt"
              rows="3"
              placeholder="继续输入任务或追问"
              :disabled="isRunning"
              @keydown.meta.enter.prevent="start"
              @keydown.ctrl.enter.prevent="start"
            />
            <div class="mt-2 flex gap-2">
              <Button class="flex-1 gap-2" :disabled="!canStart || starting" @click="start">
                <Play class="h-4 w-4" />
                发送
              </Button>
              <Button variant="outline" class="gap-2" :disabled="!isRunning" @click="emit('stop')">
                <Square class="h-4 w-4" />
                停止
              </Button>
              <Button variant="ghost" size="sm" :disabled="isRunning || timeline.length === 0" @click="emit('clear')">
                清空
              </Button>
            </div>
            <p v-if="!config?.api_key_configured" class="mt-2 text-xs text-muted-foreground">
              需要先在设置里保存 OpenAI API Key。
            </p>
          </div>
        </TabsContent>

        <TabsContent value="settings" class="m-0 min-h-0 flex-1 overflow-y-auto p-3 data-[state=inactive]:hidden">
          <div class="space-y-4">
            <div class="flex items-center justify-between rounded-md border p-3">
              <div>
                <div class="text-sm font-medium">启用 AI 操作</div>
                <div class="text-xs text-muted-foreground">配置保存后立即生效</div>
              </div>
              <Switch
                :model-value="config?.enabled ?? false"
                @update:model-value="(value) => { if (config) config.enabled = value }"
              />
            </div>

            <div class="space-y-3 rounded-md border p-3">
              <div class="grid grid-cols-2 gap-2">
                <div class="space-y-1">
                  <Label class="text-xs">模型</Label>
                  <Input v-model="defaultModel" :disabled="!config" placeholder="gpt-5.5" />
                </div>
                <div class="space-y-1">
                  <Label class="text-xs">最大步数</Label>
                  <Input v-model="defaultMaxSteps" type="number" min="1" max="100" />
                </div>
              </div>
              <div class="space-y-1">
                <Label class="text-xs">超时秒数</Label>
                <Input v-model="defaultTimeoutSeconds" type="number" min="30" max="3600" />
              </div>
              <div class="space-y-1">
                <Label class="text-xs">API URL</Label>
                <Input v-model="defaultBaseUrl" :disabled="!config" placeholder="https://api.openai.com/v1/responses" />
              </div>
              <div class="space-y-1">
                <Label class="text-xs flex items-center gap-1">
                  <KeyRound class="h-3.5 w-3.5" />
                  OpenAI API Key
                </Label>
                <Input
                  v-model="apiKey"
                  type="password"
                  autocomplete="off"
                  :placeholder="config?.api_key_configured ? `已配置：${config.api_key_source}` : 'sk-...'"
                />
              </div>
              <div class="grid grid-cols-2 gap-2">
                <Button size="sm" :disabled="savingConfig || !config" @click="saveConfig">
                  保存配置
                </Button>
                <Button size="sm" variant="outline" :disabled="savingConfig || !config?.api_key_configured" @click="clearApiKey">
                  清除 Key
                </Button>
              </div>
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  </aside>
</template>
