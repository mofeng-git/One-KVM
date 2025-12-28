<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import Keyboard from 'simple-keyboard'
import 'simple-keyboard/build/css/index.css'
import { hidApi } from '@/api'
import {
  keys,
  latchingKeys,
  modifiers,
  type KeyName,
} from '@/lib/keyboardMappings'

const props = defineProps<{
  visible: boolean
  attached?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'update:attached', value: boolean): void
  (e: 'keyDown', key: string): void
  (e: 'keyUp', key: string): void
}>()

const { t } = useI18n()

// State
const isAttached = ref(props.attached ?? true)

// Keyboard instances
const mainKeyboard = ref<Keyboard | null>(null)
const controlKeyboard = ref<Keyboard | null>(null)
const arrowsKeyboard = ref<Keyboard | null>(null)

// Pressed keys tracking
const pressedModifiers = ref<number>(0)
const keysDown = ref<string[]>([])

// Shift state for display
const isShiftActive = computed(() => {
  return (pressedModifiers.value & 0x22) !== 0
})

const layoutName = computed(() => {
  return isShiftActive.value ? 'shift' : 'default'
})

// Keys currently pressed (for highlighting)
const keyNamesForDownKeys = computed(() => {
  const activeModifierMask = pressedModifiers.value || 0
  const modifierNames = Object.entries(modifiers)
    .filter(([_, mask]) => (activeModifierMask & mask) !== 0)
    .map(([name]) => name)

  return [...modifierNames, ...keysDown.value, ' ']
})

// Dragging state (for floating mode)
const keyboardRef = ref<HTMLDivElement | null>(null)
const isDragging = ref(false)
const dragOffset = ref({ x: 0, y: 0 })
const position = ref({ x: 100, y: 100 })

// Unique ID for this keyboard instance
const keyboardId = ref(`kb-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`)

// Keyboard layouts - matching JetKVM style
const keyboardLayout = {
  main: {
    default: [
      'CtrlAltDelete AltMetaEscape CtrlAltBackspace',
      'Escape F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12',
      'Backquote Digit1 Digit2 Digit3 Digit4 Digit5 Digit6 Digit7 Digit8 Digit9 Digit0 Minus Equal Backspace',
      'Tab KeyQ KeyW KeyE KeyR KeyT KeyY KeyU KeyI KeyO KeyP BracketLeft BracketRight Backslash',
      'CapsLock KeyA KeyS KeyD KeyF KeyG KeyH KeyJ KeyK KeyL Semicolon Quote Enter',
      'ShiftLeft KeyZ KeyX KeyC KeyV KeyB KeyN KeyM Comma Period Slash ShiftRight',
      'ControlLeft MetaLeft AltLeft Space AltGr MetaRight Menu ControlRight',
    ],
    shift: [
      'CtrlAltDelete AltMetaEscape CtrlAltBackspace',
      'Escape F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12',
      '(Backquote) (Digit1) (Digit2) (Digit3) (Digit4) (Digit5) (Digit6) (Digit7) (Digit8) (Digit9) (Digit0) (Minus) (Equal) Backspace',
      'Tab (KeyQ) (KeyW) (KeyE) (KeyR) (KeyT) (KeyY) (KeyU) (KeyI) (KeyO) (KeyP) (BracketLeft) (BracketRight) (Backslash)',
      'CapsLock (KeyA) (KeyS) (KeyD) (KeyF) (KeyG) (KeyH) (KeyJ) (KeyK) (KeyL) (Semicolon) (Quote) Enter',
      'ShiftLeft (KeyZ) (KeyX) (KeyC) (KeyV) (KeyB) (KeyN) (KeyM) (Comma) (Period) (Slash) ShiftRight',
      'ControlLeft MetaLeft AltLeft Space AltGr MetaRight Menu ControlRight',
    ],
  },
  control: {
    default: [
      'PrintScreen ScrollLock Pause',
      'Insert Home PageUp',
      'Delete End PageDown',
    ],
  },
  arrows: {
    default: [
      'ArrowUp',
      'ArrowLeft ArrowDown ArrowRight',
    ],
  },
}

