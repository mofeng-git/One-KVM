<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  ClipboardPaste,
  HardDrive,
  Keyboard,
  Cable,
  Settings,
  Maximize,
  Power,
  BarChart3,
  Terminal,
  MoreHorizontal,
} from 'lucide-vue-next'
import PasteModal from '@/components/PasteModal.vue'
import AtxPopover from '@/components/AtxPopover.vue'
import VideoConfigPopover, { type VideoMode } from '@/components/VideoConfigPopover.vue'
import HidConfigPopover from '@/components/HidConfigPopover.vue'
import AudioConfigPopover from '@/components/AudioConfigPopover.vue'
import MsdDialog from '@/components/MsdDialog.vue'

const { t } = useI18n()
const router = useRouter()
const systemStore = useSystemStore()

// Overflow menu state
const overflowMenuOpen = ref(false)

// MSD is only available when HID backend is not CH9329 (CH9329 is serial-only, no USB gadget)
const hidBackend = computed(() => (systemStore.hid?.backend ?? '').toLowerCase())
const isCh9329Backend = computed(() => hidBackend.value.includes('ch9329'))
const showMsd = computed(() => {
  return !!systemStore.msd?.available && !isCh9329Backend.value
})

const props = defineProps<{
  mouseMode?: 'absolute' | 'relative'
  videoMode?: VideoMode
  ttydRunning?: boolean
}>()

const emit = defineEmits<{
  (e: 'toggleFullscreen'): void
  (e: 'toggleStats'): void
  (e: 'toggleVirtualKeyboard'): void
  (e: 'toggleMouseMode'): void
  (e: 'update:videoMode', mode: VideoMode): void
  (e: 'powerShort'): void
  (e: 'powerLong'): void
  (e: 'reset'): void
  (e: 'wol', macAddress: string): void
  (e: 'openTerminal'): void
}>()

const pasteOpen = ref(false)
const atxOpen = ref(false)
const videoPopoverOpen = ref(false)
const hidPopoverOpen = ref(false)
const audioPopoverOpen = ref(false)
const msdDialogOpen = ref(false)
const extensionOpen = ref(false)
</script>

