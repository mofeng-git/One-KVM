// HID input composable - manages keyboard and mouse input
// Extracted from ConsoleView.vue for better separation of concerns

import { ref, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { hidApi } from '@/api'
import { keyboardEventToHidCode, updateModifierMaskForHidKey } from '@/lib/keyboardMappings'

export interface HidInputState {
  mouseMode: Ref<'absolute' | 'relative'>
  pressedKeys: Ref<string[]>
  keyboardLed: Ref<{ capsLock: boolean; numLock: boolean; scrollLock: boolean }>
  mousePosition: Ref<{ x: number; y: number }>
  isPointerLocked: Ref<boolean>
  cursorVisible: Ref<boolean>
}

export interface UseHidInputOptions {
  videoContainerRef: Ref<HTMLDivElement | null>
  getVideoElement: () => HTMLElement | null
  isFullscreen: Ref<boolean>
}

export function useHidInput(options: UseHidInputOptions) {
  const { t } = useI18n()

  // State
  const mouseMode = ref<'absolute' | 'relative'>('absolute')
  const pressedKeys = ref<string[]>([])
  const keyboardLed = ref({
    capsLock: false,
    numLock: false,
    scrollLock: false,
  })
  const activeModifierMask = ref(0)
  const mousePosition = ref({ x: 0, y: 0 })
  const lastMousePosition = ref({ x: 0, y: 0 })
  const isPointerLocked = ref(false)
  const cursorVisible = ref(localStorage.getItem('hidShowCursor') !== 'false')
  const pressedMouseButton = ref<'left' | 'right' | 'middle' | null>(null)

  // Error handling - silently handle all HID errors
  function handleHidError(_error: unknown, _operation: string) {
    // All HID errors are silently ignored
  }

  // Check if a key should be blocked
  function shouldBlockKey(e: KeyboardEvent): boolean {
    if (options.isFullscreen.value) return true

    const key = e.key.toUpperCase()
    if (e.ctrlKey && ['W', 'T', 'N'].includes(key)) return false
    if (key === 'F11') return false
    if (e.altKey && key === 'TAB') return false

    return true
  }

  // Keyboard handlers
  function handleKeyDown(e: KeyboardEvent) {
    const container = options.videoContainerRef.value
    if (!container) return

    if (!options.isFullscreen.value && !container.contains(document.activeElement)) return

    if (shouldBlockKey(e)) {
      e.preventDefault()
      e.stopPropagation()
    }

    if (!options.isFullscreen.value && (e.metaKey || e.key === 'Meta')) {
      toast.info(t('console.metaKeyHint'), {
        description: t('console.metaKeyHintDesc'),
        duration: 3000,
      })
    }

    const keyName = e.key === ' ' ? 'Space' : e.key
    if (!pressedKeys.value.includes(keyName)) {
      pressedKeys.value = [...pressedKeys.value, keyName]
    }

    keyboardLed.value.capsLock = e.getModifierState('CapsLock')
    keyboardLed.value.numLock = e.getModifierState('NumLock')
    keyboardLed.value.scrollLock = e.getModifierState('ScrollLock')

    const hidKey = keyboardEventToHidCode(e.code, e.key)
    if (hidKey === undefined) {
      return
    }

    const modifierMask = updateModifierMaskForHidKey(activeModifierMask.value, hidKey, true)
    activeModifierMask.value = modifierMask
    hidApi.keyboard('down', hidKey, modifierMask).catch(err => handleHidError(err, 'keyboard down'))
  }

  function handleKeyUp(e: KeyboardEvent) {
    const container = options.videoContainerRef.value
    if (!container) return

    if (!options.isFullscreen.value && !container.contains(document.activeElement)) return

    if (shouldBlockKey(e)) {
      e.preventDefault()
      e.stopPropagation()
    }

    const keyName = e.key === ' ' ? 'Space' : e.key
    pressedKeys.value = pressedKeys.value.filter(k => k !== keyName)

    const hidKey = keyboardEventToHidCode(e.code, e.key)
    if (hidKey === undefined) {
      return
    }

    const modifierMask = updateModifierMaskForHidKey(activeModifierMask.value, hidKey, false)
    activeModifierMask.value = modifierMask
    hidApi.keyboard('up', hidKey, modifierMask).catch(err => handleHidError(err, 'keyboard up'))
  }

  // Mouse handlers
  function handleMouseMove(e: MouseEvent) {
    const videoElement = options.getVideoElement()
    if (!videoElement) return

    if (mouseMode.value === 'absolute') {
      const rect = videoElement.getBoundingClientRect()
      const x = Math.round((e.clientX - rect.left) / rect.width * 32767)
      const y = Math.round((e.clientY - rect.top) / rect.height * 32767)

      mousePosition.value = { x, y }
      hidApi.mouse({ type: 'move_abs', x, y }).catch(err => handleHidError(err, 'mouse move'))
    } else {
      if (isPointerLocked.value) {
        const dx = e.movementX
        const dy = e.movementY

        if (dx !== 0 || dy !== 0) {
          const clampedDx = Math.max(-127, Math.min(127, dx))
          const clampedDy = Math.max(-127, Math.min(127, dy))
          hidApi.mouse({ type: 'move', x: clampedDx, y: clampedDy }).catch(err => handleHidError(err, 'mouse move'))
        }

        mousePosition.value = {
          x: mousePosition.value.x + dx,
          y: mousePosition.value.y + dy,
        }
      }
    }
  }

  function handleMouseDown(e: MouseEvent) {
    e.preventDefault()

    const container = options.videoContainerRef.value
    if (container && document.activeElement !== container) {
      if (typeof container.focus === 'function') {
        container.focus()
      }
    }

    if (mouseMode.value === 'relative' && !isPointerLocked.value) {
      requestPointerLock()
      return
    }

    const button = e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle'
    pressedMouseButton.value = button
    hidApi.mouse({ type: 'down', button }).catch(err => handleHidError(err, 'mouse down'))
  }

  function handleMouseUp(e: MouseEvent) {
    e.preventDefault()
    handleMouseUpInternal(e.button)
  }

  function handleWindowMouseUp(e: MouseEvent) {
    if (pressedMouseButton.value !== null) {
      handleMouseUpInternal(e.button)
    }
  }

  function handleMouseUpInternal(rawButton: number) {
    if (mouseMode.value === 'relative' && !isPointerLocked.value) {
      pressedMouseButton.value = null
      return
    }

    const button = rawButton === 0 ? 'left' : rawButton === 2 ? 'right' : 'middle'

    if (pressedMouseButton.value !== button) return

    pressedMouseButton.value = null
    hidApi.mouse({ type: 'up', button }).catch(err => handleHidError(err, 'mouse up'))
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault()
    const scroll = e.deltaY > 0 ? -1 : 1
    hidApi.mouse({ type: 'scroll', scroll }).catch(err => handleHidError(err, 'mouse scroll'))
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault()
  }

  // Pointer lock
  function requestPointerLock() {
    const container = options.videoContainerRef.value
    if (!container) return

    container.requestPointerLock().catch((err: Error) => {
      toast.error(t('console.pointerLockFailed'), {
        description: err.message,
      })
    })
  }

  function exitPointerLock() {
    if (document.pointerLockElement) {
      document.exitPointerLock()
    }
  }

  function handlePointerLockChange() {
    const container = options.videoContainerRef.value
    isPointerLocked.value = document.pointerLockElement === container

    if (isPointerLocked.value) {
      mousePosition.value = { x: 0, y: 0 }
      toast.info(t('console.pointerLocked'), {
        description: t('console.pointerLockedDesc'),
        duration: 3000,
      })
    }
  }

  function handlePointerLockError() {
    isPointerLocked.value = false
  }

  function handleBlur() {
    pressedKeys.value = []
    activeModifierMask.value = 0
    if (pressedMouseButton.value !== null) {
      const button = pressedMouseButton.value
      pressedMouseButton.value = null
      hidApi.mouse({ type: 'up', button }).catch(err => handleHidError(err, 'mouse up (blur)'))
    }
  }

  // Mode toggle
  function toggleMouseMode() {
    if (mouseMode.value === 'relative' && isPointerLocked.value) {
      exitPointerLock()
    }

    mouseMode.value = mouseMode.value === 'absolute' ? 'relative' : 'absolute'
    lastMousePosition.value = { x: 0, y: 0 }
    mousePosition.value = { x: 0, y: 0 }

    if (mouseMode.value === 'relative') {
      toast.info(t('console.relativeModeHint'), {
        description: t('console.relativeModeHintDesc'),
        duration: 5000,
      })
    }
  }

  // Virtual keyboard handlers
  function handleVirtualKeyDown(key: string) {
    if (!pressedKeys.value.includes(key)) {
      pressedKeys.value = [...pressedKeys.value, key]
    }
  }

  function handleVirtualKeyUp(key: string) {
    pressedKeys.value = pressedKeys.value.filter(k => k !== key)
  }

  // Cursor visibility
  function handleCursorVisibilityChange(e: Event) {
    const customEvent = e as CustomEvent<{ visible: boolean }>
    cursorVisible.value = customEvent.detail.visible
  }

  // Setup event listeners
  function setupEventListeners() {
    document.addEventListener('pointerlockchange', handlePointerLockChange)
    document.addEventListener('pointerlockerror', handlePointerLockError)
    window.addEventListener('blur', handleBlur)
    window.addEventListener('mouseup', handleWindowMouseUp)
    window.addEventListener('cursor-visibility-change', handleCursorVisibilityChange)
  }

  function cleanupEventListeners() {
    document.removeEventListener('pointerlockchange', handlePointerLockChange)
    document.removeEventListener('pointerlockerror', handlePointerLockError)
    window.removeEventListener('blur', handleBlur)
    window.removeEventListener('mouseup', handleWindowMouseUp)
    window.removeEventListener('cursor-visibility-change', handleCursorVisibilityChange)
  }

  return {
    // State
    mouseMode,
    pressedKeys,
    keyboardLed,
    mousePosition,
    isPointerLocked,
    cursorVisible,

    // Keyboard handlers
    handleKeyDown,
    handleKeyUp,

    // Mouse handlers
    handleMouseMove,
    handleMouseDown,
    handleMouseUp,
    handleWheel,
    handleContextMenu,

    // Pointer lock
    requestPointerLock,
    exitPointerLock,

    // Mode toggle
    toggleMouseMode,

    // Virtual keyboard
    handleVirtualKeyDown,
    handleVirtualKeyUp,

    // Cursor visibility
    handleCursorVisibilityChange,

    // Lifecycle
    setupEventListeners,
    cleanupEventListeners,
  }
}
