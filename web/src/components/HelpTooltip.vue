<script setup lang="ts">
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { HelpCircle } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'

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
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          class="text-muted-foreground/60 hover:text-muted-foreground"
          :class="iconSize === 'sm' ? 'size-3.5' : 'size-4'"
        >
          <HelpCircle :class="iconSize === 'sm' ? 'size-3.5' : 'size-4'" />
        </Button>
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
