<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'
import {
  PopoverContent,
  PopoverTrigger,
  Popover,
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
  Settings,
  Maximize,
  Power,
  BarChart3,
  Terminal,
  MoreHorizontal,
  Bot,
} from 'lucide-vue-next'
import PasteModal from '@/components/PasteModal.vue'
import AtxPopover from '@/components/AtxPopover.vue'
import VideoConfigPopover, { type VideoMode } from '@/components/VideoConfigPopover.vue'
import HidConfigPopover from '@/components/HidConfigPopover.vue'
import AudioConfigPopover from '@/components/AudioConfigPopover.vue'
import MsdDialog from '@/components/MsdDialog.vue'

const { t, locale } = useI18n()
const router = useRouter()
const systemStore = useSystemStore()

const overflowMenuOpen = ref(false)

const hidBackend = computed(() => (systemStore.hid?.backend ?? '').toLowerCase())
const isCh9329Backend = computed(() => hidBackend.value.includes('ch9329'))
const showMsd = computed(() => {
  return !!systemStore.msd?.available && !isCh9329Backend.value
})
const showAtx = computed(() => systemStore.atx?.available === true)

const props = defineProps<{
  mouseMode?: 'absolute' | 'relative'
  videoMode?: VideoMode
  ttydRunning?: boolean
  showTerminal?: boolean
  showComputerUse?: boolean
  showPasteText?: boolean
}>()
const showStats = computed(() => (props.videoMode ?? 'mjpeg') !== 'mjpeg')
const showPasteText = computed(() => props.showPasteText !== false)

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
  (e: 'openComputerUse'): void
}>()

const pasteOpen = ref(false)
const atxOpen = ref(false)
const videoPopoverOpen = ref(false)
const hidPopoverOpen = ref(false)
const audioPopoverOpen = ref(false)
const msdDialogOpen = ref(false)

const mobileAtxOpen = ref(false)
const mobilePasteOpen = ref(false)
const mobileAtxOpenTime = ref(0)
const mobilePasteOpenTime = ref(0)

const OPEN_GUARD_MS = 350

const guardOutside = (openTime: number, e: Event) => {
  if (Date.now() - openTime < OPEN_GUARD_MS) {
    e.preventDefault()
  }
}

const openFromOverflow = (setter: () => void) => {
  overflowMenuOpen.value = false
  setTimeout(setter, 50)
}

const openMobileAtx = () => openFromOverflow(() => {
  if (!showAtx.value) return
  mobileAtxOpen.value = true
  mobileAtxOpenTime.value = Date.now()
})

const openMobilePaste = () => openFromOverflow(() => {
  if (!showPasteText.value) return
  mobilePasteOpen.value = true
  mobilePasteOpenTime.value = Date.now()
})


const barRef = ref<HTMLElement | null>(null)
const measureRef = ref<HTMLElement | null>(null)
const barWidth = ref(0)
let resizeObserver: ResizeObserver | null = null

type CollapsibleItem =
  | 'video' | 'audio' | 'hid'
  | 'msd' | 'atx' | 'paste'
  | 'stats' | 'terminal' | 'settings'

interface ItemSpec {
  id: CollapsibleItem
  side: 'left' | 'right'
}

const ITEM_SPECS: ItemSpec[] = [
  { id: 'video',     side: 'left' },
  { id: 'audio',     side: 'left' },
  { id: 'hid',       side: 'left' },
  { id: 'msd',       side: 'left' },
  { id: 'atx',       side: 'left' },
  { id: 'paste',     side: 'left' },
  { id: 'stats',     side: 'right' },
  { id: 'terminal',  side: 'right' },
  { id: 'settings',  side: 'right' },
]

const measuredWidths = ref<Map<CollapsibleItem, { icon: number; label: number }>>(new Map())
const measurementReady = ref(false)

