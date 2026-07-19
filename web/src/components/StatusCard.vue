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
import { Button } from '@/components/ui/button'
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
      return 'bg-success'
    case 'connecting':
      return 'bg-warning animate-pulse'
    case 'disconnected':
      return 'bg-muted-foreground'
    case 'error':
      return 'bg-destructive'
    default:
      return 'bg-muted-foreground'
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
      <Button
        type="button"
        variant="outline"
        :aria-label="`${title}: ${quickInfo || subtitle || statusText}`"
        :class="cn(
          'h-auto flex-col items-start gap-0.5 text-left',
          compact ? 'px-1.5 py-0.5 text-xs' : 'px-3 py-1.5 text-sm min-w-[100px]',
          status === 'error' && 'border-destructive/50'
        )"
      >
        <template v-if="compact">
          <!-- Compact: single row with dot + abbreviated title -->
          <div class="flex items-center gap-1">
            <span :class="cn('size-1.5 rounded-full shrink-0', statusColor)" />
            <span class="text-[10px] text-muted-foreground leading-tight truncate">{{ title }}</span>
          </div>
        </template>
        <template v-else>
          <!-- Top: Title -->
          <span class="font-medium text-foreground text-xs truncate">{{ title }}</span>
          <!-- Bottom: Status dot + Quick info -->
          <div class="flex items-center gap-1.5">
            <span :class="cn('size-2 rounded-full shrink-0', statusColor)" />
            <span class="text-[11px] text-muted-foreground leading-tight truncate">
              {{ quickInfo || subtitle || statusText }}
            </span>
          </div>
        </template>
      </Button>
    </HoverCardTrigger>

    <HoverCardContent class="w-80" :align="hoverAlign">
      <div class="space-y-3">
        <!-- Header -->
        <div class="flex items-center gap-3">
          <div :class="cn(
            'rounded-md p-2',
            status === 'connected' ? 'bg-success/10 text-success' :
            status === 'error' ? 'bg-destructive/10 text-destructive' :
            'bg-muted text-muted-foreground'
          )">
            <component :is="StatusIcon" class="size-5" />
          </div>
          <div class="flex-1 min-w-0">
            <h4 class="font-semibold text-sm">{{ title }}</h4>
            <div class="flex items-center gap-1.5 mt-0.5">
              <component
                v-if="statusIcon"
                :is="statusIcon"
                :class="cn(
                  'size-3.5',
                  status === 'connected' ? 'text-success' :
                  status === 'connecting' ? 'text-warning animate-spin' :
                  status === 'error' ? 'text-destructive' :
                  'text-muted-foreground'
                )"
              />
              <Badge
                :variant="status === 'connected' ? 'success' : status === 'connecting' ? 'warning' : status === 'error' ? 'destructive' : 'secondary'"
                class="text-[10px] px-1.5 py-0"
              >
                {{ statusBadgeText }}
              </Badge>
            </div>
          </div>
        </div>

        <!-- Details -->
        <div v-if="details && details.length > 0" class="space-y-2">
          <Separator />
          <div class="space-y-1">
            <div
              v-for="(detail, index) in details"
              :key="index"
              class="flex items-baseline justify-between gap-3 py-0.5 text-xs"
            >
              <span class="shrink-0 text-muted-foreground">{{ detail.label }}</span>
              <span :class="cn('min-w-0 text-right text-xs font-medium', detail.status === 'ok' ? 'text-success' : detail.status === 'warning' ? 'text-warning' : detail.status === 'error' ? 'text-destructive' : 'text-foreground')">{{ detail.value }}</span>
            </div>
          </div>
        </div>
      </div>
    </HoverCardContent>
  </HoverCard>

  <Popover v-else>
    <PopoverTrigger as-child>
      <!-- New layout: vertical with title on top, status+quickInfo on bottom -->
      <Button
        type="button"
        variant="outline"
        :aria-label="`${title}: ${quickInfo || subtitle || statusText}`"
        :class="cn(
          'h-auto flex-col items-start gap-0.5 text-left',
          compact ? 'px-1.5 py-0.5 text-xs' : 'px-3 py-1.5 text-sm min-w-[100px]',
          status === 'error' && 'border-destructive/50'
        )"
      >
        <template v-if="compact">
          <!-- Compact: single row with dot + abbreviated title -->
          <div class="flex items-center gap-1">
            <span :class="cn('size-1.5 rounded-full shrink-0', statusColor)" />
            <span class="text-[10px] text-muted-foreground leading-tight truncate">{{ title }}</span>
          </div>
        </template>
        <template v-else>
          <!-- Top: Title -->
          <span class="font-medium text-foreground text-xs truncate">{{ title }}</span>
          <!-- Bottom: Status dot + Quick info -->
          <div class="flex items-center gap-1.5">
            <span :class="cn('size-2 rounded-full shrink-0', statusColor)" />
            <span class="text-[11px] text-muted-foreground leading-tight truncate">
              {{ quickInfo || subtitle || statusText }}
            </span>
          </div>
        </template>
      </Button>
    </PopoverTrigger>

    <PopoverContent class="w-[min(320px,90vw)]" :align="hoverAlign">
      <div class="space-y-3">
        <!-- Header -->
        <div class="flex items-center gap-3">
          <div :class="cn(
            'rounded-md p-2',
            status === 'connected' ? 'bg-success/10 text-success' :
            status === 'error' ? 'bg-destructive/10 text-destructive' :
            'bg-muted text-muted-foreground'
          )">
            <component :is="StatusIcon" class="size-5" />
          </div>
          <div class="flex-1 min-w-0">
            <h4 class="font-semibold text-sm">{{ title }}</h4>
            <div class="flex items-center gap-1.5 mt-0.5">
              <component
                v-if="statusIcon"
                :is="statusIcon"
                :class="cn(
                  'size-3.5',
                  status === 'connected' ? 'text-success' :
                  status === 'connecting' ? 'text-warning animate-spin' :
                  status === 'error' ? 'text-destructive' :
                  'text-muted-foreground'
                )"
              />
              <Badge
                :variant="status === 'connected' ? 'success' : status === 'connecting' ? 'warning' : status === 'error' ? 'destructive' : 'secondary'"
                class="text-[10px] px-1.5 py-0"
              >
                {{ statusBadgeText }}
              </Badge>
            </div>
          </div>
        </div>

        <!-- Details -->
        <div v-if="details && details.length > 0" class="space-y-2">
          <Separator />
          <div class="space-y-1">
            <div
              v-for="(detail, index) in details"
              :key="index"
              class="flex items-baseline justify-between gap-3 py-0.5 text-xs"
            >
              <span class="shrink-0 text-muted-foreground">{{ detail.label }}</span>
              <span :class="cn('min-w-0 text-right text-xs font-medium', detail.status === 'ok' ? 'text-success' : detail.status === 'warning' ? 'text-warning' : detail.status === 'error' ? 'text-destructive' : 'text-foreground')">{{ detail.value }}</span>
            </div>
          </div>
        </div>
      </div>
    </PopoverContent>
  </Popover>
</template>
