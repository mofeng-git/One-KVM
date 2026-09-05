<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown, AlertCircle, Info } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import type { StatusDetail, ConnectionStatus } from '@/components/StatusCard.vue'

export interface ConsoleStatusItem {
  id: string
  title: string
  status: ConnectionStatus
  quickInfo: string
  details: StatusDetail[]
  errorMessage?: string
  required?: boolean
}

const props = defineProps<{
  items: ConsoleStatusItem[]
  showVideoInfo?: boolean
}>()
const { t } = useI18n()
const issues = computed(() => props.items.filter(item =>
  ['error', 'no_signal', 'busy'].includes(item.status) || (item.required && item.status !== 'connected'),
))
const summary = computed(() => issues.value.length
  ? issues.value.map(item => `${item.title}: ${t(`status.${item.status}`)}`).join(' · ')
  : t('status.connected'))
const videoInfo = computed(() => props.items.find(item => item.id === 'video')?.quickInfo)
const hasError = computed(() => issues.value.some(item => item.status === 'error'))

function dotClass(status: ConsoleStatusItem['status']) {
  return {
    connected: 'bg-status-active',
    connecting: 'bg-warning animate-pulse',
    disconnected: 'bg-muted-foreground',
    error: 'bg-destructive',
    no_signal: 'bg-warning',
    busy: 'bg-warning',
  }[status]
}
</script>

<template>
  <Popover>
    <PopoverTrigger as-child>
      <Button
        variant="ghost"
        size="sm"
        class="min-w-0 shrink gap-1.5 px-2 text-xs"
        :class="hasError ? 'text-destructive' : issues.length ? 'text-warning' : ''"
        :aria-label="`${t('statusCard.connectionDetails')}: ${summary}`"
        :title="summary"
      >
        <AlertCircle v-if="hasError" class="size-3.5 shrink-0" />
        <Info v-else-if="issues.length" class="size-3.5 shrink-0" />
        <span v-else class="size-2 shrink-0 rounded-full bg-status-active" />
        <span class="truncate" role="status">{{ summary }}</span>
        <span v-if="showVideoInfo && !issues.length" class="hidden text-muted-foreground md:inline">{{ videoInfo }}</span>
        <ChevronDown class="size-3 shrink-0" />
      </Button>
    </PopoverTrigger>
    <PopoverContent align="start" class="w-[min(360px,calc(100vw-1rem))] max-h-[70dvh] overflow-y-auto">
      <h2 class="mb-3 text-sm font-semibold">{{ t('statusCard.connectionDetails') }}</h2>
      <div class="divide-y">
        <section v-for="item in items" :key="item.id" class="space-y-2 py-3 first:pt-0 last:pb-0">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span class="font-medium">{{ item.title }}</span>
            <span class="flex items-center gap-1.5 text-xs">
              <span class="size-1.5 rounded-full" :class="dotClass(item.status)" />
              {{ t(`status.${item.status}`) }}
            </span>
          </div>
          <p v-if="item.quickInfo" class="text-xs text-muted-foreground">{{ item.quickInfo }}</p>
          <p v-if="item.errorMessage" class="break-words text-xs" :class="item.status === 'error' ? 'text-destructive' : 'text-muted-foreground'">{{ item.errorMessage }}</p>
          <dl class="space-y-1 text-xs">
            <div v-for="(detail, index) in item.details" :key="index" class="flex justify-between gap-4">
              <dt class="shrink-0 text-muted-foreground">{{ detail.label }}</dt>
              <dd class="min-w-0 break-words text-right" :class="detail.status === 'error' ? 'text-destructive' : detail.status === 'warning' ? 'text-warning' : ''">{{ detail.value }}</dd>
            </div>
          </dl>
        </section>
      </div>
    </PopoverContent>
  </Popover>
</template>