const measureButtonWidths = async () => {
  await nextTick()
  if (!measureRef.value) return

  const newWidths = new Map<CollapsibleItem, { icon: number; label: number }>()
  
  for (const spec of ITEM_SPECS) {
    const iconEl = measureRef.value.querySelector(`[data-measure="${spec.id}-icon"]`) as HTMLElement
    const labelEl = measureRef.value.querySelector(`[data-measure="${spec.id}-label"]`) as HTMLElement
    
    if (iconEl && labelEl) {
      newWidths.set(spec.id, {
        icon: Math.ceil(iconEl.offsetWidth) + 8,
        label: Math.ceil(labelEl.offsetWidth) + 8,
      })
    }
  }
  
  measuredWidths.value = newWidths
  measurementReady.value = true
}

onMounted(() => {
  if (barRef.value) {
    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0]
      if (entry) barWidth.value = entry.contentRect.width
    })
    resizeObserver.observe(barRef.value)
    barWidth.value = barRef.value.clientWidth
  }
  
  measureButtonWidths()
})

onUnmounted(() => { 
  resizeObserver?.disconnect() 
})

watch(locale, () => {
  measurementReady.value = false
  measureButtonWidths()
})

watch(showAtx, (visible) => {
  if (!visible) {
    atxOpen.value = false
    mobileAtxOpen.value = false
  }
})

watch(showPasteText, (visible) => {
  if (!visible) {
    pasteOpen.value = false
    mobilePasteOpen.value = false
  }
})

const RIGHT_FIXED_PX = 120

const collapsibleItems = computed(() => {
  const items = ITEM_SPECS.slice(3).filter(item => {
    if (item.id === 'msd' && !showMsd.value) return false
    if (item.id === 'atx' && !showAtx.value) return false
    if (item.id === 'paste' && !showPasteText.value) return false
    if (item.id === 'stats' && !showStats.value) return false
    if (item.id === 'terminal' && props.showTerminal === false) return false
    return true
  })
  return items
})

const visibleSet = computed(() => {
  if (!measurementReady.value) {
    return new Map<CollapsibleItem, 'icon' | 'label'>()
  }

  const available = barWidth.value - RIGHT_FIXED_PX
  
  let used = 0
  if (barRef.value) {
    const leftContainer = barRef.value.querySelector('.left-buttons') as HTMLElement
    if (leftContainer) {
      const children = Array.from(leftContainer.children).slice(0, 3) as HTMLElement[]
      used = children.reduce((sum, el) => sum + el.offsetWidth, 0)
    }
  }
  
  if (used === 0) used = 330
  
  const result = new Map<CollapsibleItem, 'icon' | 'label'>()

  for (const item of collapsibleItems.value) {
    const widths = measuredWidths.value.get(item.id)
    if (!widths) continue
    
    if (used + widths.icon <= available) {
      if (used + widths.label <= available) {
        result.set(item.id, 'label')
        used += widths.label
      } else {
        result.set(item.id, 'icon')
        used += widths.icon
      }
    }
  }
  
  return result
})

const isVisible = (id: CollapsibleItem) => visibleSet.value.has(id)
const hasOverflow = computed(() => {
  return collapsibleItems.value.some(i => !visibleSet.value.has(i.id))
})
const hasLeftOverflow = computed(() => {
  return collapsibleItems.value.some(i => i.side === 'left' && !visibleSet.value.has(i.id))
})
const hasRightOverflow = computed(() => {
  return collapsibleItems.value.some(i => i.side === 'right' && !visibleSet.value.has(i.id))
})
</script>

