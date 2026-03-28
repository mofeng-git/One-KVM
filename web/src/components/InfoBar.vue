<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { CanonicalKey } from '@/types/generated'
import { cn } from '@/lib/utils'

const props = defineProps<{
  pressedKeys?: CanonicalKey[]
  capsLock?: boolean
  numLock?: boolean
  scrollLock?: boolean
  keyboardLedEnabled?: boolean
  mousePosition?: { x: number; y: number }
  debugMode?: boolean
  compact?: boolean
}>()

const { t } = useI18n()

// Key name mapping for friendly display
const keyNameMap: Record<string, string> = {
  MetaLeft: 'Win', MetaRight: 'Win',
  ControlLeft: 'Ctrl', ControlRight: 'Ctrl',
  ShiftLeft: 'Shift', ShiftRight: 'Shift',
  AltLeft: 'Alt', AltRight: 'AltGr',
  CapsLock: 'Caps', NumLock: 'Num', ScrollLock: 'Scroll',
  Backspace: 'Back', Delete: 'Del',
  ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→',
  Escape: 'Esc', Enter: 'Enter', Tab: 'Tab', Space: 'Space',
  PageUp: 'PgUp', PageDown: 'PgDn',
  Insert: 'Ins', Home: 'Home', End: 'End',
  ContextMenu: 'Menu',
}

const keysDisplay = computed(() => {
  if (!props.pressedKeys || props.pressedKeys.length === 0) return ''
  return props.pressedKeys
    .map(key => keyNameMap[key] || key.replace(/^(Key|Digit)/, ''))
    .join(', ')
})
</script>

<template>
  <div class="w-full border-t border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
    <!-- Compact mode for small screens -->
    <div v-if="compact" class="flex items-center justify-between text-xs px-2 py-0.5">
      <!-- LED indicator only in compact mode -->
      <div v-if="keyboardLedEnabled" class="flex items-center gap-1">
        <span
          v-if="capsLock"
          class="px-1.5 py-0.5 bg-primary/10 text-primary rounded text-[10px] font-medium"
        >C</span>
        <span v-else class="text-muted-foreground/40 text-[10px]">C</span>
        <span
          :class="numLock ? 'px-1.5 py-0.5 bg-primary/10 text-primary rounded text-[10px] font-medium' : 'text-muted-foreground/40 text-[10px]'"
        >N</span>
        <span
          :class="scrollLock ? 'px-1.5 py-0.5 bg-primary/10 text-primary rounded text-[10px] font-medium' : 'text-muted-foreground/40 text-[10px]'"
        >S</span>
      </div>
      <div v-else class="text-[10px] text-muted-foreground/60">
        {{ t('infobar.keyboardLedUnavailable') }}
      </div>
      <!-- Keys in compact mode -->
      <div v-if="keysDisplay" class="text-[10px] text-muted-foreground truncate max-w-[150px]">
        {{ keysDisplay }}
      </div>
    </div>

    <!-- Normal mode -->
    <div v-else class="flex flex-wrap items-center justify-between text-xs">
      <!-- Left side: Debug info and pressed keys -->
      <div class="flex items-center gap-4 px-3 py-1 min-w-0 flex-1">
        <!-- Pressed Keys -->
        <div class="flex items-center gap-1.5 min-w-0">
          <span class="font-medium text-muted-foreground shrink-0 hidden sm:inline">{{ t('infobar.keys') }}:</span>
          <span class="text-foreground truncate">{{ keysDisplay || '-' }}</span>
        </div>

        <!-- Debug: Mouse Position -->
        <div v-if="debugMode && mousePosition" class="flex items-center gap-1.5 hidden md:flex">
          <span class="font-medium text-muted-foreground">{{ t('infobar.pointer') }}:</span>
          <span class="text-foreground">{{ mousePosition.x }}, {{ mousePosition.y }}</span>
        </div>
      </div>

      <!-- Right side: Keyboard LED states -->
      <div class="flex items-center shrink-0">
        <template v-if="keyboardLedEnabled">
          <div
            :class="cn(
              'px-2 py-1 select-none transition-colors',
              capsLock ? 'text-foreground font-medium bg-primary/5' : 'text-muted-foreground/40'
            )"
          >
            <span class="hidden sm:inline">{{ t('infobar.caps') }}</span>
            <span class="sm:hidden">C</span>
          </div>
          <div
            :class="cn(
              'px-2 py-1 select-none transition-colors',
              numLock ? 'text-foreground font-medium bg-primary/5' : 'text-muted-foreground/40'
            )"
          >
            <span class="hidden sm:inline">{{ t('infobar.num') }}</span>
            <span class="sm:hidden">N</span>
          </div>
          <div
            :class="cn(
              'px-2 py-1 select-none transition-colors',
              scrollLock ? 'text-foreground font-medium bg-primary/5' : 'text-muted-foreground/40'
            )"
          >
            <span class="hidden sm:inline">{{ t('infobar.scroll') }}</span>
            <span class="sm:hidden">S</span>
          </div>
        </template>
        <div v-else class="px-3 py-1 text-muted-foreground/60">
          {{ t('infobar.keyboardLedUnavailable') }}
        </div>
      </div>
    </div>
  </div>
</template>