<template>
  <div class="w-full border-b border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
    <div class="flex flex-wrap items-center gap-x-2 gap-y-2 px-4 py-1.5">
      <!-- Left side buttons -->
      <div class="flex flex-wrap items-center gap-1.5 w-full sm:flex-1 sm:min-w-0">
        <!-- Video Config - Always visible -->
        <VideoConfigPopover
          v-model:open="videoPopoverOpen"
          :video-mode="props.videoMode || 'mjpeg'"
          @update:video-mode="emit('update:videoMode', $event)"
        />

        <!-- Audio Config - Always visible -->
        <AudioConfigPopover v-model:open="audioPopoverOpen" />

        <!-- HID Config - Always visible -->
        <HidConfigPopover
          v-model:open="hidPopoverOpen"
          :mouse-mode="mouseMode"
          @update:mouse-mode="emit('toggleMouseMode')"
        />

        <!-- Virtual Media (MSD) - Hidden on small screens, shown in overflow -->
        <!-- Also hidden when HID backend is CH9329 (no USB gadget support) -->
        <TooltipProvider v-if="showMsd" class="hidden sm:block">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="msdDialogOpen = true">
                <HardDrive class="h-4 w-4" />
                <span class="hidden md:inline">{{ t('actionbar.virtualMedia') }}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{{ t('actionbar.virtualMediaTip') }}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <!-- ATX Power Control - Hidden on small screens -->
        <Popover v-model:open="atxOpen" class="hidden sm:block">
          <PopoverTrigger as-child>
            <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
              <Power class="h-4 w-4" />
              <span class="hidden md:inline">{{ t('actionbar.power') }}</span>
            </Button>
          </PopoverTrigger>
          <PopoverContent class="w-[280px] p-0" align="start">
            <AtxPopover
              @close="atxOpen = false"
              @power-short="emit('powerShort')"
              @power-long="emit('powerLong')"
              @reset="emit('reset')"
              @wol="(mac) => emit('wol', mac)"
            />
          </PopoverContent>
        </Popover>

        <!-- Paste Text - Hidden on small screens -->
        <Popover v-model:open="pasteOpen" class="hidden md:block">
          <PopoverTrigger as-child>
            <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
              <ClipboardPaste class="h-4 w-4" />
              <span class="hidden lg:inline">{{ t('actionbar.paste') }}</span>
            </Button>
          </PopoverTrigger>
          <PopoverContent class="w-[400px] p-0" align="start">
            <PasteModal @close="pasteOpen = false" />
          </PopoverContent>
        </Popover>
      </div>

      <!-- Right side buttons -->
      <div class="flex items-center gap-1.5 w-full justify-end sm:w-auto sm:ml-auto shrink-0">
        <!-- Extension Menu - Hidden on small screens -->
        <Popover v-model:open="extensionOpen" class="hidden lg:block">
          <PopoverTrigger as-child>
            <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
              <Cable class="h-4 w-4" />
              <span class="hidden xl:inline">{{ t('actionbar.extension') }}</span>
            </Button>
          </PopoverTrigger>
          <PopoverContent class="w-48 p-1" align="start">
            <div class="space-y-0.5">
              <Button
                variant="ghost"
                size="sm"
                class="w-full justify-start gap-2 h-8"
                :disabled="!props.ttydRunning"
                @click="extensionOpen = false; emit('openTerminal')"
              >
                <Terminal class="h-4 w-4" />
                {{ t('extensions.ttyd.title') }}
              </Button>
            </div>
          </PopoverContent>
        </Popover>

        <!-- Settings - Hidden on small screens -->
        <TooltipProvider class="hidden lg:block">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="router.push('/settings')">
                <Settings class="h-4 w-4" />
                <span class="hidden xl:inline">{{ t('actionbar.settings') }}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{{ t('actionbar.settingsTip') }}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <!-- Connection Stats - Hidden on very small screens -->
        <TooltipProvider class="hidden sm:block">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="emit('toggleStats')">
                <BarChart3 class="h-4 w-4" />
                <span class="hidden xl:inline">{{ t('actionbar.stats') }}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{{ t('actionbar.statsTip') }}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <div class="h-5 w-px bg-slate-200 dark:bg-slate-700 hidden sm:block" />

        <!-- Virtual Keyboard - Always visible (important for mobile) -->
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="sm"
                class="h-8 gap-1.5 text-xs"
                @click="emit('toggleVirtualKeyboard')"
              >
                <Keyboard class="h-4 w-4" />
                <span class="hidden xl:inline">{{ t('actionbar.keyboard') }}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{{ t('actionbar.keyboardTip') }}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <!-- Fullscreen - Always visible -->
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="sm"
                class="h-8 gap-1.5 text-xs"
                @click="emit('toggleFullscreen')"
              >
                <Maximize class="h-4 w-4" />
                <span class="hidden xl:inline">{{ t('actionbar.fullscreen') }}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{{ t('actionbar.fullscreenTip') }}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <!-- Overflow Menu - Shows hidden items on small screens -->
        <DropdownMenu v-model:open="overflowMenuOpen">
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="sm" class="h-8 w-8 p-0 lg:hidden">
              <MoreHorizontal class="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <!-- MSD - Mobile only, hidden when CH9329 backend -->
            <DropdownMenuItem v-if="showMsd" class="sm:hidden" @click="msdDialogOpen = true; overflowMenuOpen = false">
              <HardDrive class="h-4 w-4 mr-2" />
              {{ t('actionbar.virtualMedia') }}
            </DropdownMenuItem>

            <!-- ATX - Mobile only -->
            <DropdownMenuItem class="sm:hidden" @click="atxOpen = true; overflowMenuOpen = false">
              <Power class="h-4 w-4 mr-2" />
              {{ t('actionbar.power') }}
            </DropdownMenuItem>

            <!-- Paste - Tablet and below -->
            <DropdownMenuItem class="md:hidden" @click="pasteOpen = true; overflowMenuOpen = false">
              <ClipboardPaste class="h-4 w-4 mr-2" />
              {{ t('actionbar.paste') }}
            </DropdownMenuItem>

            <DropdownMenuSeparator class="lg:hidden" />

            <!-- Stats - Mobile only -->
            <DropdownMenuItem class="sm:hidden" @click="emit('toggleStats'); overflowMenuOpen = false">
              <BarChart3 class="h-4 w-4 mr-2" />
              {{ t('actionbar.stats') }}
            </DropdownMenuItem>

            <!-- Extension - Tablet and below -->
            <DropdownMenuItem
              class="lg:hidden"
              :disabled="!props.ttydRunning"
              @click="emit('openTerminal'); overflowMenuOpen = false"
            >
              <Terminal class="h-4 w-4 mr-2" />
              {{ t('extensions.ttyd.title') }}
            </DropdownMenuItem>

            <!-- Settings - Tablet and below -->
            <DropdownMenuItem class="lg:hidden" @click="router.push('/settings'); overflowMenuOpen = false">
              <Settings class="h-4 w-4 mr-2" />
              {{ t('actionbar.settings') }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  </div>

  <!-- MSD Dialog -->
  <MsdDialog v-if="showMsd" v-model:open="msdDialogOpen" />
</template>
