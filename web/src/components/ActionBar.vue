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
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
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

// Desktop toolbar popover/dialog state
const pasteOpen = ref(false)
const atxOpen = ref(false)
const videoPopoverOpen = ref(false)
const hidPopoverOpen = ref(false)
const audioPopoverOpen = ref(false)
const msdDialogOpen = ref(false)
const extensionOpen = ref(false)

// Mobile Sheet state — opened from the overflow menu.
// We use Sheet (bottom drawer) instead of Popover because Popover relies on an
// anchor element that is hidden / clipped on small screens, causing it to
// immediately close after opening.
const mobileAtxOpen = ref(false)
const mobilePasteOpen = ref(false)

// Timestamps used to suppress spurious "interact-outside" events that arrive
// within ~300 ms of the Sheet opening (e.g. delayed synthetic pointer events
// from the same touch gesture that opened the overflow menu).
const mobileAtxOpenTime = ref(0)
const mobilePasteOpenTime = ref(0)

const OPEN_GUARD_MS = 350

const guardOutside = (openTime: number, e: Event) => {
  if (Date.now() - openTime < OPEN_GUARD_MS) {
    e.preventDefault()
  }
}

// On mobile, clicking a DropdownMenuItem generates pointer events that can
// immediately dismiss any overlay opened in the same tick. Close the dropdown
// first, then open the target after a short delay.
const openFromOverflow = (setter: () => void) => {
  overflowMenuOpen.value = false
  setTimeout(setter, 50)
}

const openMobileAtx = () => openFromOverflow(() => {
  mobileAtxOpen.value = true
  mobileAtxOpenTime.value = Date.now()
})

const openMobilePaste = () => openFromOverflow(() => {
  mobilePasteOpen.value = true
  mobilePasteOpenTime.value = Date.now()
})
</script>

