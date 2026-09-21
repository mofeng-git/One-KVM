<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Keyboard, Maximize, Scaling } from 'lucide-vue-next'
import type { VideoScaleMode } from '@/composables/useVideoScaling'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

const props = defineProps<{
  minimal?: boolean
  textOnlyScale?: boolean
  scaleMode?: VideoScaleMode
  sourceSizeAvailable?: boolean
}>()

const emit = defineEmits<{
  (e: 'toggleFullscreen'): void
  (e: 'toggleVirtualKeyboard'): void
  (e: 'update:scaleMode', mode: VideoScaleMode): void
}>()

const { t } = useI18n()

function toggleScaleMode() {
  if (!props.sourceSizeAvailable) return
  emit('update:scaleMode', props.scaleMode === 'actual' ? 'fit' : 'actual')
}
</script>

<template>
  <TooltipProvider>
    <Tooltip>
      <TooltipTrigger as-child>
        <Button
          data-fixed-action
          variant="ghost"
          size="sm"
          class="size-8 p-0 text-xs sm:w-auto sm:gap-1.5 sm:px-2"
          :aria-label="t('actionbar.fullscreen')"
          @click="emit('toggleFullscreen')"
        >
          <Maximize class="size-3.5 sm:size-4" />
          <span class="hidden xl:inline">{{ t('actionbar.fullscreen') }}</span>
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        <p>{{ t('actionbar.fullscreenTip') }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>

  <TooltipProvider v-if="!minimal">
    <Tooltip>
      <TooltipTrigger as-child>
        <span data-fixed-action class="inline-flex">
          <Button
            :variant="props.scaleMode === 'actual' ? 'secondary' : 'ghost'"
            size="sm"
            class="min-w-10 rounded-none px-2 text-xs tabular-nums"
            :disabled="!props.sourceSizeAvailable"
            :aria-label="t(props.scaleMode === 'actual' ? 'actionbar.fitSizeAria' : 'actionbar.actualSizeAria')"
            :aria-pressed="props.scaleMode === 'actual'"
            @click="toggleScaleMode"
          >
            <Scaling v-if="!textOnlyScale" class="size-3.5" />
            1:1
          </Button>
        </span>
      </TooltipTrigger>
      <TooltipContent>
        <p>{{ t(props.scaleMode === 'actual' ? 'actionbar.fitSizeTip' : 'actionbar.actualSizeTip') }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>

  <TooltipProvider v-if="!minimal">
    <Tooltip>
      <TooltipTrigger as-child>
        <Button
          data-fixed-action
          variant="ghost"
          size="sm"
          class="size-8 p-0 text-xs sm:w-auto sm:gap-1.5 sm:px-2"
          :aria-label="t('actionbar.keyboard')"
          @click="emit('toggleVirtualKeyboard')"
        >
          <Keyboard class="size-3.5 sm:size-4" />
          <span class="hidden xl:inline">{{ t('actionbar.keyboard') }}</span>
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        <p>{{ t('actionbar.keyboardTip') }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>

  <div
    v-if="!minimal"
    data-fixed-action
    aria-hidden="true"
    class="mx-2 h-5 w-px shrink-0 self-center bg-border"
  />
</template>
