<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { AlertDialogDescription, type AlertDialogDescriptionProps } from 'reka-ui'
import { cn } from '@/lib/utils'

const props = defineProps<AlertDialogDescriptionProps & { class?: HTMLAttributes['class'] }>()
</script>

<template>
  <AlertDialogDescription :class="cn('text-sm text-muted-foreground', props.class)">
    <slot />
  </AlertDialogDescription>
</template>
