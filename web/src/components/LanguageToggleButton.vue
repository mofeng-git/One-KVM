<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { HTMLAttributes } from 'vue'
import type { ButtonVariants } from '@/components/ui/button'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { setLanguage } from '@/i18n'
import { Languages } from 'lucide-vue-next'

interface Props {
  class?: HTMLAttributes['class']
  size?: ButtonVariants['size']
  variant?: ButtonVariants['variant']
  labelMode?: 'hidden' | 'current' | 'next'
}

const props = withDefaults(defineProps<Props>(), {
  size: 'icon',
  variant: 'ghost',
  labelMode: 'hidden',
})

const { t, locale } = useI18n()

const currentLanguageLabel = computed(() => (locale.value === 'zh-CN' ? '中文' : 'English'))
const nextLanguageLabel = computed(() => (locale.value === 'zh-CN' ? 'English' : '中文'))
const buttonLabel = computed(() => (
  props.labelMode === 'current' ? currentLanguageLabel.value : nextLanguageLabel.value
))

function toggleLanguage() {
  const newLang = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN'
  setLanguage(newLang)
}
</script>

<template>
  <Button
    :variant="variant"
    :size="size"
    :class="cn(props.labelMode !== 'hidden' && 'gap-2', props.class)"
    :aria-label="t('common.toggleLanguage')"
    @click="toggleLanguage"
  >
    <Languages class="h-4 w-4" />
    <span v-if="props.labelMode !== 'hidden'">{{ buttonLabel }}</span>
    <span class="sr-only">{{ t('common.toggleLanguage') }}</span>
  </Button>
</template>
