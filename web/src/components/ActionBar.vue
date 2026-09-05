<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useSystemStore } from '@/stores/system'
import type { VideoScaleMode } from '@/composables/useVideoScaling'
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
  Settings,
  Power,
  BarChart3,
  Terminal,
  MoreHorizontal,
  Bot,
  ChevronUp,
  ChevronDown,
  Keyboard,
  Scaling,
} from 'lucide-vue-next'
import PasteModal from '@/components/PasteModal.vue'
import AtxPopover from '@/components/AtxPopover.vue'
import VideoConfigPopover, { type VideoMode } from '@/components/VideoConfigPopover.vue'
import HidConfigPopover from '@/components/HidConfigPopover.vue'
import AudioConfigPopover from '@/components/AudioConfigPopover.vue'
import MsdDialog from '@/components/MsdDialog.vue'
import VideoDisplayControls from '@/components/VideoDisplayControls.vue'
import type { ConsoleLayout } from '@/composables/useConsoleLayout'

const { t, locale } = useI18n()
const router = useRouter()
const systemStore = useSystemStore()

const overflowMenuOpen = ref(false)

const hidBackend = computed(() => (systemStore.hid?.backend ?? '').toLowerCase())
const isCh9329Backend = computed(() => hidBackend.value.includes('ch9329'))
const showMsd = computed(() => {
  return !!systemStore.msd?.available && !isCh9329Backend.value
})
const props = defineProps<{
  layout?: ConsoleLayout
  mouseMode?: 'absolute' | 'relative'
  videoMode?: VideoMode
  ttydRunning?: boolean
  showPower?: boolean
  showTerminal?: boolean
  showComputerUse?: boolean
  showPasteText?: boolean
  showMic?: boolean
  scaleMode?: VideoScaleMode
  sourceSizeAvailable?: boolean
}>()
const isSidebarLayout = computed(() => props.layout === 'sidebar')
const isFloatingLayout = computed(() => props.layout === 'floating')
const floatingCollapsed = ref(false)
const expandButtonRef = ref<InstanceType<typeof Button> | null>(null)
const collapseButtonRef = ref<InstanceType<typeof Button> | null>(null)

async function setFloatingCollapsed(collapsed: boolean) {
  floatingCollapsed.value = collapsed
  await nextTick()
  const target = collapsed ? expandButtonRef.value : collapseButtonRef.value
  target?.$el?.focus()
}
const showAtx = computed(() => props.showPower !== false)
const showStats = computed(() => (props.videoMode ?? 'mjpeg') !== 'mjpeg')
const showPasteText = computed(() => props.showPasteText !== false)
const showMic = computed(() => props.showMic === true)


