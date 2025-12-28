<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { AlertDialogCancel, type AlertDialogCancelProps } from 'reka-ui'
import { cn } from '@/lib/utils'
import { buttonVariants } from '@/components/ui/button'

const props = defineProps<AlertDialogCancelProps & { class?: HTMLAttributes['class'] }>()
</script>

<template>
  <AlertDialogCancel :class="cn(buttonVariants({ variant: 'outline' }), 'mt-2 sm:mt-0', props.class)">
    <slot />
  </AlertDialogCancel>
</template>
