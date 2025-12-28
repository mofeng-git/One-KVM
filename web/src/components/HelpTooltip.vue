<script setup lang="ts">
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { HelpCircle } from 'lucide-vue-next'

defineProps<{
  content: string
  side?: 'top' | 'right' | 'bottom' | 'left'
  align?: 'start' | 'center' | 'end'
  iconSize?: 'sm' | 'md'
}>()
</script>

<template>
  <TooltipProvider :delay-duration="200">
    <Tooltip>
      <TooltipTrigger as-child>
        <button
          type="button"
          class="inline-flex items-center justify-center text-muted-foreground/60 hover:text-muted-foreground transition-colors focus:outline-none"
          :class="iconSize === 'sm' ? 'h-3.5 w-3.5' : 'h-4 w-4'"
        >
          <HelpCircle :class="iconSize === 'sm' ? 'h-3.5 w-3.5' : 'h-4 w-4'" />
        </button>
      </TooltipTrigger>
      <TooltipContent
        :side="side || 'top'"
        :align="align || 'center'"
        class="max-w-[280px] text-xs leading-relaxed"
      >
        <p>{{ content }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
