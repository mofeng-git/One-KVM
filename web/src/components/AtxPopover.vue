<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Power, RotateCcw, CircleDot, Wifi, Send, HardDrive } from 'lucide-vue-next'
import { atxApi } from '@/api'
import { atxConfigApi } from '@/api/config'

type AtxAction = 'short' | 'long' | 'reset'

const minActionFeedbackMs = 800
const actionDurations: Record<AtxAction, number> = {
  short: 500,
  long: 5000,
  reset: 500,
}

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'powerShort'): void
  (e: 'powerLong'): void
  (e: 'reset'): void
  (e: 'wol', macAddress: string): void
}>()

const { t } = useI18n()

const activeTab = ref('atx')
const tabTriggerClass = 'h-9 rounded-md border-0 bg-transparent text-center text-xs text-muted-foreground shadow-none hover:text-foreground data-[state=active]:border-0 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm'

const powerState = ref<'on' | 'off' | 'unknown'>('unknown')
const hddState = ref<'active' | 'inactive' | 'unknown'>('unknown')
let powerStateTimer: number | null = null
let actionTimer: number | null = null

const wolMacAddress = ref('')
const wolHistory = ref<string[]>([])
const wolSending = ref(false)
const wolLoadingHistory = ref(false)
const activeAction = ref<AtxAction | null>(null)

const actionBusy = computed(() => activeAction.value !== null)

const powerStateIconColor = computed(() => {
  switch (powerState.value) {
    case 'on': return 'text-success'
    case 'off': return 'text-muted-foreground'
    default: return 'text-warning'
  }
})

const powerStateTextColor = computed(() => {
  switch (powerState.value) {
    case 'on': return 'text-success'
    default: return ''
  }
})

const powerStateText = computed(() => {
  switch (powerState.value) {
    case 'on': return t('atx.stateOn')
    case 'off': return t('atx.stateOff')
    default: return t('atx.stateUnknown')
  }
})

const hddStateIconColor = computed(() => {
  switch (hddState.value) {
    case 'active': return 'text-success'
    case 'inactive': return 'text-muted-foreground'
    default: return 'text-warning'
  }
})

const hddStateTextColor = computed(() => {
  switch (hddState.value) {
    case 'active': return 'text-success'
    default: return ''
  }
})

const hddStateText = computed(() => {
  switch (hddState.value) {
    case 'active': return t('atx.hddActive')
    case 'inactive': return t('atx.hddInactive')
    default: return t('atx.stateUnknown')
  }
})

function handleAction(action: AtxAction) {
  if (actionBusy.value) return

  console.log('[AtxPopover] Running action:', action)
  activeAction.value = action

  if (action === 'short') emit('powerShort')
  else if (action === 'long') emit('powerLong')
  else emit('reset')

  if (actionTimer !== null) {
    window.clearTimeout(actionTimer)
  }
  actionTimer = window.setTimeout(() => {
    activeAction.value = null
    actionTimer = null
    refreshPowerState().catch(() => {})
  }, Math.max(actionDurations[action], minActionFeedbackMs))
}

const isValidMac = computed(() => {
  const mac = wolMacAddress.value.trim()
  const macRegex = /^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$|^([0-9A-Fa-f]{12})$/
  return macRegex.test(mac)
})

function sendWol() {
  if (!isValidMac.value) return
  wolSending.value = true

  let mac = wolMacAddress.value.trim().toUpperCase()
  if (mac.length === 12) {
    mac = mac.match(/.{2}/g)!.join(':')
  } else {
    mac = mac.replace(/-/g, ':')
  }

  emit('wol', mac)

  wolHistory.value = [mac, ...wolHistory.value.filter(item => item !== mac)].slice(0, 5)
  setTimeout(() => {
    loadWolHistory().catch(() => {})
  }, 1200)

  setTimeout(() => {
    wolSending.value = false
  }, 1000)
}

function selectFromHistory(mac: string) {
  wolMacAddress.value = mac
}

async function loadWolHistory() {
  wolLoadingHistory.value = true
  try {
    const response = await atxConfigApi.getWolHistory(5)
    wolHistory.value = response.history.map(item => item.mac_address)
  } catch {
    wolHistory.value = []
  } finally {
    wolLoadingHistory.value = false
  }
}

async function refreshPowerState() {
  try {
    const state = await atxApi.status()
    powerState.value = state.power_status
    hddState.value = state.hdd_status
  } catch {
    powerState.value = 'unknown'
    hddState.value = 'unknown'
  }
}

onMounted(() => {
  refreshPowerState().catch(() => {})
  powerStateTimer = window.setInterval(() => {
    refreshPowerState().catch(() => {})
  }, 3000)
})

onUnmounted(() => {
  if (powerStateTimer !== null) {
    window.clearInterval(powerStateTimer)
    powerStateTimer = null
  }
  if (actionTimer !== null) {
    window.clearTimeout(actionTimer)
    actionTimer = null
  }
})