// Key display mapping with Unicode symbols (JetKVM style)
const keyDisplayMap: Record<string, string> = {
  // Macros - compact format
  CtrlAltDelete: 'Ctrl+Alt+Del',
  AltMetaEscape: 'Alt+Meta+Esc',
  CtrlAltBackspace: 'Ctrl+Alt+Bksp',

  // Modifiers with symbols
  ControlLeft: '^Ctrl',
  ControlRight: 'Ctrl^',
  ShiftLeft: '⇧Shift',
  ShiftRight: 'Shift⇧',
  AltLeft: '⌥Alt',
  AltGr: 'AltGr',
  MetaLeft: '⌘Meta',
  MetaRight: 'Meta⌘',
  Menu: 'Menu',

  // Special keys with symbols
  Escape: 'Esc',
  Backspace: '⌫',
  Tab: '⇥Tab',
  CapsLock: '⇪Caps',
  Enter: '↵',
  Space: ' ',

  // Navigation with symbols
  Insert: 'Ins',
  Delete: '⌫Del',
  Home: 'Home',
  End: 'End',
  PageUp: 'PgUp',
  PageDown: 'PgDn',

  // Arrows
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',

  // Control cluster
  PrintScreen: 'PrtSc',
  ScrollLock: 'ScrLk',
  Pause: 'Pause',

  // Function keys
  F1: 'F1', F2: 'F2', F3: 'F3', F4: 'F4',
  F5: 'F5', F6: 'F6', F7: 'F7', F8: 'F8',
  F9: 'F9', F10: 'F10', F11: 'F11', F12: 'F12',

  // Letters
  KeyA: 'a', KeyB: 'b', KeyC: 'c', KeyD: 'd', KeyE: 'e',
  KeyF: 'f', KeyG: 'g', KeyH: 'h', KeyI: 'i', KeyJ: 'j',
  KeyK: 'k', KeyL: 'l', KeyM: 'm', KeyN: 'n', KeyO: 'o',
  KeyP: 'p', KeyQ: 'q', KeyR: 'r', KeyS: 's', KeyT: 't',
  KeyU: 'u', KeyV: 'v', KeyW: 'w', KeyX: 'x', KeyY: 'y',
  KeyZ: 'z',

  // Capital letters
  '(KeyA)': 'A', '(KeyB)': 'B', '(KeyC)': 'C', '(KeyD)': 'D', '(KeyE)': 'E',
  '(KeyF)': 'F', '(KeyG)': 'G', '(KeyH)': 'H', '(KeyI)': 'I', '(KeyJ)': 'J',
  '(KeyK)': 'K', '(KeyL)': 'L', '(KeyM)': 'M', '(KeyN)': 'N', '(KeyO)': 'O',
  '(KeyP)': 'P', '(KeyQ)': 'Q', '(KeyR)': 'R', '(KeyS)': 'S', '(KeyT)': 'T',
  '(KeyU)': 'U', '(KeyV)': 'V', '(KeyW)': 'W', '(KeyX)': 'X', '(KeyY)': 'Y',
  '(KeyZ)': 'Z',

  // Numbers
  Digit1: '1', Digit2: '2', Digit3: '3', Digit4: '4', Digit5: '5',
  Digit6: '6', Digit7: '7', Digit8: '8', Digit9: '9', Digit0: '0',

  // Shifted Numbers
  '(Digit1)': '!', '(Digit2)': '@', '(Digit3)': '#', '(Digit4)': '$', '(Digit5)': '%',
  '(Digit6)': '^', '(Digit7)': '&', '(Digit8)': '*', '(Digit9)': '(', '(Digit0)': ')',

  // Symbols
  Minus: '-', '(Minus)': '_',
  Equal: '=', '(Equal)': '+',
  BracketLeft: '[', '(BracketLeft)': '{',
  BracketRight: ']', '(BracketRight)': '}',
  Backslash: '\\', '(Backslash)': '|',
  Semicolon: ';', '(Semicolon)': ':',
  Quote: "'", '(Quote)': '"',
  Comma: ',', '(Comma)': '<',
  Period: '.', '(Period)': '>',
  Slash: '/', '(Slash)': '?',
  Backquote: '`', '(Backquote)': '~',
}

