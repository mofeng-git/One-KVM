<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { AlertTriangle, RefreshCw } from 'lucide-vue-next'
import type { VideoDevice, VideoFormat, VideoResolution } from '@/api'
import { formatFpsLabel } from '@/lib/fps'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const props = defineProps<{
  device?: VideoDevice
  formats: Array<VideoFormat & { disabled?: boolean }>
  resolutions: VideoResolution[]
  fpsOptions: number[]
  format: string
  resolution: string
  fps: number | null
  compact?: boolean
  resolutionFpsInline?: boolean
  refreshing?: boolean
}>()

const emit = defineEmits<{
  (event: 'update:format', value: string): void
  (event: 'update:resolution', value: string): void
  (event: 'update:fps', value: number): void
  (event: 'refresh'): void
}>()

const { t } = useI18n()
</script>

<template>
  <template v-if="device?.control_mode === 'source_following'">
    <div v-if="device.input_status.state === 'locked'" class="space-y-2">
      <dl class="grid grid-cols-3 gap-3 text-center" :class="compact ? 'text-xs' : 'text-sm'">
        <div class="min-w-0 space-y-1">
          <dt class="text-muted-foreground">{{ t('videoInput.format') }}</dt>
          <dd class="truncate font-medium" :title="device.input_status.format ?? ''">
            {{ device.input_status.format ?? '—' }}
          </dd>
        </div>
        <div class="min-w-0 space-y-1">
          <dt class="text-muted-foreground">{{ t('videoInput.resolution') }}</dt>
          <dd class="whitespace-nowrap font-medium">
            {{ device.input_status.width }}x{{ device.input_status.height }}
          </dd>
        </div>
        <div class="min-w-0 space-y-1">
          <dt class="text-muted-foreground">{{ t('videoInput.frameRate') }}</dt>
          <dd class="whitespace-nowrap font-medium">
            {{ device.input_status.fps === null ? '—' : formatFpsLabel(device.input_status.fps) }}
          </dd>
        </div>
      </dl>
    </div>

    <div
      v-else-if="device.input_status.state === 'no_signal'"
      class="flex items-center gap-2 text-warning"
      :class="compact ? 'text-xs' : 'text-sm'"
      role="status"
    >
      <AlertTriangle class="size-4 shrink-0" />
      <span>{{ t('videoInput.noSignal') }}</span>
    </div>

    <div v-else class="flex items-center justify-between gap-3" role="status">
      <div class="flex min-w-0 items-center gap-2 text-muted-foreground" :class="compact ? 'text-xs' : 'text-sm'">
        <AlertTriangle class="size-4 shrink-0" />
        <span>{{ t('videoInput.unavailable') }}</span>
      </div>
      <Button
        type="button"
        variant="outline"
        :size="compact ? 'icon-xs' : 'icon'"
        :disabled="refreshing"
        :title="t('videoInput.refresh')"
        :aria-label="t('videoInput.refresh')"
        @click="emit('refresh')"
      >
        <RefreshCw :class="['size-4', refreshing && 'animate-spin']" />
      </Button>
    </div>
  </template>

  <template v-else-if="device">
    <div class="space-y-2">
      <Label :class="compact ? 'text-xs text-muted-foreground' : undefined">{{ t('videoInput.format') }}</Label>
      <Select :model-value="format" @update:model-value="value => emit('update:format', String(value))">
        <SelectTrigger :size="compact ? 'sm' : 'default'" class="w-full" :class="compact ? 'text-xs' : undefined">
          <SelectValue :placeholder="t('videoInput.selectFormat')" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="item in formats" :key="item.format" :value="item.format" :disabled="item.disabled">
            {{ item.description || item.format }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <div :class="resolutionFpsInline ? 'grid grid-cols-2 gap-3' : 'space-y-4'">
      <div class="space-y-2">
        <Label :class="compact ? 'text-xs text-muted-foreground' : undefined">{{ t('videoInput.resolution') }}</Label>
        <Select :model-value="resolution" @update:model-value="value => emit('update:resolution', String(value))">
          <SelectTrigger :size="compact ? 'sm' : 'default'" class="w-full" :class="compact ? 'text-xs' : undefined">
            <SelectValue :placeholder="t('videoInput.selectResolution')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="item in resolutions" :key="`${item.width}x${item.height}`" :value="`${item.width}x${item.height}`">
              {{ item.width }}x{{ item.height }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div class="space-y-2">
        <Label :class="compact ? 'text-xs text-muted-foreground' : undefined">{{ t('videoInput.frameRate') }}</Label>
        <Select :model-value="fps === null ? '' : String(fps)" @update:model-value="value => emit('update:fps', Number(value))">
          <SelectTrigger :size="compact ? 'sm' : 'default'" class="w-full" :class="compact ? 'text-xs' : undefined">
            <SelectValue :placeholder="t('videoInput.selectFps')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="item in fpsOptions" :key="item" :value="String(item)">
              {{ formatFpsLabel(item) }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  </template>
</template>
