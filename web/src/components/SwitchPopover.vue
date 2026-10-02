<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { MonitorCog, Server, CheckCircle2, CircleAlert } from 'lucide-vue-next'
import { switchApi, type SwitchStatus } from '@/api'
import { switchConfigApi } from '@/api/config'
import type { SwitchConfig } from '@/types/generated'

const emit = defineEmits<{
  (e: 'close'): void
}>()

const { t } = useI18n()

const activeTab = ref('switch')
const tabTriggerClass = 'h-8 rounded-md border-0 bg-transparent text-center text-xs text-muted-foreground shadow-none hover:text-foreground data-[state=active]:border-0 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm'

const status = ref<SwitchStatus | null>(null)
const loading = ref(false)
const switchingChannel = ref<number | null>(null)
const serialDevices = ref<string[]>([])
const saving = ref(false)

// Config state
const config = ref<SwitchConfig | null>(null)
const enabled = ref(false)
const device = ref('')
const baudRate = ref(19200)
const channelCount = ref(8)
const channelNames = ref<string[]>([])

let refreshTimer: number | null = null

const channels = computed(() =>
  Array.from({ length: Math.max(1, Math.min(8, channelCount.value)) }, (_, i) => i + 1),
)

const currentInput = computed(() =>
  status.value?.current_channel != null ? status.value.current_channel : null,
)

const connected = computed(() => status.value?.connected === true)
const available = computed(() => status.value?.available === true)

function channelLabel(index: number): string {
  const names = channelNames.value
  const name = names[index - 1]?.trim()
  return name || t('kvmSwitch.switch') + ' ' + index
}

async function refreshStatus() {
  try {
    status.value = await switchApi.status()
  } catch {
    status.value = null
  }
}

async function loadConfig() {
  try {
    const cfg = await switchConfigApi.get()
    config.value = cfg
    enabled.value = cfg.enabled
    device.value = cfg.device
    baudRate.value = cfg.baud_rate
    channelCount.value = cfg.channel_count
    channelNames.value = Array.from({ length: cfg.channel_count }, (_, i) =>
      cfg.channel_names?.[i] ?? '',
    )
    serialDevices.value = await switchConfigApi.listDevices()
  } catch {
    // ignore
  }
}

async function switchTo(index: number) {
  if (switchingChannel.value != null) return
  switchingChannel.value = index
  try {
    await switchApi.switchChannel(index)
    // Optimistically mark the target and let the next poll confirm.
    if (status.value) {
      status.value.current_channel = index
      status.value.channels = status.value.channels.map((ch) => ({ ...ch, active: ch.index === index }))
    }
    setTimeout(() => refreshStatus(), 600)
  } catch {
    // Leave state as-is; the periodic poll will reflect reality.
  } finally {
    switchingChannel.value = null
  }
}

async function saveConfig() {
  saving.value = true
  try {
    const names = channelNames.value.slice(0, Math.max(1, channelCount.value))
    await switchConfigApi.update({
      enabled: enabled.value,
      device: device.value,
      baud_rate: baudRate.value,
      channel_count: channelCount.value,
      channel_names: names,
    })
    await loadConfig()
    await refreshStatus()
    activeTab.value = 'switch'
  } catch {
    // error toast handled by request layer
  } finally {
    saving.value = false
  }
}

watch(channelCount, (count) => {
  const len = Math.max(1, Math.min(8, count))
  channelNames.value = Array.from({ length: len }, (_, i) => channelNames.value[i] ?? '')
})

onMounted(() => {
  refreshStatus()
  loadConfig()
  refreshTimer = window.setInterval(() => refreshStatus(), 3000)
})

onUnmounted(() => {
  if (refreshTimer !== null) {
    window.clearInterval(refreshTimer)
    refreshTimer = null
  }
})
</script>

