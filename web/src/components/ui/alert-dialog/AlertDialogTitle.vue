<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { AlertDialogTitle, type AlertDialogTitleProps } from 'reka-ui'
import { cn } from '@/lib/utils'

const props = defineProps<AlertDialogTitleProps & { class?: HTMLAttributes['class'] }>()
</script>

<template>
  <AlertDialogTitle :class="cn('text-lg font-semibold', props.class)">
    <slot />
  </AlertDialogTitle>
</template>
