<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(
  defineProps<{
    size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
    class?: string
  }>(),
  { size: 'md' },
)

const dim = computed(() => {
  switch (props.size) {
    case 'xs':
      return 'h-4 w-4'
    case 'sm':
      return 'h-5 w-5'
    case 'md':
      return 'h-6 w-6'
    case 'lg':
      return 'h-10 w-10'
    case 'xl':
      return 'h-14 w-14'
    default:
      return 'h-6 w-6'
  }
})
</script>

<template>
  <img
    src="/favicon.png"
    alt=""
    :class="cn('shrink-0 object-contain select-none', dim, props.class)"
    decoding="async"
    draggable="false"
  />
</template>