<template>
  <div class="p-2.5 space-y-2.5">
    <Tabs v-model="activeTab">
      <TabsList class="grid h-auto w-full grid-cols-2 gap-1 rounded-md border border-border bg-muted p-0.5">
        <TabsTrigger value="switch" :class="tabTriggerClass">
          <MonitorCog class="size-3 mr-1" />
          {{ t('kvmSwitch.title') }}
        </TabsTrigger>
        <TabsTrigger value="config" :class="tabTriggerClass">
          <Server class="size-3 mr-1" />
          {{ t('kvmSwitch.config') }}
        </TabsTrigger>
      </TabsList>

      <!-- Input switching tab -->
      <TabsContent value="switch" class="mt-2.5 space-y-2.5">
        <div class="flex min-w-0 items-center gap-2 rounded-md border bg-muted/40 px-2 py-1.5">
          <CheckCircle2 v-if="connected" class="size-4 shrink-0 text-success" />
          <CircleAlert v-else class="size-4 shrink-0 text-warning" />
          <div class="min-w-0">
            <p class="truncate text-[11px] leading-none text-muted-foreground">{{ t('kvmSwitch.currentInput') }}</p>
            <p class="mt-1 truncate text-xs font-medium leading-none">
              {{ currentInput != null ? channelLabel(currentInput) : '—' }}
            </p>
          </div>
        </div>

        <p v-if="!available" class="text-xs text-muted-foreground">{{ t('kvmSwitch.notConfigured') }}</p>
        <p v-else-if="!connected" class="text-xs text-warning">{{ t('kvmSwitch.notConnected') }}</p>

        <Separator />

        <div v-if="available" class="grid grid-cols-2 gap-1.5">
          <Button
            v-for="ch in status?.channels ?? []"
            :key="ch.index"
            variant="outline"
            size="sm"
            :disabled="switchingChannel != null"
            :class="[
              'h-8 w-full justify-start gap-2 text-xs',
              ch.active ? 'bg-muted text-foreground border-success' : '',
              switchingChannel === ch.index ? 'opacity-60' : '',
            ]"
            @click="switchTo(ch.index)"
          >
            <span class="size-2 rounded-full" :class="ch.active ? 'bg-success' : 'bg-muted-foreground/40'" />
            <span class="truncate">{{ ch.label }}</span>
          </Button>
        </div>

        <p v-else-if="loading" class="text-xs text-muted-foreground">{{ t('common.loading') }}</p>
      </TabsContent>

      <!-- Configuration tab -->
      <TabsContent value="config" class="mt-2.5 space-y-2.5">
        <div class="flex items-center justify-between">
          <Label for="switch-enabled" class="text-xs">{{ t('kvmSwitch.enabled') }}</Label>
          <Switch id="switch-enabled" v-model="enabled" />
        </div>

        <div class="space-y-1.5">
          <Label for="switch-device" class="text-xs">{{ t('kvmSwitch.device') }}</Label>
          <Select v-model="device">
            <SelectTrigger id="switch-device" class="h-8 text-xs">
              <SelectValue :placeholder="t('kvmSwitch.selectDevice')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="d in serialDevices" :key="d" :value="d">{{ d }}</SelectItem>
            </SelectContent>
          </Select>
          <p v-if="serialDevices.length === 0" class="text-xs text-muted-foreground">
            {{ t('kvmSwitch.noDevice') }}
          </p>
        </div>

        <div class="grid grid-cols-2 gap-2">
          <div class="space-y-1.5">
            <Label for="switch-baud" class="text-xs">{{ t('kvmSwitch.baudRate') }}</Label>
            <Input id="switch-baud" v-model.number="baudRate" type="number" class="h-8 text-xs" />
          </div>
          <div class="space-y-1.5">
            <Label class="text-xs">{{ t('kvmSwitch.channelCount') }}</Label>
            <Input v-model.number="channelCount" type="number" min="1" max="8" class="h-8 text-xs" />
          </div>
        </div>


        <div v-if="channelNames.length" class="space-y-1.5">
          <Label class="text-xs">{{ t('kvmSwitch.title') }}</Label>
          <div v-for="ch in channels" :key="ch" class="flex items-center gap-2">
            <span class="w-6 shrink-0 text-right text-[11px] text-muted-foreground">{{ ch }}</span>
            <Input
              v-model="channelNames[ch - 1]"
              class="h-8 text-xs"
              :placeholder="t('kvmSwitch.switch') + ' ' + ch"
            />
          </div>
        </div>

        <Button
          size="sm"
          class="h-8 w-full"
          :disabled="saving"
          @click="saveConfig"
        >
          {{ t('common.save') }}
        </Button>
      </TabsContent>
    </Tabs>
  </div>
</template>