watch(
  () => activeTab.value,
  (tab) => {
    if (tab === 'wol') {
      loadWolHistory().catch(() => {})
    }
  },
  { immediate: true },
)
</script>

<template>
  <div class="p-2.5 space-y-2.5">
    <Tabs v-model="activeTab">
      <TabsList class="grid h-auto w-full grid-cols-2 gap-1 rounded-md border border-border bg-muted p-1">
        <TabsTrigger
          value="atx"
          :class="tabTriggerClass"
        >
          <Power class="h-3 w-3 mr-1" />
          {{ t('atx.title') }}
        </TabsTrigger>
        <TabsTrigger
          value="wol"
          :class="tabTriggerClass"
        >
          <Wifi class="h-3 w-3 mr-1" />
          WOL
        </TabsTrigger>
      </TabsList>

      <!-- ATX Tab -->
      <TabsContent value="atx" class="mt-2.5 space-y-2.5">
        <!-- Status -->
        <div class="grid grid-cols-2 gap-2">
          <div class="flex min-w-0 items-center gap-2 rounded-md border bg-muted/40 px-2 py-1.5">
            <Power :class="['h-4 w-4 shrink-0', powerStateIconColor]" />
            <div class="min-w-0">
              <p class="truncate text-[11px] leading-none text-muted-foreground">{{ t('atx.powerState') }}</p>
              <p :class="['mt-1 truncate text-xs font-medium leading-none', powerStateTextColor]">{{ powerStateText }}</p>
            </div>
          </div>
          <div class="flex min-w-0 items-center gap-2 rounded-md border bg-muted/40 px-2 py-1.5">
            <HardDrive :class="['h-4 w-4 shrink-0', hddStateIconColor]" />
            <div class="min-w-0">
              <p class="truncate text-[11px] leading-none text-muted-foreground">{{ t('atx.hddState') }}</p>
              <p :class="['mt-1 truncate text-xs font-medium leading-none', hddStateTextColor]">{{ hddStateText }}</p>
            </div>
          </div>
        </div>

        <Separator />

        <!-- Power Actions -->
        <div class="space-y-1">
          <Button
            variant="outline"
            size="sm"
            :disabled="actionBusy"
            :class="[
              'w-full justify-start gap-2 h-7 text-xs',
              activeAction === 'short' ? 'bg-muted text-muted-foreground' : '',
            ]"
            @click="handleAction('short')"
          >
            <Power class="h-3 w-3" />
            {{ t('atx.shortPress') }}
          </Button>

          <Button
            variant="outline"
            size="sm"
            :disabled="actionBusy"
            :class="[
              'h-7 w-full justify-start gap-2 text-xs text-warning hover:bg-warning/10 hover:text-warning',
              activeAction === 'long' ? 'bg-muted text-muted-foreground hover:text-muted-foreground hover:bg-muted dark:hover:bg-muted' : '',
            ]"
            @click="handleAction('long')"
          >
            <CircleDot class="h-3 w-3" />
            {{ t('atx.longPress') }}
          </Button>

          <Button
            variant="outline"
            size="sm"
            :disabled="actionBusy"
            :class="[
              'h-7 w-full justify-start gap-2 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive',
              activeAction === 'reset' ? 'bg-muted text-muted-foreground hover:text-muted-foreground hover:bg-muted dark:hover:bg-muted' : '',
            ]"
            @click="handleAction('reset')"
          >
            <RotateCcw class="h-3 w-3" />
            {{ t('atx.reset') }}
          </Button>
        </div>
      </TabsContent>

      <!-- WOL Tab -->
      <TabsContent value="wol" class="mt-2.5 space-y-2.5">
        <div class="space-y-2">
          <Label for="mac-address" class="text-xs">{{ t('atx.macAddress') }}</Label>
          <div class="flex gap-2">
            <Input
              id="mac-address"
              v-model="wolMacAddress"
              placeholder="AA:BB:CC:DD:EE:FF"
              class="h-8 text-xs font-mono"
              @keyup.enter="sendWol"
            />
            <Button
              size="sm"
              class="h-8 px-3"
              :disabled="!isValidMac || wolSending"
              @click="sendWol"
            >
              <Send class="h-3.5 w-3.5" />
            </Button>
          </div>
          <p v-if="wolMacAddress && !isValidMac" class="text-xs text-destructive">
            {{ t('atx.invalidMac') }}
          </p>
        </div>

        <p v-if="wolLoadingHistory" class="text-xs text-muted-foreground">
          {{ t('common.loading') }}
        </p>

        <!-- History -->
        <div v-if="wolHistory.length > 0" class="space-y-2">
          <Separator />
          <Label class="text-xs text-muted-foreground">{{ t('atx.recentMac') }}</Label>
          <div class="space-y-1">
            <Button
              v-for="mac in wolHistory"
              :key="mac"
              variant="ghost"
              size="sm"
              class="w-full justify-start font-mono text-xs"
              @click="selectFromHistory(mac)"
            >
              {{ mac }}
            </Button>
          </div>
        </div>
      </TabsContent>
    </Tabs>
  </div>
</template>
