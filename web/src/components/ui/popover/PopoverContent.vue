<script setup lang="ts">
import { type HTMLAttributes, computed } from 'vue'
import {
  PopoverContent,
  PopoverPortal,
  type PopoverContentProps,
} from 'reka-ui'
import { cn } from '@/lib/utils'

const props = defineProps<PopoverContentProps & { class?: HTMLAttributes['class'] }>()

const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
</script>

<template>
  <PopoverPortal>
    <PopoverContent
      v-bind="delegatedProps"
      :class="cn(
        'z-50 w-72 rounded-md border bg-popover p-4 text-popover-foreground shadow-md outline-none data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2',
        props.class
      )"
    >
      <slot />
    </PopoverContent>
  </PopoverPortal>
</template>