const emit = defineEmits<{
  (e: 'toggleFullscreen'): void
  (e: 'update:scaleMode', mode: VideoScaleMode): void
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
const barHeight = ref(0)
const coreWidth = ref(0)
const coreHeight = ref(0)
const fixedHeight = ref(0)
const actionHeight = ref(36)
const minimalDisplayControls = computed(() => isFloatingLayout.value && barWidth.value < 640)
const alwaysRightWidth = ref(152)
let layoutResizeObserver: ResizeObserver | null = null

type CollapsibleItem =
  | 'msd' | 'atx' | 'paste'
  | 'stats' | 'terminal' | 'settings' | 'ai'

interface ItemSpec {
  id: CollapsibleItem
  side: 'left' | 'right'
}

const ITEM_SPECS: ItemSpec[] = [
  { id: 'msd',       side: 'left' },
  { id: 'atx',       side: 'left' },
  { id: 'paste',     side: 'left' },
  { id: 'stats',     side: 'right' },
  { id: 'terminal',  side: 'right' },
  { id: 'settings',  side: 'right' },
  { id: 'ai',        side: 'right' },
]

const measuredWidths = ref<Map<CollapsibleItem, { icon: number; label: number }>>(new Map())
const measurementReady = ref(false)

const measureLayout = async () => {
  await nextTick()
  const bar = barRef.value
  const measureContainer = measureRef.value
  if (!bar || !measureContainer) return

  const style = window.getComputedStyle(bar)
  barWidth.value = bar.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight)
  barHeight.value = bar.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom)
  const core = Array.from(bar.querySelectorAll<HTMLElement>('[data-core-action]'))
  coreWidth.value = core.reduce((sum, element) => sum + element.offsetWidth, 0)
  coreHeight.value = core.reduce((sum, element) => sum + element.offsetHeight, 0)
  actionHeight.value = core[0]?.offsetHeight || 36

  const newWidths = new Map<CollapsibleItem, { icon: number; label: number }>()
  for (const spec of ITEM_SPECS) {
    const iconEl = measureContainer.querySelector(`[data-measure="${spec.id}-icon"]`) as HTMLElement
    const labelEl = measureContainer.querySelector(`[data-measure="${spec.id}-label"]`) as HTMLElement
    if (iconEl && labelEl) {
      newWidths.set(spec.id, {
        icon: Math.ceil(iconEl.offsetWidth) + 8,
        label: Math.ceil(labelEl.offsetWidth) + 8,
      })
    }
  }
  measuredWidths.value = newWidths

  const elements = Array.from(bar.querySelectorAll('[data-fixed-action]')) as HTMLElement[]
  const width = elements.reduce((sum, element) => {
    const style = window.getComputedStyle(element)
    return sum
      + element.getBoundingClientRect().width
      + Number.parseFloat(style.marginLeft || '0')
      + Number.parseFloat(style.marginRight || '0')
  }, 0)
  alwaysRightWidth.value = Math.ceil(width)
  fixedHeight.value = elements.reduce((sum, element) => {
    const style = window.getComputedStyle(element)
    return sum + element.getBoundingClientRect().height
      + parseFloat(style.marginTop || '0') + parseFloat(style.marginBottom || '0')
  }, 0)

  measurementReady.value = true
}

const observeLayout = async () => {
  await measureLayout()
  layoutResizeObserver?.disconnect()
  layoutResizeObserver = new ResizeObserver(() => {
    void measureLayout()
  })
  if (barRef.value) layoutResizeObserver.observe(barRef.value)
  barRef.value?.querySelectorAll('[data-fixed-action], [data-core-action]').forEach((element) => {
    layoutResizeObserver?.observe(element)
  })
}

onMounted(() => {
  void observeLayout()
})

onUnmounted(() => {
  layoutResizeObserver?.disconnect()
})

watch(locale, () => {
  measurementReady.value = false
  void measureLayout()
})