<template>
  <div class="w-full border-b bg-background">
    <div ref="barRef" class="flex items-center px-2 sm:px-4 py-1 sm:py-1.5">
      <!-- Left side buttons -->
      <ButtonGroup class="left-buttons flex-1 min-w-0 overflow-hidden">
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

        <!-- Virtual Media (MSD) - Adaptive -->
        <div v-if="showMsd && isVisible('msd')">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="msdDialogOpen = true">
                  <HardDrive class="h-4 w-4" />
                  <span v-if="visibleSet.get('msd') === 'label'">{{ t('actionbar.virtualMedia') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ t('actionbar.virtualMediaTip') }}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <!-- ATX Power Control - Adaptive -->
        <div v-if="showAtx && isVisible('atx')">
          <Popover v-model:open="atxOpen">
            <PopoverTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
                <Power class="h-4 w-4" />
                <span v-if="visibleSet.get('atx') === 'label'">{{ t('actionbar.power') }}</span>
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

        <!-- Paste Text - Adaptive -->
        <div v-if="showPasteText && isVisible('paste')">
          <Popover v-model:open="pasteOpen">
            <PopoverTrigger as-child>
              <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs">
                <ClipboardPaste class="h-4 w-4" />
                <span v-if="visibleSet.get('paste') === 'label'">{{ t('actionbar.paste') }}</span>
              </Button>
            </PopoverTrigger>
            <PopoverContent class="w-[min(400px,90vw)] p-0" align="start">
              <PasteModal @close="pasteOpen = false" />
            </PopoverContent>
          </Popover>
        </div>
      </ButtonGroup>

      <!-- Right side buttons -->
      <ButtonGroup class="shrink-0 ml-1 sm:ml-2">
        <!-- Connection Stats - Adaptive -->
        <div v-if="isVisible('stats')">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="emit('toggleStats')">
                  <BarChart3 class="h-4 w-4" />
                  <span v-if="visibleSet.get('stats') === 'label'">{{ t('actionbar.stats') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ t('actionbar.statsTip') }}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <!-- Web Terminal - Adaptive -->
        <div v-if="props.showTerminal !== false && isVisible('terminal')">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-8 gap-1.5 text-xs"
                  :disabled="!props.ttydRunning"
                  @click="emit('openTerminal')"
                >
                  <Terminal class="h-4 w-4" />
                  <span v-if="visibleSet.get('terminal') === 'label'">{{ t('actionbar.webTerminal') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ t('extensions.ttyd.title') }}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <!-- Computer Use - Optional -->
        <TooltipProvider v-if="props.showComputerUse !== false">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="sm"
                class="h-7 w-7 sm:h-8 sm:w-auto p-0 sm:px-2 sm:gap-1.5 text-xs"
                @click="emit('openComputerUse')"
              >
                <Bot class="h-3.5 w-3.5 sm:h-4 sm:w-4" />
                <span class="hidden xl:inline">AI</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>Computer Use</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <!-- Settings - Adaptive -->
        <div v-if="isVisible('settings')">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" @click="router.push('/settings')">
                  <Settings class="h-4 w-4" />
                  <span v-if="visibleSet.get('settings') === 'label'">{{ t('actionbar.settings') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ t('actionbar.settingsTip') }}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <div
          v-if="isVisible('settings')"
          aria-hidden="true"
          class="mr-4 h-5 w-px shrink-0 -translate-x-px self-center bg-border"
        />

        <!-- Virtual Keyboard - Always visible -->
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

        <!-- Overflow Menu - Only show if there are overflowed items -->
        <DropdownMenu v-if="hasOverflow" v-model:open="overflowMenuOpen">
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="sm" class="h-7 w-7 sm:h-8 sm:w-8 p-0">
              <MoreHorizontal class="h-3.5 w-3.5 sm:h-4 sm:w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <!-- MSD -->
            <DropdownMenuItem v-if="showMsd && !isVisible('msd')" @click="openFromOverflow(() => msdDialogOpen = true)">
              <HardDrive class="h-4 w-4 mr-2" />
              {{ t('actionbar.virtualMedia') }}
            </DropdownMenuItem>

            <!-- ATX -->
            <DropdownMenuItem v-if="showAtx && !isVisible('atx')" @click="openMobileAtx">
              <Power class="h-4 w-4 mr-2" />
              {{ t('actionbar.power') }}
            </DropdownMenuItem>

            <!-- Paste -->
            <DropdownMenuItem v-if="showPasteText && !isVisible('paste')" @click="openMobilePaste">
              <ClipboardPaste class="h-4 w-4 mr-2" />
              {{ t('actionbar.paste') }}
            </DropdownMenuItem>

            <DropdownMenuSeparator v-if="hasLeftOverflow && hasRightOverflow" />

            <!-- Stats -->
            <DropdownMenuItem v-if="showStats && !isVisible('stats')" @click="openFromOverflow(() => emit('toggleStats'))">
              <BarChart3 class="h-4 w-4 mr-2" />
              {{ t('actionbar.stats') }}
            </DropdownMenuItem>

            <!-- Web Terminal -->
            <DropdownMenuItem
              v-if="props.showTerminal !== false && !isVisible('terminal')"
              :disabled="!props.ttydRunning"
              @click="openFromOverflow(() => emit('openTerminal'))"
            >
              <Terminal class="h-4 w-4 mr-2" />
              {{ t('actionbar.webTerminal') }}
            </DropdownMenuItem>

            <!-- Settings -->
            <DropdownMenuItem v-if="!isVisible('settings')" @click="openFromOverflow(() => router.push('/settings'))">
              <Settings class="h-4 w-4 mr-2" />
              {{ t('actionbar.settings') }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </ButtonGroup>
    </div>
  </div>

  <!-- MSD Dialog -->
  <MsdDialog v-if="showMsd" v-model:open="msdDialogOpen" />

  <!-- Mobile ATX Sheet — used when ATX is opened from the overflow menu.
       A Sheet avoids the Popover anchor-positioning issues on mobile. -->
  <Sheet v-if="showAtx" v-model:open="mobileAtxOpen">
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
  <Sheet v-if="showPasteText" v-model:open="mobilePasteOpen">
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

  <!-- Hidden measurement container: renders each collapsible button in both
       icon-only and with-label forms so we can read their real offsetWidth. -->
  <div ref="measureRef" aria-hidden="true" class="fixed pointer-events-none" style="visibility: hidden; top: -9999px; left: -9999px; white-space: nowrap;">
    <div class="flex items-center gap-0.5 sm:gap-1.5 px-2 sm:px-4 py-1 sm:py-1.5">
      <!-- MSD -->
      <Button data-measure="msd-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" /></Button>
      <Button data-measure="msd-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" />{{ t('actionbar.virtualMedia') }}</Button>
      <!-- ATX -->
      <Button data-measure="atx-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Power class="h-4 w-4" /></Button>
      <Button data-measure="atx-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Power class="h-4 w-4" />{{ t('actionbar.power') }}</Button>
      <!-- Paste -->
      <Button data-measure="paste-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><ClipboardPaste class="h-4 w-4" /></Button>
      <Button data-measure="paste-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><ClipboardPaste class="h-4 w-4" />{{ t('actionbar.paste') }}</Button>
      <!-- Stats -->
      <Button data-measure="stats-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><BarChart3 class="h-4 w-4" /></Button>
      <Button data-measure="stats-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><BarChart3 class="h-4 w-4" />{{ t('actionbar.stats') }}</Button>
      <!-- Web Terminal -->
      <Button data-measure="terminal-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Terminal class="h-4 w-4" /></Button>
      <Button data-measure="terminal-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Terminal class="h-4 w-4" />{{ t('actionbar.webTerminal') }}</Button>
      <!-- Settings -->
      <Button data-measure="settings-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Settings class="h-4 w-4" /></Button>
      <Button data-measure="settings-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Settings class="h-4 w-4" />{{ t('actionbar.settings') }}</Button>
      <!-- Always-visible items (for measuring their actual width) -->
      <Button data-measure="video-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" /></Button>
      <Button data-measure="video-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" /></Button>
      <Button data-measure="audio-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" /></Button>
      <Button data-measure="audio-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" /></Button>
      <Button data-measure="hid-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" /></Button>
      <Button data-measure="hid-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="h-4 w-4" /></Button>
    </div>
  </div>
</template>
