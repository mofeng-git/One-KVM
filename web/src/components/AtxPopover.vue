<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Power, RotateCcw, CircleDot, Wifi, Send } from 'lucide-vue-next'

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'powerShort'): void
  (e: 'powerLong'): void
  (e: 'reset'): void
  (e: 'wol', macAddress: string): void
}>()

const { t } = useI18n()

const activeTab = ref('atx')

// ATX state
const powerState = ref<'on' | 'off' | 'unknown'>('unknown')
const confirmAction = ref<'short' | 'long' | 'reset' | null>(null)

// WOL state
const wolMacAddress = ref('')
const wolHistory = ref<string[]>([])
const wolSending = ref(false)

const powerStateColor = computed(() => {
  switch (powerState.value) {
    case 'on': return 'bg-green-500'
    case 'off': return 'bg-slate-400'
    default: return 'bg-yellow-500'
  }
})

const powerStateText = computed(() => {
  switch (powerState.value) {
    case 'on': return t('atx.stateOn')
    case 'off': return t('atx.stateOff')
    default: return t('atx.stateUnknown')
  }
})

function handleAction() {
  if (confirmAction.value === 'short') emit('powerShort')
  else if (confirmAction.value === 'long') emit('powerLong')
  else if (confirmAction.value === 'reset') emit('reset')
  confirmAction.value = null
}

const confirmTitle = computed(() => {
  switch (confirmAction.value) {
    case 'short': return t('atx.confirmShortTitle')
    case 'long': return t('atx.confirmLongTitle')
    case 'reset': return t('atx.confirmResetTitle')
    default: return ''
  }
})

const confirmDescription = computed(() => {
  switch (confirmAction.value) {
    case 'short': return t('atx.confirmShortDesc')
    case 'long': return t('atx.confirmLongDesc')
    case 'reset': return t('atx.confirmResetDesc')
    default: return ''
  }
})

// MAC address validation
const isValidMac = computed(() => {
  const mac = wolMacAddress.value.trim()
  // Support formats: AA:BB:CC:DD:EE:FF or AA-BB-CC-DD-EE-FF or AABBCCDDEEFF
  const macRegex = /^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$|^([0-9A-Fa-f]{12})$/
  return macRegex.test(mac)
})

function sendWol() {
  if (!isValidMac.value) return
  wolSending.value = true

  // Normalize MAC address
  let mac = wolMacAddress.value.trim().toUpperCase()
  if (mac.length === 12) {
    mac = mac.match(/.{2}/g)!.join(':')
  } else {
    mac = mac.replace(/-/g, ':')
  }

  emit('wol', mac)

  // Add to history if not exists
  if (!wolHistory.value.includes(mac)) {
    wolHistory.value.unshift(mac)
    // Keep only last 5
    if (wolHistory.value.length > 5) {
      wolHistory.value.pop()
    }
    // Save to localStorage
    localStorage.setItem('wol_history', JSON.stringify(wolHistory.value))
  }

  setTimeout(() => {
    wolSending.value = false
  }, 1000)
}

function selectFromHistory(mac: string) {
  wolMacAddress.value = mac
}

// Load WOL history on mount
const savedHistory = localStorage.getItem('wol_history')
if (savedHistory) {
  try {
    wolHistory.value = JSON.parse(savedHistory)
  } catch (e) {
    wolHistory.value = []
  }
}
</script>

<template>
  <div class="p-3 space-y-3">
    <Tabs v-model="activeTab">
      <TabsList class="w-full grid grid-cols-2">
        <TabsTrigger value="atx" class="text-xs">
          <Power class="h-3.5 w-3.5 mr-1" />
          {{ t('atx.title') }}
        </TabsTrigger>
        <TabsTrigger value="wol" class="text-xs">
          <Wifi class="h-3.5 w-3.5 mr-1" />
          WOL
        </TabsTrigger>
      </TabsList>

      <!-- ATX Tab -->
      <TabsContent value="atx" class="mt-3 space-y-3">
        <p class="text-xs text-muted-foreground">{{ t('atx.description') }}</p>

        <!-- Power State -->
        <div class="flex items-center justify-between p-2 rounded-md bg-muted/50">
          <span class="text-xs text-muted-foreground">{{ t('atx.powerState') }}</span>
          <Badge variant="outline" class="gap-1.5 text-xs">
            <span :class="['h-2 w-2 rounded-full', powerStateColor]" />
            {{ powerStateText }}
          </Badge>
        </div>

        <Separator />

        <!-- Power Actions -->
        <div class="space-y-1.5">
          <Button
            variant="outline"
            size="sm"
            class="w-full justify-start gap-2 h-8 text-xs"
            @click="confirmAction = 'short'"
          >
            <Power class="h-3.5 w-3.5" />
            {{ t('atx.shortPress') }}
          </Button>

          <Button
            variant="outline"
            size="sm"
            class="w-full justify-start gap-2 h-8 text-xs text-orange-600 hover:text-orange-700 hover:bg-orange-50 dark:hover:bg-orange-950"
            @click="confirmAction = 'long'"
          >
            <CircleDot class="h-3.5 w-3.5" />
            {{ t('atx.longPress') }}
          </Button>

          <Button
            variant="outline"
            size="sm"
            class="w-full justify-start gap-2 h-8 text-xs text-red-600 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-950"
            @click="confirmAction = 'reset'"
          >
            <RotateCcw class="h-3.5 w-3.5" />
            {{ t('atx.reset') }}
          </Button>
        </div>
      </TabsContent>

      <!-- WOL Tab -->
      <TabsContent value="wol" class="mt-3 space-y-3">
        <p class="text-xs text-muted-foreground">
          {{ t('atx.wolDescription') }}
        </p>

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

        <!-- History -->
        <div v-if="wolHistory.length > 0" class="space-y-2">
          <Separator />
          <Label class="text-xs text-muted-foreground">{{ t('atx.recentMac') }}</Label>
          <div class="space-y-1">
            <button
              v-for="mac in wolHistory"
              :key="mac"
              class="w-full text-left px-2 py-1.5 rounded text-xs font-mono hover:bg-muted transition-colors"
              @click="selectFromHistory(mac)"
            >
              {{ mac }}
            </button>
          </div>
        </div>
      </TabsContent>
    </Tabs>
  </div>

  <!-- Confirm Dialog -->
  <AlertDialog :open="!!confirmAction" @update:open="confirmAction = null">
    <AlertDialogContent>
      <AlertDialogHeader>
        <AlertDialogTitle>{{ confirmTitle }}</AlertDialogTitle>
        <AlertDialogDescription>{{ confirmDescription }}</AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel>{{ t('common.cancel') }}</AlertDialogCancel>
        <AlertDialogAction @click="handleAction">{{ t('common.confirm') }}</AlertDialogAction>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
