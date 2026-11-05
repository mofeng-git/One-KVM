<script setup lang="ts">
import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'
import { Textarea } from '@/components/ui/textarea'

defineProps<{
  disabled: boolean
  quickLabel: string
  fullLabel: string
  fullConfigHint: string
  fullConfigRequired: string
}>()

const mode = defineModel<'quick' | 'full'>('mode', { required: true })
const config = defineModel<string>('config', { required: true })
</script>

<template>
  <ButtonGroup class="grid w-full grid-cols-2">
    <Button
      type="button"
      :variant="mode === 'quick' ? 'default' : 'outline'"
      :disabled="disabled"
      @click="mode = 'quick'"
    >
      {{ quickLabel }}
    </Button>
    <Button
      type="button"
      :variant="mode === 'full' ? 'default' : 'outline'"
      :disabled="disabled"
      @click="mode = 'full'"
    >
      {{ fullLabel }}
    </Button>
  </ButtonGroup>

  <slot v-if="mode === 'quick'" name="quick" />
  <div v-else class="space-y-1">
    <Textarea
      v-model="config"
      class="min-h-[300px] font-mono text-xs"
      spellcheck="false"
      :disabled="disabled"
    />
    <p class="text-xs text-muted-foreground">{{ fullConfigHint }}</p>
    <p v-if="!config.trim()" class="text-xs text-destructive">{{ fullConfigRequired }}</p>
  </div>
</template>
