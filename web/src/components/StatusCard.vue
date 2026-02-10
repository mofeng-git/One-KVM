<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { cn } from '@/lib/utils'
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from '@/components/ui/hover-card'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { Monitor, Video, Usb, AlertCircle, CheckCircle, Loader2, Volume2, HardDrive } from 'lucide-vue-next'

const { t } = useI18n()

export interface StatusDetail {
  label: string
  value: string
  status?: 'ok' | 'warning' | 'error'
}

const props = withDefaults(defineProps<{
  title: string
  type: 'device' | 'video' | 'hid' | 'audio' | 'msd'
  status: 'connected' | 'connecting' | 'disconnected' | 'error'
  quickInfo?: string  // Quick info displayed on trigger (e.g., "1920x1080 30fps")
  subtitle?: string
  errorMessage?: string
  details?: StatusDetail[]
  hoverAlign?: 'start' | 'center' | 'end'  // HoverCard alignment
  compact?: boolean
}>(), {
  hoverAlign: 'start',
  compact: false,
})

const prefersPopover = ref(false)

onMounted(() => {
  const hasTouch = 'ontouchstart' in window || navigator.maxTouchPoints > 0
  const coarsePointer = window.matchMedia?.('(pointer: coarse)')?.matches
  prefersPopover.value = hasTouch || !!coarsePointer
})

const statusColor = computed(() => {
  switch (props.status) {
    case 'connected':
      return 'bg-green-500'
    case 'connecting':
      return 'bg-yellow-500 animate-pulse'
    case 'disconnected':
      return 'bg-slate-400'
    case 'error':
      return 'bg-red-500'
    default:
      return 'bg-slate-400'
  }
})

const StatusIcon = computed(() => {
  switch (props.type) {
    case 'device':
      return Monitor
    case 'video':
      return Video
    case 'hid':
      return Usb
    case 'audio':
      return Volume2
    case 'msd':
      return HardDrive
    default:
      return Monitor
  }
})

const statusIcon = computed(() => {
  switch (props.status) {
    case 'connected':
      return CheckCircle
    case 'connecting':
      return Loader2
    case 'error':
      return AlertCircle
    default:
      return null
  }
})

// Localized status text
const statusText = computed(() => {
  switch (props.status) {
    case 'connected':
      return t('status.connected')
    case 'connecting':
      return t('status.connecting')
    case 'disconnected':
      return t('status.disconnected')
    case 'error':
      return t('status.error')
    default:
      return props.status
  }
})

// Localized status badge text (for hover card)
const statusBadgeText = computed(() => {
  switch (props.status) {
    case 'connected':
      return t('statusCard.online')
    case 'connecting':
      return t('statusCard.connecting')
    case 'disconnected':
      return t('statusCard.offline')
    case 'error':
      return t('status.error')
    default:
      return props.status
  }
})
</script>