// Key press handler
async function onKeyDown(key: string) {
  // Handle macro keys
  if (key === 'CtrlAltDelete') {
    await executeMacro([
      { keys: ['Delete'], modifiers: ['ControlLeft', 'AltLeft'] },
    ])
    return
  }

  if (key === 'AltMetaEscape') {
    await executeMacro([
      { keys: ['Escape'], modifiers: ['AltLeft', 'MetaLeft'] },
    ])
    return
  }

  if (key === 'CtrlAltBackspace') {
    await executeMacro([
      { keys: ['Backspace'], modifiers: ['ControlLeft', 'AltLeft'] },
    ])
    return
  }

  // Clean key name (remove parentheses for shifted keys)
  const cleanKey = key.replace(/[()]/g, '')

  // Check if key exists
  if (!(cleanKey in keys)) {
    console.warn(`[VirtualKeyboard] Unknown key: ${cleanKey}`)
    return
  }

  const keyCode = keys[cleanKey as KeyName]

  // Handle latching keys (Caps Lock, etc.)
  if ((latchingKeys as readonly string[]).includes(cleanKey)) {
    emit('keyDown', cleanKey)
    await sendKeyPress(keyCode, true)
    setTimeout(() => {
      sendKeyPress(keyCode, false)
      emit('keyUp', cleanKey)
    }, 100)
    return
  }

  // Handle modifier keys (toggle)
  if (cleanKey in modifiers) {
    const mask = modifiers[cleanKey as keyof typeof modifiers]
    const isCurrentlyDown = (pressedModifiers.value & mask) !== 0

    if (isCurrentlyDown) {
      pressedModifiers.value &= ~mask
      await sendKeyPress(keyCode, false)
      emit('keyUp', cleanKey)
    } else {
      pressedModifiers.value |= mask
      await sendKeyPress(keyCode, true)
      emit('keyDown', cleanKey)
    }
    updateKeyboardButtonTheme()
    return
  }

  // Regular key: press and release
  keysDown.value.push(cleanKey)
  emit('keyDown', cleanKey)
  await sendKeyPress(keyCode, true)
  updateKeyboardButtonTheme()
  setTimeout(async () => {
    keysDown.value = keysDown.value.filter(k => k !== cleanKey)
    await sendKeyPress(keyCode, false)
    emit('keyUp', cleanKey)
    updateKeyboardButtonTheme()
  }, 50)
}

async function onKeyUp() {
  // Not used for now - we handle up in onKeyDown with setTimeout
}

async function sendKeyPress(keyCode: number, press: boolean) {
  try {
    const mods = {
      ctrl: (pressedModifiers.value & 0x11) !== 0,
      shift: (pressedModifiers.value & 0x22) !== 0,
      alt: (pressedModifiers.value & 0x44) !== 0,
      meta: (pressedModifiers.value & 0x88) !== 0,
    }

    await hidApi.keyboard(press ? 'down' : 'up', keyCode, mods)
  } catch (err) {
    console.error('[VirtualKeyboard] Key send failed:', err)
  }
}

interface MacroStep {
  keys: string[]
  modifiers: string[]
}

async function executeMacro(steps: MacroStep[]) {
  for (const step of steps) {
    for (const mod of step.modifiers) {
      if (mod in keys) {
        await sendKeyPress(keys[mod as KeyName], true)
      }
    }

    for (const key of step.keys) {
      if (key in keys) {
        await sendKeyPress(keys[key as KeyName], true)
      }
    }

    await new Promise(resolve => setTimeout(resolve, 50))

    for (const key of step.keys) {
      if (key in keys) {
        await sendKeyPress(keys[key as KeyName], false)
      }
    }

    for (const mod of step.modifiers) {
      if (mod in keys) {
        await sendKeyPress(keys[mod as KeyName], false)
      }
    }
  }
}

// Update keyboard button theme for pressed keys
function updateKeyboardButtonTheme() {
  const downKeys = keyNamesForDownKeys.value.join(' ')
  const buttonTheme = [
    {
      class: 'combination-key',
      buttons: 'CtrlAltDelete AltMetaEscape CtrlAltBackspace',
    },
    {
      class: 'down-key',
      buttons: downKeys,
    },
  ]

  mainKeyboard.value?.setOptions({ buttonTheme })
  controlKeyboard.value?.setOptions({ buttonTheme })
  arrowsKeyboard.value?.setOptions({ buttonTheme })
}

