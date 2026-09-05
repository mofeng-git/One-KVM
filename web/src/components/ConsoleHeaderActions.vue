<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { BarChart3, Bot, Settings, Terminal, MoreHorizontal } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'

defineProps<{
  showStats: boolean
  showTerminal: boolean
  terminalRunning: boolean
  showComputerUse: boolean
}>()
const emit = defineEmits<{
  (e: 'openStats'): void
  (e: 'openTerminal'): void
  (e: 'openComputerUse'): void
}>()
const router = useRouter()
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
      <Button v-if="showStats" variant="ghost" size="sm" class="gap-1.5 rounded-lg text-xs" :aria-label="t('actionbar.stats')" :title="t('actionbar.stats')" @click="emit('openStats')">
        <BarChart3 class="size-4" /><span class="hidden lg:inline">{{ t('actionbar.stats') }}</span>
      </Button>
      <Button v-if="showTerminal" variant="ghost" size="sm" class="gap-1.5 rounded-lg text-xs" :aria-label="t('actionbar.webTerminal')" :title="t('actionbar.webTerminal')" :disabled="!terminalRunning" @click="emit('openTerminal')">
        <Terminal class="size-4" /><span class="hidden lg:inline">{{ t('actionbar.webTerminal') }}</span>
      </Button>
      <Button v-if="showComputerUse" variant="ghost" size="sm" class="gap-1.5 rounded-lg text-xs" :aria-label="t('computerUse.title')" :title="t('computerUse.title')" @click="emit('openComputerUse')">
        <Bot class="size-4" /><span class="hidden lg:inline">AI</span>
      </Button>
    </div>
    <DropdownMenu v-if="showStats || showTerminal || showComputerUse" v-model:open="menuOpen">
      <DropdownMenuTrigger as-child class="md:hidden">
        <Button variant="ghost" size="icon-sm" :aria-label="t('actionbar.more')"><MoreHorizontal class="size-4" /></Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-48">
        <DropdownMenuItem v-if="showStats" @select="openFromMenu(() => emit('openStats'))"><BarChart3 class="size-4" />{{ t('actionbar.stats') }}</DropdownMenuItem>
        <DropdownMenuItem v-if="showTerminal" :disabled="!terminalRunning" @select="openFromMenu(() => emit('openTerminal'))"><Terminal class="size-4" />{{ t('actionbar.webTerminal') }}</DropdownMenuItem>
        <DropdownMenuItem v-if="showComputerUse" @select="openFromMenu(() => emit('openComputerUse'))"><Bot class="size-4" />{{ t('computerUse.title') }}</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <Button variant="ghost" size="sm" class="gap-1.5 rounded-lg px-2 text-xs" :aria-label="t('actionbar.settings')" :title="t('actionbar.settings')" @click="router.push('/settings')">
      <Settings class="size-4" /><span class="hidden lg:inline">{{ t('actionbar.settings') }}</span>
    </Button>
  </div>
</template>