<template>
  <HoverCard v-if="!prefersPopover" :open-delay="200" :close-delay="100">
    <HoverCardTrigger as-child>
      <!-- New layout: vertical with title on top, status+quickInfo on bottom -->
      <div
        :class="cn(
          'flex flex-col gap-0.5 rounded-md border cursor-pointer transition-colors',
          compact ? 'px-2 py-1 text-xs min-w-[80px]' : 'px-3 py-1.5 text-sm min-w-[100px]',
          'bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700',
          'border-slate-200 dark:border-slate-700',
          status === 'error' && 'border-red-300 dark:border-red-800'
        )"
      >
        <!-- Top: Title -->
        <span class="font-medium text-foreground text-xs truncate">{{ title }}</span>
        <!-- Bottom: Status dot + Quick info -->
        <div class="flex items-center gap-1.5">
          <span :class="cn('h-2 w-2 rounded-full shrink-0', statusColor)" />
          <span class="text-[11px] text-muted-foreground leading-tight truncate">
            {{ quickInfo || subtitle || statusText }}
          </span>
        </div>
      </div>
    </HoverCardTrigger>

    <HoverCardContent class="w-80" :align="hoverAlign">
      <div class="space-y-3">
        <!-- Header -->
        <div class="flex items-center gap-3">
          <div :class="cn(
            'p-2 rounded-lg',
            status === 'connected' ? 'bg-green-100 text-green-600 dark:bg-green-900/30 dark:text-green-400' :
            status === 'error' ? 'bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400' :
            'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-400'
          )">
            <component :is="StatusIcon" class="h-5 w-5" />
          </div>
          <div class="flex-1 min-w-0">
            <h4 class="font-semibold text-sm">{{ title }}</h4>
            <div class="flex items-center gap-1.5 mt-0.5">
              <component
                v-if="statusIcon"
                :is="statusIcon"
                :class="cn(
                  'h-3.5 w-3.5',
                  status === 'connected' ? 'text-green-500' :
                  status === 'connecting' ? 'text-yellow-500 animate-spin' :
                  status === 'error' ? 'text-red-500' :
                  'text-slate-400'
                )"
              />
              <Badge
                :variant="status === 'connected' ? 'default' : status === 'error' ? 'destructive' : 'secondary'"
                class="text-[10px] px-1.5 py-0"
              >
                {{ statusBadgeText }}
              </Badge>
            </div>
          </div>
        </div>

        <!-- Error Message -->
        <div
          v-if="status === 'error' && errorMessage"
          class="p-2 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800"
        >
          <p class="text-xs text-red-600 dark:text-red-400">
            <AlertCircle class="h-3.5 w-3.5 inline mr-1" />
            {{ errorMessage }}
          </p>
        </div>

        <!-- Details -->
        <div v-if="details && details.length > 0" class="space-y-2">
          <Separator />
          <div class="space-y-1.5">
            <div
              v-for="(detail, index) in details"
              :key="index"
              class="flex items-center justify-between text-xs"
            >
              <span class="text-muted-foreground">{{ detail.label }}</span>
              <span
                :class="cn(
                  'font-medium',
                  detail.status === 'ok' ? 'text-green-600 dark:text-green-400' :
                  detail.status === 'warning' ? 'text-yellow-600 dark:text-yellow-400' :
                  detail.status === 'error' ? 'text-red-600 dark:text-red-400' :
                  'text-foreground'
                )"
              >
                {{ detail.value }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </HoverCardContent>
  </HoverCard>

  <Popover v-else>
    <PopoverTrigger as-child>
      <!-- New layout: vertical with title on top, status+quickInfo on bottom -->
      <div
        :class="cn(
          'flex flex-col gap-0.5 rounded-md border cursor-pointer transition-colors',
          compact ? 'px-2 py-1 text-xs min-w-[80px]' : 'px-3 py-1.5 text-sm min-w-[100px]',
          'bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700',
          'border-slate-200 dark:border-slate-700',
          status === 'error' && 'border-red-300 dark:border-red-800'
        )"
      >
        <!-- Top: Title -->
        <span class="font-medium text-foreground text-xs truncate">{{ title }}</span>
        <!-- Bottom: Status dot + Quick info -->
        <div class="flex items-center gap-1.5">
          <span :class="cn('h-2 w-2 rounded-full shrink-0', statusColor)" />
          <span class="text-[11px] text-muted-foreground leading-tight truncate">
            {{ quickInfo || subtitle || statusText }}
          </span>
        </div>
      </div>
    </PopoverTrigger>

    <PopoverContent class="w-80" :align="hoverAlign">
      <div class="space-y-3">
        <!-- Header -->
        <div class="flex items-center gap-3">
          <div :class="cn(
            'p-2 rounded-lg',
            status === 'connected' ? 'bg-green-100 text-green-600 dark:bg-green-900/30 dark:text-green-400' :
            status === 'error' ? 'bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400' :
            'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-400'
          )">
            <component :is="StatusIcon" class="h-5 w-5" />
          </div>
          <div class="flex-1 min-w-0">
            <h4 class="font-semibold text-sm">{{ title }}</h4>
            <div class="flex items-center gap-1.5 mt-0.5">
              <component
                v-if="statusIcon"
                :is="statusIcon"
                :class="cn(
                  'h-3.5 w-3.5',
                  status === 'connected' ? 'text-green-500' :
                  status === 'connecting' ? 'text-yellow-500 animate-spin' :
                  status === 'error' ? 'text-red-500' :
                  'text-slate-400'
                )"
              />
              <Badge
                :variant="status === 'connected' ? 'default' : status === 'error' ? 'destructive' : 'secondary'"
                class="text-[10px] px-1.5 py-0"
              >
                {{ statusBadgeText }}
              </Badge>
            </div>
          </div>
        </div>

        <!-- Error Message -->
        <div
          v-if="status === 'error' && errorMessage"
          class="p-2 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800"
        >
          <p class="text-xs text-red-600 dark:text-red-400">
            <AlertCircle class="h-3.5 w-3.5 inline mr-1" />
            {{ errorMessage }}
          </p>
        </div>

        <!-- Details -->
        <div v-if="details && details.length > 0" class="space-y-2">
          <Separator />
          <div class="space-y-1.5">
            <div
              v-for="(detail, index) in details"
              :key="index"
              class="flex items-center justify-between text-xs"
            >
              <span class="text-muted-foreground">{{ detail.label }}</span>
              <span
                :class="cn(
                  'font-medium',
                  detail.status === 'ok' ? 'text-green-600 dark:text-green-400' :
                  detail.status === 'warning' ? 'text-yellow-600 dark:text-yellow-400' :
                  detail.status === 'error' ? 'text-red-600 dark:text-red-400' :
                  'text-foreground'
                )"
              >
                {{ detail.value }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </PopoverContent>
  </Popover>
</template>