// Update layout when shift state changes
watch(layoutName, (name) => {
  mainKeyboard.value?.setOptions({ layoutName: name })
})

// Initialize keyboards with unique selectors
function initKeyboards() {
  const id = keyboardId.value

  // Check if elements exist - use full selector with #
  const mainEl = document.querySelector(`#${id}-main`)
  const controlEl = document.querySelector(`#${id}-control`)
  const arrowsEl = document.querySelector(`#${id}-arrows`)

  if (!mainEl || !controlEl || !arrowsEl) {
    console.warn('[VirtualKeyboard] DOM elements not ready, retrying...', id)
    setTimeout(initKeyboards, 50)
    return
  }

  // Main keyboard - pass element directly instead of selector string
  mainKeyboard.value = new Keyboard(mainEl, {
    layout: keyboardLayout.main,
    layoutName: layoutName.value,
    display: keyDisplayMap,
    theme: 'hg-theme-default hg-layout-default vkb-keyboard',
    onKeyPress: onKeyDown,
    onKeyReleased: onKeyUp,
    buttonTheme: [
      {
        class: 'combination-key',
        buttons: 'CtrlAltDelete AltMetaEscape CtrlAltBackspace',
      },
    ],
    disableButtonHold: true,
    preventMouseDownDefault: true,
    preventMouseUpDefault: true,
    stopMouseDownPropagation: true,
    stopMouseUpPropagation: true,
  })

  // Control keyboard
  controlKeyboard.value = new Keyboard(controlEl, {
    layout: keyboardLayout.control,
    layoutName: 'default',
    display: keyDisplayMap,
    theme: 'hg-theme-default hg-layout-default vkb-keyboard',
    onKeyPress: onKeyDown,
    onKeyReleased: onKeyUp,
    disableButtonHold: true,
    preventMouseDownDefault: true,
    preventMouseUpDefault: true,
    stopMouseDownPropagation: true,
    stopMouseUpPropagation: true,
  })

  // Arrows keyboard
  arrowsKeyboard.value = new Keyboard(arrowsEl, {
    layout: keyboardLayout.arrows,
    layoutName: 'default',
    display: keyDisplayMap,
    theme: 'hg-theme-default hg-layout-default vkb-keyboard',
    onKeyPress: onKeyDown,
    onKeyReleased: onKeyUp,
    disableButtonHold: true,
    preventMouseDownDefault: true,
    preventMouseUpDefault: true,
    stopMouseDownPropagation: true,
    stopMouseUpPropagation: true,
  })

  console.log('[VirtualKeyboard] Keyboards initialized:', id)
}

// Destroy keyboards
function destroyKeyboards() {
  mainKeyboard.value?.destroy()
  controlKeyboard.value?.destroy()
  arrowsKeyboard.value?.destroy()
  mainKeyboard.value = null
  controlKeyboard.value = null
  arrowsKeyboard.value = null
}

// Dragging handlers
function getClientCoords(e: MouseEvent | TouchEvent): { x: number; y: number } | null {
  if ('touches' in e) {
    const touch = e.touches[0]
    return touch ? { x: touch.clientX, y: touch.clientY } : null
  }
  return { x: e.clientX, y: e.clientY }
}

function startDrag(e: MouseEvent | TouchEvent) {
  if (isAttached.value || !keyboardRef.value) return

  const coords = getClientCoords(e)
  if (!coords) return

  isDragging.value = true
  const rect = keyboardRef.value.getBoundingClientRect()
  dragOffset.value = {
    x: coords.x - rect.left,
    y: coords.y - rect.top,
  }
}

function onDrag(e: MouseEvent | TouchEvent) {
  if (!isDragging.value || !keyboardRef.value) return

  const coords = getClientCoords(e)
  if (!coords) return

  const rect = keyboardRef.value.getBoundingClientRect()
  const maxX = window.innerWidth - rect.width
  const maxY = window.innerHeight - rect.height

  position.value = {
    x: Math.min(maxX, Math.max(0, coords.x - dragOffset.value.x)),
    y: Math.min(maxY, Math.max(0, coords.y - dragOffset.value.y)),
  }
}

