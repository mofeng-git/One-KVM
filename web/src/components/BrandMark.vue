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
      return 'size-4'
    case 'sm':
      return 'size-5'
    case 'md':
      return 'size-6'
    case 'lg':
      return 'size-10'
    case 'xl':
      return 'size-14'
    default:
      return 'size-6'
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
