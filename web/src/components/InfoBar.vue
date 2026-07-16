<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { CanonicalKey } from '@/types/generated'
import { Badge } from '@/components/ui/badge'
import { Kbd } from '@/components/ui/kbd'
import { Separator } from '@/components/ui/separator'

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
  <div class="w-full border-t bg-background">
    <!-- Compact mode (explicit prop or auto on small screens via sm:hidden) -->
    <div :class="compact ? '' : 'sm:hidden'">
      <div class="flex items-center justify-between text-xs px-2 py-0.5">
        <div v-if="keyboardLedEnabled" class="flex items-center gap-1">
          <Badge variant="outline" class="h-4 gap-1 px-1 text-[10px] text-foreground">
            <span class="h-1.5 w-1.5 rounded-full" :class="capsLock ? 'bg-success' : 'bg-warning'" />C
          </Badge>
          <Badge variant="outline" class="h-4 gap-1 px-1 text-[10px] text-foreground">
            <span class="h-1.5 w-1.5 rounded-full" :class="numLock ? 'bg-success' : 'bg-warning'" />N
          </Badge>
          <Badge variant="outline" class="h-4 gap-1 px-1 text-[10px] text-foreground">
            <span class="h-1.5 w-1.5 rounded-full" :class="scrollLock ? 'bg-success' : 'bg-warning'" />S
          </Badge>
        </div>
        <div v-else class="text-[10px] text-muted-foreground/60">
          {{ t('infobar.keyboardLedUnavailable') }}
        </div>
        <div v-if="keysDisplay" class="text-[10px] text-muted-foreground truncate max-w-[200px]">
          {{ keysDisplay }}
        </div>
      </div>
    </div>

    <!-- Normal mode (hidden on small screens unless compact is explicitly set) -->
    <div :class="compact ? 'hidden' : 'hidden sm:block'">
      <div class="flex flex-wrap items-center justify-between text-xs">
        <!-- Left side: Debug info and pressed keys -->
        <div class="flex items-center gap-4 px-3 py-1 min-w-0 flex-1">
          <div class="flex items-center gap-1.5 min-w-0">
            <span class="font-medium text-muted-foreground shrink-0">{{ t('infobar.keys') }}:</span>
            <Kbd class="max-w-full truncate">{{ keysDisplay || '-' }}</Kbd>
          </div>

          <div v-if="debugMode && mousePosition" class="flex items-center gap-1.5 hidden md:flex">
            <span class="font-medium text-muted-foreground">{{ t('infobar.pointer') }}:</span>
            <span class="text-foreground">{{ mousePosition.x }}, {{ mousePosition.y }}</span>
          </div>
        </div>

        <!-- Right side: Keyboard LED states -->
        <div class="flex items-center shrink-0">
          <Separator orientation="vertical" class="h-5" />
          <template v-if="keyboardLedEnabled">
            <Badge variant="outline" class="mx-1 gap-1.5 text-foreground">
              <span class="h-1.5 w-1.5 rounded-full" :class="capsLock ? 'bg-success' : 'bg-warning'" />
              {{ t('infobar.caps') }}
            </Badge>
            <Badge variant="outline" class="mx-1 gap-1.5 text-foreground">
              <span class="h-1.5 w-1.5 rounded-full" :class="numLock ? 'bg-success' : 'bg-warning'" />
              {{ t('infobar.num') }}
            </Badge>
            <Badge variant="outline" class="mx-1 gap-1.5 text-foreground">
              <span class="h-1.5 w-1.5 rounded-full" :class="scrollLock ? 'bg-success' : 'bg-warning'" />
              {{ t('infobar.scroll') }}
            </Badge>
          </template>
          <div v-else class="px-3 py-1 text-muted-foreground/60">
            {{ t('infobar.keyboardLedUnavailable') }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