function endDrag() {
  isDragging.value = false
}

async function toggleAttached() {
  destroyKeyboards()
  isAttached.value = !isAttached.value
  emit('update:attached', isAttached.value)

  // Wait for Teleport to move the component
  await nextTick()
  await nextTick() // Extra tick for Teleport

  // Reinitialize keyboards in new location
  setTimeout(() => {
    initKeyboards()
  }, 100)
}

function close() {
  emit('update:visible', false)
}

// Watch visibility to init/destroy keyboards
watch(() => props.visible, async (visible) => {
  console.log('[VirtualKeyboard] Visibility changed:', visible, 'attached:', isAttached.value, 'id:', keyboardId.value)
  if (visible) {
    await nextTick()
    initKeyboards()
  } else {
    destroyKeyboards()
  }
}, { immediate: true })

watch(() => props.attached, (value) => {
  if (value !== undefined) {
    isAttached.value = value
  }
})

onMounted(() => {
  document.addEventListener('mousemove', onDrag)
  document.addEventListener('touchmove', onDrag)
  document.addEventListener('mouseup', endDrag)
  document.addEventListener('touchend', endDrag)
})

onUnmounted(() => {
  document.removeEventListener('mousemove', onDrag)
  document.removeEventListener('touchmove', onDrag)
  document.removeEventListener('mouseup', endDrag)
  document.removeEventListener('touchend', endDrag)
  destroyKeyboards()
})
</script>

<template>
  <Transition name="keyboard-fade">
    <div
      v-if="visible"
      :id="keyboardId"
      ref="keyboardRef"
      class="vkb"
      :class="{
        'vkb--attached': isAttached,
        'vkb--floating': !isAttached,
        'vkb--dragging': isDragging,
      }"
      :style="!isAttached ? { transform: `translate(${position.x}px, ${position.y}px)` } : undefined"
    >
      <!-- Header -->
      <div
        class="vkb-header"
        @mousedown="startDrag"
        @touchstart="startDrag"
      >
        <button class="vkb-btn" @click="toggleAttached">
          {{ isAttached ? t('virtualKeyboard.detach') : t('virtualKeyboard.attach') }}
        </button>
        <span class="vkb-title">{{ t('virtualKeyboard.title') }}</span>
        <button class="vkb-btn" @click="close">
          {{ t('virtualKeyboard.hide') }}
        </button>
      </div>

      <!-- Keyboard body -->
      <div class="vkb-body">
        <div :id="`${keyboardId}-main`" class="kb-main-container"></div>
        <div class="vkb-side">
          <div :id="`${keyboardId}-control`" class="kb-control-container"></div>
          <div :id="`${keyboardId}-arrows`" class="kb-arrows-container"></div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style>
/* Simple-keyboard global overrides */
.vkb .simple-keyboard.hg-theme-default {
  font-family: inherit;
  background: transparent;
  padding: 0;
}