<template>
  <div class="w-full border-b border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
    <div class="flex items-center px-2 sm:px-4 py-1 sm:py-1.5">
      <!-- Left side buttons — overflow hidden so it never pushes into right side -->
      <div class="flex items-center gap-0.5 sm:gap-1.5 flex-1 min-w-0 overflow-hidden">
        <!-- Video Config - Always visible -->
        <VideoConfigPopover
          v-model:open="videoPopoverOpen"
          :video-mode="props.videoMode || 'mjpeg'"
          @update:video-mode="emit('update:videoMode', $event)"
        />

        <!-- Audio Config - Always visible (xs shows icon only) -->
        <AudioConfigPopover v-model:open="audioPopoverOpen" />

        <!-- HID Config - Always visible -->
        <HidConfigPopover
          v-model:open="hidPopoverOpen"
          :mouse-mode="mouseMode"
          @update:mouse-mode="emit('toggleMouseMode')"
        />

        <!-- Virtual Media (MSD) - Hidden below md, shown in overflow -->
        <div v-if="showMsd" class="hidden md:block">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="msdDialogOpen = true">
                  <HardDrive class="h-4 w-4" />
                  <span class="hidden lg:inline">{{ t('actionbar.virtualMedia') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ t('actionbar.virtualMediaTip') }}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <!-- ATX Power Control - Hidden below md; shown as Sheet on mobile -->
        <div class="hidden md:block">
          <Popover v-model:open="atxOpen">
            <PopoverTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
                <Power class="h-4 w-4" />
                <span class="hidden lg:inline">{{ t('actionbar.power') }}</span>
              </Button>
            </PopoverTrigger>
            <PopoverContent class="w-[min(280px,90vw)] p-0" align="start">
              <AtxPopover
                @close="atxOpen = false"
                @power-short="emit('powerShort')"
                @power-long="emit('powerLong')"
                @reset="emit('reset')"
                @wol="(mac) => emit('wol', mac)"
              />
            </PopoverContent>
          </Popover>
        </div>

        <!-- Paste Text - Hidden below lg; shown as Sheet on mobile -->
        <div class="hidden lg:block">
          <Popover v-model:open="pasteOpen">
            <PopoverTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
                <ClipboardPaste class="h-4 w-4" />
                <span class="hidden xl:inline">{{ t('actionbar.paste') }}</span>
              </Button>
            </PopoverTrigger>
            <PopoverContent class="w-[min(400px,90vw)] p-0" align="start">
              <PasteModal @close="pasteOpen = false" />
            </PopoverContent>
          </Popover>
        </div>
      </div>

      <!-- Right side buttons — always shrink-0, never compressed -->
      <div class="flex items-center gap-0.5 sm:gap-1.5 shrink-0 ml-1 sm:ml-2">
        <!-- Extension Menu - Hidden below xl -->
        <div class="hidden xl:block">
          <Popover v-model:open="extensionOpen">
            <PopoverTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
                <Cable class="h-4 w-4" />
                {{ t('actionbar.extension') }}
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
        </div>

        <!-- Settings - Hidden below xl -->
        <div class="hidden xl:block">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="router.push('/settings')">
                  <Settings class="h-4 w-4" />
                  {{ t('actionbar.settings') }}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ t('actionbar.settingsTip') }}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <!-- Connection Stats - Hidden below md -->
        <div class="hidden md:block">
          <TooltipProvider>
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
        </div>

        <div class="h-5 w-px bg-slate-200 dark:bg-slate-700 hidden md:block" />

        <!-- Virtual Keyboard - Always visible (important for mobile) -->
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="sm"
                class="h-7 w-7 sm:h-8 sm:w-auto p-0 sm:px-2 sm:gap-1.5 text-xs"
                @click="emit('toggleVirtualKeyboard')"
              >
                <Keyboard class="h-3.5 w-3.5 sm:h-4 sm:w-4" />
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
                class="h-7 w-7 sm:h-8 sm:w-auto p-0 sm:px-2 sm:gap-1.5 text-xs"
                @click="emit('toggleFullscreen')"
              >
                <Maximize class="h-3.5 w-3.5 sm:h-4 sm:w-4" />
                <span class="hidden xl:inline">{{ t('actionbar.fullscreen') }}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{{ t('actionbar.fullscreenTip') }}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <!-- Overflow Menu - Shows hidden items on smaller screens -->
        <DropdownMenu v-model:open="overflowMenuOpen">
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="sm" class="h-7 w-7 sm:h-8 sm:w-8 p-0 xl:hidden">
              <MoreHorizontal class="h-3.5 w-3.5 sm:h-4 sm:w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <!-- MSD - Below md, hidden when CH9329 backend -->
            <DropdownMenuItem v-if="showMsd" class="md:hidden" @click="openFromOverflow(() => msdDialogOpen = true)">
              <HardDrive class="h-4 w-4 mr-2" />
              {{ t('actionbar.virtualMedia') }}
            </DropdownMenuItem>

            <!-- ATX - Opens a Sheet on mobile (below md) -->
            <DropdownMenuItem class="md:hidden" @click="openMobileAtx">
              <Power class="h-4 w-4 mr-2" />
              {{ t('actionbar.power') }}
            </DropdownMenuItem>

            <!-- Paste - Opens a Sheet on mobile (below lg) -->
            <DropdownMenuItem class="lg:hidden" @click="openMobilePaste">
              <ClipboardPaste class="h-4 w-4 mr-2" />
              {{ t('actionbar.paste') }}
            </DropdownMenuItem>

            <DropdownMenuSeparator />

            <!-- Stats - Below md -->
            <DropdownMenuItem class="md:hidden" @click="openFromOverflow(() => emit('toggleStats'))">
              <BarChart3 class="h-4 w-4 mr-2" />
              {{ t('actionbar.stats') }}
            </DropdownMenuItem>

            <!-- Extension - Below xl -->
            <DropdownMenuItem
              class="xl:hidden"
              :disabled="!props.ttydRunning"
              @click="openFromOverflow(() => emit('openTerminal'))"
            >
              <Terminal class="h-4 w-4 mr-2" />
              {{ t('extensions.ttyd.title') }}
            </DropdownMenuItem>

            <!-- Settings - Below xl -->
            <DropdownMenuItem class="xl:hidden" @click="openFromOverflow(() => router.push('/settings'))">
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

  <!-- Mobile ATX Sheet — used when ATX is opened from the overflow menu.
       A Sheet avoids the Popover anchor-positioning issues on mobile. -->
  <Sheet v-model:open="mobileAtxOpen">
    <SheetContent
      side="bottom"
      class="max-h-[90dvh] overflow-y-auto"
      @pointer-down-outside="(e) => guardOutside(mobileAtxOpenTime, e)"
      @interact-outside="(e) => guardOutside(mobileAtxOpenTime, e)"
    >
      <SheetHeader class="mb-2">
        <SheetTitle>{{ t('actionbar.power') }}</SheetTitle>
      </SheetHeader>
      <AtxPopover
        @close="mobileAtxOpen = false"
        @power-short="emit('powerShort')"
        @power-long="emit('powerLong')"
        @reset="emit('reset')"
        @wol="(mac) => emit('wol', mac)"
      />
    </SheetContent>
  </Sheet>

  <!-- Mobile Paste Sheet — used when Paste is opened from the overflow menu. -->
  <Sheet v-model:open="mobilePasteOpen">
    <SheetContent
      side="bottom"
      class="max-h-[90dvh] overflow-y-auto"
      @pointer-down-outside="(e) => guardOutside(mobilePasteOpenTime, e)"
      @interact-outside="(e) => guardOutside(mobilePasteOpenTime, e)"
    >
      <SheetHeader class="mb-2">
        <SheetTitle>{{ t('actionbar.paste') }}</SheetTitle>
      </SheetHeader>
      <PasteModal @close="mobilePasteOpen = false" />
    </SheetContent>
  </Sheet>
</template>
