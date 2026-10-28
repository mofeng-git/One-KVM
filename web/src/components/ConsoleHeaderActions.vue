<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { BarChart3, Terminal, MoreHorizontal } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'

defineProps<{
  showStats: boolean
  showTerminal: boolean
  terminalRunning: boolean
  statsOpen?: boolean
  terminalOpen?: boolean
}>()
const emit = defineEmits<{
  (e: 'openStats'): void
  (e: 'openTerminal'): void
}>()
const { t } = useI18n()
const menuOpen = ref(false)
function openFromMenu(action: () => void) {
  menuOpen.value = false
  window.setTimeout(action, 50)
}
</script>

<template>
  <div class="console-header-actions flex shrink-0 items-center gap-1">
    <div class="hidden items-center gap-1 md:flex">
      <Button v-if="showStats" variant="ghost" size="sm" class="gap-1.5 rounded-lg text-xs" :aria-label="t('actionbar.stats')" :title="t('actionbar.stats')" :aria-pressed="!!statsOpen" @click="emit('openStats')">
        <BarChart3 class="size-4" /><span class="hidden lg:inline">{{ t('actionbar.stats') }}</span>
      </Button>
      <Button v-if="showTerminal" variant="ghost" size="sm" class="gap-1.5 rounded-lg text-xs" :aria-label="t('actionbar.webTerminal')" :title="t('actionbar.webTerminal')" :disabled="!terminalRunning" :aria-pressed="!!terminalOpen" @click="emit('openTerminal')">
        <Terminal class="size-4" /><span class="hidden lg:inline">{{ t('actionbar.webTerminal') }}</span>
      </Button>
    </div>
    <DropdownMenu v-if="showStats || showTerminal" v-model:open="menuOpen">
      <DropdownMenuTrigger as-child class="md:hidden">
        <Button variant="ghost" size="icon-sm" :aria-label="t('actionbar.more')"><MoreHorizontal class="size-4" /></Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-48">
        <DropdownMenuItem v-if="showStats" @select="openFromMenu(() => emit('openStats'))"><BarChart3 class="size-4" />{{ t('actionbar.stats') }}</DropdownMenuItem>
        <DropdownMenuItem v-if="showTerminal" :disabled="!terminalRunning" @select="openFromMenu(() => emit('openTerminal'))"><Terminal class="size-4" />{{ t('actionbar.webTerminal') }}</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
