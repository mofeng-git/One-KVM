<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Command, CircleHelp, Loader2, X } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { focusConsolePanel } from '@/composables/useConsoleAppearance'
import {
  keyboardShortcuts, sysrqDiagnostics, sysrqRecovery,
  type KeyboardShortcut,
} from '@/lib/keyboardShortcuts'

const props = defineProps<{
  ready: boolean
  inputBusy?: boolean
  busy?: boolean
  side?: 'right' | 'bottom'
}>()
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{
  execute: [shortcut: KeyboardShortcut]
  cancel: []
}>()
const { t } = useI18n()
const disabled = computed(() => !props.ready || props.inputBusy || props.busy)
const groups = [keyboardShortcuts, sysrqDiagnostics, sysrqRecovery]

function label(shortcut: KeyboardShortcut) {
  return shortcut.id.startsWith('sysrq-') ? `Alt+SysRq+${shortcut.label}` : shortcut.label
}
function description(shortcut: KeyboardShortcut) {
  return `${t(`shortcuts.actions.${shortcut.description}`)} · ${label(shortcut)}`
}
function select(shortcut: KeyboardShortcut) {
  if (disabled.value) return
  emit('execute', shortcut)
}
watch(open, value => {
  if (!value && props.busy) emit('cancel')
})
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        data-fixed-action data-shortcut-surface
        variant="ghost" size="sm"
        class="size-8 p-0 text-xs sm:w-auto sm:gap-1.5 sm:px-2"
        :aria-label="t('actionbar.shortcuts')" :title="t('actionbar.shortcuts')"
      >
        <Loader2 v-if="busy" class="size-3.5 animate-spin sm:size-4" />
        <Command v-else class="size-3.5 sm:size-4" />
        <span class="hidden xl:inline">{{ t('actionbar.shortcuts') }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent
      data-shortcut-surface data-keyboard-shortcuts
      class="console-config-panel shortcuts-panel w-[330px] max-h-[min(80dvh,var(--reka-popover-content-available-height))] overflow-y-auto p-3.5"
      align="end" :side="side ?? 'bottom'" @open-auto-focus="focusConsolePanel"
    >
      <div class="mb-3 flex items-center justify-between">
        <h4 class="text-sm font-medium">{{ t('actionbar.shortcuts') }}</h4>
        <button type="button" class="text-muted-foreground" :aria-label="t('shortcuts.close')" @click="open = false"><X class="size-4" /></button>
      </div>
      <template v-for="(group, index) in groups" :key="index">
        <div v-if="index === 1" class="mt-3 flex items-center justify-between border-t pt-3 text-[13px] text-muted-foreground">
          <span>Alt + SysRq +</span>
          <a
            href="https://docs.kernel.org/admin-guide/sysrq.html" target="_blank" rel="noopener noreferrer"
            :title="t('shortcuts.sysrqHelp')" :aria-label="t('shortcuts.sysrqHelp')"
          ><CircleHelp class="size-3.5" /></a>
        </div>
        <div class="grid gap-1.5" :class="index === 0 ? 'grid-cols-2' : index === 1 ? 'mt-2 grid-cols-4' : 'mt-1.5 grid-cols-6'">
          <button
            v-for="shortcut in group" :key="shortcut.id" type="button"
            class="shortcut-key" :class="{ 'shortcut-key--danger': shortcut.destructive }"
            :data-shortcut="shortcut.id" :disabled="disabled"
            :title="!ready ? t('shortcuts.unavailable') : inputBusy ? t('shortcuts.releaseKeys') : description(shortcut)"
            :aria-label="description(shortcut)" @click="select(shortcut)"
          >{{ shortcut.label }}</button>
        </div>
      </template>
    </PopoverContent>
  </Popover>
</template>

<style scoped>
.shortcut-key {
  height: 35px;
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: color-mix(in srgb, var(--muted) 45%, var(--background));
  font-size: 13px;
  white-space: nowrap;
  transition: background-color 120ms;
}
.shortcut-key:hover:not(:disabled) { background: var(--muted); }
.shortcut-key:active:not(:disabled) { background: var(--accent); }
.shortcut-key:disabled { cursor: not-allowed; opacity: .45; }
.shortcut-key--danger {
  color: var(--destructive);
  border-color: color-mix(in srgb, var(--destructive) 20%, var(--border));
  background: color-mix(in srgb, var(--destructive) 3%, var(--background));
}
</style>