.vkb .simple-keyboard .hg-button {
  height: 36px;
  border-radius: 6px;
  background: var(--keyboard-button-bg, white);
  color: var(--keyboard-button-color, #1f2937);
  border: 1px solid var(--keyboard-button-border, #e5e7eb);
  border-bottom-width: 2px;
  box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);
  font-size: 12px;
  font-weight: 500;
  padding: 0 6px;
  margin: 0 2px 4px 0;
  /* Key sizing for alignment */
  flex-grow: 1;
  flex-shrink: 1;
  flex-basis: 0;
  min-width: 40px;
}

.vkb .simple-keyboard .hg-button:hover {
  background: var(--keyboard-button-hover-bg, #f3f4f6);
}

.vkb .simple-keyboard .hg-button:active {
  background: #3b82f6;
  color: white;
  border-bottom-width: 1px;
  margin-top: 1px;
}

.vkb .simple-keyboard .hg-button span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Combination/macro keys */
.vkb .simple-keyboard .hg-button.combination-key {
  font-size: 10px;
  height: 28px;
  min-width: auto !important;
  max-width: fit-content !important;
  flex-grow: 0 !important;
  padding: 0 8px;
}

/* Pressed keys */
.vkb .simple-keyboard .hg-button.down-key {
  background: #3b82f6;
  color: white;
  font-weight: 600;
  border-color: #2563eb;
}

/* Space bar */
.vkb .simple-keyboard .hg-button[data-skbtn="Space"] {
  min-width: 200px;
  flex-grow: 6;
}

/* Row spacing */
.vkb .simple-keyboard .hg-row {
  margin-bottom: 0;
}

.vkb .simple-keyboard .hg-row:last-child {
  margin-bottom: 0;
}

/* First row (macros) - left aligned */
.kb-main-container .hg-row:first-child {
  justify-content: flex-start !important;
  margin-bottom: 8px;
  gap: 4px;
}

/* Second row (function keys) spacing */
.kb-main-container .hg-row:nth-child(2) {
  margin-bottom: 8px;
}

/* Backspace - wider */
.vkb .simple-keyboard .hg-button[data-skbtn="Backspace"] {
  flex-grow: 2;
  min-width: 80px;
}

/* Tab key */
.vkb .simple-keyboard .hg-button[data-skbtn="Tab"] {
  flex-grow: 1.5;
  min-width: 60px;
}

/* Backslash - adjust to match row width */
.vkb .simple-keyboard .hg-button[data-skbtn="Backslash"],
.vkb .simple-keyboard .hg-button[data-skbtn="(Backslash)"] {
  flex-grow: 1.5;
  min-width: 60px;
}

/* Caps Lock */
.vkb .simple-keyboard .hg-button[data-skbtn="CapsLock"] {
  flex-grow: 1.75;
  min-width: 70px;
}

/* Enter key */
.vkb .simple-keyboard .hg-button[data-skbtn="Enter"] {
  flex-grow: 2.25;
  min-width: 90px;
}

/* Left Shift - wider */
.vkb .simple-keyboard .hg-button[data-skbtn="ShiftLeft"] {
  flex-grow: 2.25;
  min-width: 90px;
}

/* Right Shift - wider */
.vkb .simple-keyboard .hg-button[data-skbtn="ShiftRight"] {
  flex-grow: 2.75;
  min-width: 110px;
}

/* Bottom row modifiers */
.vkb .simple-keyboard .hg-button[data-skbtn="ControlLeft"],
.vkb .simple-keyboard .hg-button[data-skbtn="ControlRight"] {
  flex-grow: 1.25;
  min-width: 55px;
}

.vkb .simple-keyboard .hg-button[data-skbtn="MetaLeft"],
.vkb .simple-keyboard .hg-button[data-skbtn="MetaRight"] {
  flex-grow: 1.25;
  min-width: 55px;
}

.vkb .simple-keyboard .hg-button[data-skbtn="AltLeft"] {
  flex-grow: 1.25;
  min-width: 55px;
}

.vkb .simple-keyboard .hg-button[data-skbtn="AltGr"] {
  flex-grow: 1.25;
  min-width: 55px;
}

.vkb .simple-keyboard .hg-button[data-skbtn="Menu"] {
  flex-grow: 1.25;
  min-width: 55px;
}

/* Control keyboard */
.kb-control-container .hg-button {
  min-width: 54px !important;
  justify-content: center;
}

/* Arrow buttons */
.kb-arrows-container .hg-button {
  min-width: 44px !important;
  width: 44px !important;
  justify-content: center;
}

.kb-arrows-container .hg-row {
  justify-content: center;
}

/* Dark mode - must be after simple-keyboard CSS import */
/* Use multiple selectors to ensure matching */
:root.dark .hg-theme-default .hg-button,
html.dark .hg-theme-default .hg-button,
.dark .hg-theme-default .hg-button {
  background: #374151 !important;
  color: #f9fafb !important;
  border-color: #4b5563 !important;
  border-bottom-color: #4b5563 !important;
  box-shadow: none !important;
}

:root.dark .hg-theme-default .hg-button:hover,
html.dark .hg-theme-default .hg-button:hover,
.dark .hg-theme-default .hg-button:hover {
  background: #4b5563 !important;
}

:root.dark .hg-theme-default .hg-button:active,
html.dark .hg-theme-default .hg-button:active,
.dark .hg-theme-default .hg-button:active,
:root.dark .hg-theme-default .hg-button.hg-activeButton,
html.dark .hg-theme-default .hg-button.hg-activeButton,
.dark .hg-theme-default .hg-button.hg-activeButton {
  background: #3b82f6 !important;
  color: white !important;
}

:root.dark .hg-theme-default .hg-button.down-key,
html.dark .hg-theme-default .hg-button.down-key,
.dark .hg-theme-default .hg-button.down-key {
  background: #3b82f6 !important;
  color: white !important;
  border-color: #2563eb !important;
  border-bottom-color: #2563eb !important;
}
</style>

<style scoped>
.vkb {
  z-index: 100;
  user-select: none;
  background: white;
  border: 1px solid #e5e7eb;
}

:global(.dark .vkb) {
  background: #1f2937;
  border-color: #374151;
}

.vkb--attached {
  position: relative;
  width: 100%;
  border-left: 0;
  border-right: 0;
  border-bottom: 0;
}

.vkb--floating {
  position: fixed;
  top: 0;
  left: 0;
  min-width: 1200px;
  max-width: 1600px;
  width: auto;
  border-radius: 8px;
  box-shadow: 0 25px 50px -12px rgb(0 0 0 / 0.25);
}

.vkb--dragging {
  cursor: move;
}

/* Header - compact */
.vkb-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px;
  border-bottom: 1px solid #e5e7eb;
  background: #f9fafb;
  min-height: 28px;
}

:global(.dark .vkb-header) {
  background: #111827;
  border-color: #374151;
}

.vkb--floating .vkb-header {
  cursor: move;
  border-radius: 8px 8px 0 0;
}

.vkb-title {
  font-size: 12px;
  font-weight: 500;
  color: #374151;
}

:global(.dark .vkb-title) {
  color: #d1d5db;
}

.vkb-btn {
  padding: 2px 8px;
  font-size: 11px;
  font-weight: 500;
  color: #374151;
  background: white;
  border: 1px solid #d1d5db;
  border-radius: 4px;
  cursor: pointer;
  line-height: 1.4;
}

.vkb-btn:hover {
  background: #f3f4f6;
}

:global(.dark .vkb-btn) {
  color: #d1d5db;
  background: #374151;
  border-color: #4b5563;
}

:global(.dark .vkb-btn:hover) {
  background: #4b5563;
}

/* Keyboard body */
.vkb-body {
  display: flex;
  flex-direction: row;
  padding: 8px;
  gap: 8px;
  background: #f3f4f6;
}

:global(.dark .vkb-body) {
  background: #111827;
}

.vkb--floating .vkb-body {
  border-radius: 0 0 8px 8px;
}

.kb-main-container {
  flex: 1;
}

.vkb-side {
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: flex-end;
}

.kb-control-container,
.kb-arrows-container {
  display: inline-block;
}

/* Responsive */
@media (max-width: 900px) {
  .vkb-body {
    flex-direction: column;
  }

  .vkb-side {
    flex-direction: row;
    justify-content: center;
    gap: 16px;
  }
}

/* Floating mode - slightly smaller keys but still readable */
.vkb--floating .vkb-body {
  padding: 8px;
}

.vkb--floating :deep(.simple-keyboard .hg-button) {
  height: 34px;
  font-size: 12px;
}

.vkb--floating :deep(.simple-keyboard .hg-button.combination-key) {
  height: 26px;
  font-size: 10px;
}

.vkb--floating :deep(.kb-control-container .hg-button) {
  min-width: 52px !important;
}

.vkb--floating :deep(.kb-arrows-container .hg-button) {
  min-width: 42px !important;
  width: 42px !important;
}

/* Animation */
.keyboard-fade-enter-active,
.keyboard-fade-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.keyboard-fade-enter-from,
.keyboard-fade-leave-to {
  opacity: 0;
}

.vkb--attached.keyboard-fade-enter-from,
.vkb--attached.keyboard-fade-leave-to {
  transform: translateY(20px);
}

.vkb--floating.keyboard-fade-enter-from,
.vkb--floating.keyboard-fade-leave-to {
  transform: scale(0.95);
}
</style>