watch([() => props.layout, () => props.showComputerUse, floatingCollapsed, minimalDisplayControls], () => {
  void observeLayout()
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

const OVERFLOW_BUTTON_BUDGET_PX = 36

const collapsibleItems = computed(() => {
  const items = ITEM_SPECS.filter(item => {
    if (isFloatingLayout.value && item.side === 'right') return false
    if (item.id === 'msd' && !showMsd.value) return false
    if (item.id === 'atx' && !showAtx.value) return false
    if (item.id === 'paste' && !showPasteText.value) return false
    if (item.id === 'stats' && !showStats.value) return false
    if (item.id === 'terminal' && props.showTerminal === false) return false
    if (item.id === 'ai' && props.showComputerUse === false) return false
    return true
  })
  return items
})

const visibleSet = computed(() => {
  const result = new Map<CollapsibleItem, 'icon' | 'label'>()
  if (!measurementReady.value) return result

  if (isSidebarLayout.value) {
    // Reserve More and a gap between the two groups before assigning vertical slots.
    let available = barHeight.value - coreHeight.value - fixedHeight.value - actionHeight.value - 16
    const priority: CollapsibleItem[] = ['paste', 'settings', 'msd', 'atx', 'stats', 'terminal', 'ai']
    for (const id of priority) {
      if (!collapsibleItems.value.some(item => item.id === id)) continue
      if (available < actionHeight.value) break
      result.set(id, 'icon')
      available -= actionHeight.value
    }
    return result
  }

  const available = barWidth.value - alwaysRightWidth.value - Math.max(OVERFLOW_BUTTON_BUDGET_PX, actionHeight.value) - (isFloatingLayout.value ? 48 : 12)
  let used = coreWidth.value
  // Keep actions reachable before spending the remaining space on labels.
  for (const item of collapsibleItems.value) {
    const widths = measuredWidths.value.get(item.id)
    if (!widths || used + widths.icon > available) continue
    result.set(item.id, 'icon')
    used += widths.icon
  }
  if (isFloatingLayout.value && barWidth.value < 640) return result
  for (const item of collapsibleItems.value) {
    const widths = measuredWidths.value.get(item.id)
    if (!widths || !result.has(item.id)) continue
    const extra = widths.label - widths.icon
    if (used + extra <= available) {
      result.set(item.id, 'label')
      used += extra
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
  <div
    :class="[
      'console-action-bar bg-background',
      props.layout === 'floating' && 'console-action-bar--floating',
      isFloatingLayout && floatingCollapsed && 'console-action-bar--collapsed',
      props.layout === 'sidebar' && 'console-action-bar--sidebar',
      (!props.layout || props.layout === 'current') && 'w-full border-b',
    ]"
  >
    <Button
      v-if="isFloatingLayout && floatingCollapsed"
      ref="expandButtonRef"
      variant="ghost"
      size="sm"
      class="console-action-bar__expand gap-1.5 rounded-xl text-xs"
      :aria-label="t('actionbar.expandToolbar')"
      :aria-expanded="false"
      @click="setFloatingCollapsed(false)"
    >
      <ChevronDown class="size-4" />{{ t('actionbar.expandToolbar') }}
    </Button>
    <div
      v-show="!isFloatingLayout || !floatingCollapsed"
      ref="barRef"
      class="console-action-bar__inner flex items-center"
      :class="isSidebarLayout
        ? 'h-full flex-col px-1 py-2 sm:px-1.5'
        : 'px-2 py-1 sm:px-4 sm:py-1.5'"
    >
      <!-- Left side buttons -->
      <ButtonGroup
        class="left-buttons min-w-0"
        :class="isSidebarLayout
          ? 'flex-none flex-col overflow-visible'
          : 'flex-1 overflow-hidden'"
        :orientation="isSidebarLayout ? 'vertical' : 'horizontal'"
      >
        <!-- Video Config - Always visible -->
        <div data-core-action class="flex shrink-0">
          <VideoConfigPopover
            v-model:open="videoPopoverOpen"
            :video-mode="props.videoMode || 'mjpeg'"
            :side="isSidebarLayout ? 'right' : 'bottom'"
            @update:video-mode="emit('update:videoMode', $event)"
          />
        </div>

        <!-- Audio Config - Always visible -->
        <div data-core-action class="flex shrink-0">
          <AudioConfigPopover
            v-model:open="audioPopoverOpen"
            :microphone-enabled="showMic"
            :side="isSidebarLayout ? 'right' : 'bottom'"
          />
        </div>

        <!-- HID Config - Always visible -->
        <div data-core-action class="flex shrink-0">
          <HidConfigPopover
            v-model:open="hidPopoverOpen"
            :mouse-mode="mouseMode"
            :side="isSidebarLayout ? 'right' : 'bottom'"
            @update:mouse-mode="emit('toggleMouseMode')"
          />
        </div>

        <!-- Virtual Media (MSD) - Adaptive -->
        <div v-if="showMsd && isVisible('msd')">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-8 gap-1.5 text-xs"
                  :aria-label="t('actionbar.virtualMedia')"
                  :title="t('actionbar.virtualMedia')"
                  @click="msdDialogOpen = true"
                >
                  <HardDrive class="size-4" />
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
              <Button
                variant="ghost"
                size="sm"
                class="h-8 gap-1.5 text-xs"
                :aria-label="t('actionbar.power')"
                :title="t('actionbar.power')"
              >
                <Power class="size-4" />
                <span v-if="visibleSet.get('atx') === 'label'">{{ t('actionbar.power') }}</span>
              </Button>
            </PopoverTrigger>
            <PopoverContent
              class="w-[min(280px,90vw)] p-0"
              align="start"
              :side="isSidebarLayout ? 'right' : 'bottom'"
            >
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
              <Button
                variant="ghost"
                size="sm"
                class="h-8 gap-1.5 text-xs"
                :aria-label="t('actionbar.paste')"
                :title="t('actionbar.paste')"
              >
                <ClipboardPaste class="size-4" />
                <span v-if="visibleSet.get('paste') === 'label'">{{ t('actionbar.paste') }}</span>
              </Button>
            </PopoverTrigger>
            <PopoverContent
              :class="isSidebarLayout
                ? 'w-[min(400px,calc(100vw-4.5rem))] p-0'
                : 'w-[min(400px,90vw)] p-0'"
              align="start"
              :side="isSidebarLayout ? 'right' : 'bottom'"
            >
              <PasteModal v-if="pasteOpen" @close="pasteOpen = false" />
            </PopoverContent>
          </Popover>
        </div>

      </ButtonGroup>

      <!-- Right side buttons -->
      <ButtonGroup
        class="shrink-0"
        :class="isSidebarLayout
          ? 'mt-auto flex-none flex-col'
          : 'ml-1 sm:ml-2'"
        :orientation="isSidebarLayout ? 'vertical' : 'horizontal'"
      >
        <VideoDisplayControls
          :minimal="minimalDisplayControls"
          :text-only-scale="isSidebarLayout"
          :scale-mode="props.scaleMode"
          :source-size-available="props.sourceSizeAvailable"
          @toggle-fullscreen="emit('toggleFullscreen')"
          @update:scale-mode="emit('update:scaleMode', $event)"
          @toggle-virtual-keyboard="emit('toggleVirtualKeyboard')"
        />

        <!-- Connection Stats - Adaptive -->
        <div v-if="isVisible('stats')">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" :aria-label="t('actionbar.stats')" @click="emit('toggleStats')">
                  <BarChart3 class="size-4" />
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
                  :aria-label="t('actionbar.webTerminal')"
                  @click="emit('openTerminal')"
                >
                  <Terminal class="size-4" />
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
        <TooltipProvider v-if="isVisible('ai')">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="sm"
                class="h-8 gap-1.5 text-xs"
                :aria-label="t('computerUse.title')"
                @click="emit('openComputerUse')"
              >
                <Bot class="size-3.5 sm:size-4" />
                <span v-if="visibleSet.get('ai') === 'label'">AI</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{{ t('computerUse.title') }}</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <!-- Settings - Adaptive -->
        <div v-if="isVisible('settings')">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="sm" class="h-8 gap-1.5 text-xs" :aria-label="t('actionbar.settings')" @click="router.push('/settings')">
                  <Settings class="size-4" />
                  <span v-if="visibleSet.get('settings') === 'label'">{{ t('actionbar.settings') }}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ t('actionbar.settingsTip') }}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <!-- Overflow Menu - Only show if there are overflowed items -->
        <DropdownMenu v-if="hasOverflow || minimalDisplayControls" v-model:open="overflowMenuOpen">
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="sm" class="size-8 p-0" :aria-label="t('actionbar.more')" :title="t('actionbar.more')">
              <MoreHorizontal class="size-3.5 sm:size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" :side="isSidebarLayout ? 'right' : 'bottom'" class="w-56 max-h-[70dvh] overflow-y-auto">
            <template v-if="minimalDisplayControls">
              <DropdownMenuItem @click="openFromOverflow(() => emit('toggleVirtualKeyboard'))">
                <Keyboard class="size-4 mr-2" />{{ t('actionbar.keyboard') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                :disabled="!props.sourceSizeAvailable"
                @click="emit('update:scaleMode', props.scaleMode === 'actual' ? 'fit' : 'actual')"
              >
                <Scaling class="size-4 mr-2" />{{ t(props.scaleMode === 'actual' ? 'actionbar.fitSizeAria' : 'actionbar.actualSizeAria') }}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
            </template>
            <!-- MSD -->
            <DropdownMenuItem v-if="showMsd && !isVisible('msd')" @click="openFromOverflow(() => msdDialogOpen = true)">
              <HardDrive class="size-4 mr-2" />
              {{ t('actionbar.virtualMedia') }}
            </DropdownMenuItem>

            <!-- ATX -->
            <DropdownMenuItem v-if="showAtx && !isVisible('atx')" @click="openMobileAtx">
              <Power class="size-4 mr-2" />
              {{ t('actionbar.power') }}
            </DropdownMenuItem>

            <!-- Paste -->
            <DropdownMenuItem v-if="showPasteText && !isVisible('paste')" @click="openMobilePaste">
              <ClipboardPaste class="size-4 mr-2" />
              {{ t('actionbar.paste') }}
            </DropdownMenuItem>

            <DropdownMenuSeparator v-if="hasLeftOverflow && hasRightOverflow" />

            <!-- Stats -->
            <DropdownMenuItem v-if="!isFloatingLayout && showStats && !isVisible('stats')" @click="openFromOverflow(() => emit('toggleStats'))">
              <BarChart3 class="size-4 mr-2" />
              {{ t('actionbar.stats') }}
            </DropdownMenuItem>

            <!-- Web Terminal -->
            <DropdownMenuItem
              v-if="!isFloatingLayout && props.showTerminal !== false && !isVisible('terminal')"
              :disabled="!props.ttydRunning"
              @click="openFromOverflow(() => emit('openTerminal'))"
            >
              <Terminal class="size-4 mr-2" />
              {{ t('actionbar.webTerminal') }}
            </DropdownMenuItem>

            <DropdownMenuItem v-if="!isFloatingLayout && props.showComputerUse !== false && !isVisible('ai')" @click="openFromOverflow(() => emit('openComputerUse'))">
              <Bot class="size-4 mr-2" />{{ t('computerUse.title') }}
            </DropdownMenuItem>

            <!-- Settings -->
            <DropdownMenuItem v-if="!isFloatingLayout && !isVisible('settings')" @click="openFromOverflow(() => router.push('/settings'))">
              <Settings class="size-4 mr-2" />
              {{ t('actionbar.settings') }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          v-if="isFloatingLayout"
          ref="collapseButtonRef"
          data-fixed-action
          variant="ghost"
          size="icon-sm"
          :aria-label="t('actionbar.collapseToolbar')"
          :title="t('actionbar.collapseToolbar')"
          :aria-expanded="true"
          @click="setFloatingCollapsed(true)"
        >
          <ChevronUp class="size-4" />
        </Button>
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
      <PasteModal v-if="mobilePasteOpen" @close="mobilePasteOpen = false" />
    </SheetContent>
  </Sheet>

  <!-- Hidden measurement container: renders each collapsible button in both
       icon-only and with-label forms so we can read their real offsetWidth. -->
  <div ref="measureRef" aria-hidden="true" class="fixed pointer-events-none" style="visibility: hidden; top: -9999px; left: -9999px; white-space: nowrap;">
    <div class="flex items-center gap-0.5 sm:gap-1.5 px-2 sm:px-4 py-1 sm:py-1.5">
      <!-- MSD -->
      <Button data-measure="msd-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="size-4" /></Button>
      <Button data-measure="msd-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><HardDrive class="size-4" />{{ t('actionbar.virtualMedia') }}</Button>
      <!-- ATX -->
      <Button data-measure="atx-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Power class="size-4" /></Button>
      <Button data-measure="atx-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Power class="size-4" />{{ t('actionbar.power') }}</Button>
      <!-- Paste -->
      <Button data-measure="paste-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><ClipboardPaste class="size-4" /></Button>
      <Button data-measure="paste-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><ClipboardPaste class="size-4" />{{ t('actionbar.paste') }}</Button>
      <!-- Stats -->
      <Button data-measure="stats-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><BarChart3 class="size-4" /></Button>
      <Button data-measure="stats-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><BarChart3 class="size-4" />{{ t('actionbar.stats') }}</Button>
      <!-- Web Terminal -->
      <Button data-measure="terminal-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Terminal class="size-4" /></Button>
      <Button data-measure="terminal-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Terminal class="size-4" />{{ t('actionbar.webTerminal') }}</Button>
      <!-- Settings -->
      <Button data-measure="settings-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Settings class="size-4" /></Button>
      <Button data-measure="settings-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Settings class="size-4" />{{ t('actionbar.settings') }}</Button>
      <Button data-measure="ai-icon" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Bot class="size-4" /></Button>
      <Button data-measure="ai-label" variant="ghost" size="sm" class="h-8 gap-1.5 text-xs"><Bot class="size-4" />AI</Button>
    </div>
  </div>
</template>

<style scoped>
.console-action-bar--floating {
  position: relative;
  z-index: 40;
  width: 100%;
  max-width: 64rem;
  border: 1px solid var(--border);
  border-radius: 1rem;
  background: color-mix(in srgb, var(--background) 96%, transparent);
  box-shadow: 0 2px 8px rgb(0 0 0 / 8%);
  backdrop-filter: blur(14px);
}

.console-action-bar--floating .console-action-bar__inner {
  border-radius: inherit;
}

.console-action-bar--sidebar {
  position: absolute;
  z-index: 40;
  top: 2.5rem;
  bottom: 1.75rem;
  left: 0;
  width: 3.5rem;
  border-right: 1px solid var(--border);
  background: color-mix(in srgb, var(--background) 94%, transparent);
  backdrop-filter: blur(12px);
}

.console-action-bar--sidebar :deep([data-slot='button-group']) {
  width: 100%;
}

.console-action-bar--sidebar :deep(button) {
  height: 36px;
  flex-shrink: 0;
  width: 100%;
  min-width: 0;
  gap: 0;
  overflow: hidden;
  padding-inline: 0.5rem;
  border-radius: 0.5rem;
}

.console-action-bar--sidebar :deep(button > span) {
  display: none;
}

.console-action-bar--sidebar :deep([data-fixed-action][aria-hidden='true']) {
  width: 1.5rem;
  height: 1px;
  margin: 0.35rem auto;
}

.console-action-bar--sidebar .console-action-bar__inner {
  overflow-y: auto;
  gap: 1rem;
  scrollbar-width: thin;
}

.console-action-bar--floating .console-action-bar__inner {
  padding: 4px 12px;
  gap: 8px;
}

.console-action-bar--floating :deep([data-slot='button-group']) {
  gap: 4px;
}

.console-action-bar--floating .console-action-bar__inner :deep(button) {
  min-width: 36px;
  height: 38px;
  padding-inline: 10px;
  flex-shrink: 0;
  border-radius: 10px;
}

.console-action-bar--floating .console-action-bar__inner :deep(button[aria-expanded='true']) {
  background: var(--accent);
}

@media (max-width: 639px) {
  .console-action-bar--floating .console-action-bar__inner {
    padding-inline: 4px;
    gap: 0;
  }
  .console-action-bar--floating :deep([data-slot='button-group']) {
    gap: 0;
  }
  .console-action-bar--floating .console-action-bar__inner :deep(button) {
    width: 36px;
    padding-inline: 0;
  }
}

.console-action-bar--floating.console-action-bar--collapsed {
  width: auto;
}

@media (pointer: coarse) {
  .console-action-bar__expand {
    min-height: 44px;
  }
  .console-action-bar--sidebar :deep(button) {
    height: 44px;
  }
  .console-action-bar--floating .console-action-bar__inner :deep(button) {
    min-width: 44px;
    height: 44px;
  }
  .console-action-bar--floating .console-action-bar__inner {
    padding-inline: 4px;
  }
}

@media (min-width: 640px) {
  .console-action-bar--sidebar {
    top: 3.5rem;
    width: 4rem;
  }
}
</style>
