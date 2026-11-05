import { CanonicalKey as Key } from '@/types/generated'
import { updateModifierMaskForKey } from '@/lib/keyboardMappings'

export interface KeyboardShortcut {
  id: string
  label: string
  description: string
  keys: Key[]
  destructive?: boolean
}

function combination(id: string, label: string, keys: Key[]): KeyboardShortcut {
  return { id, label, description: id, keys }
}

export const keyboardShortcuts: KeyboardShortcut[] = [
  combination('ctrl-alt-delete', 'Ctrl+Alt+Del', [Key.ControlLeft, Key.AltLeft, Key.Delete]),
  combination('task-manager', 'Ctrl+Shift+Esc', [Key.ControlLeft, Key.ShiftLeft, Key.Escape]),
  combination('switch-window', 'Alt+Tab', [Key.AltLeft, Key.Tab]),
  combination('close-window', 'Alt+F4', [Key.AltLeft, Key.F4]),
  combination('lock-windows', 'Win+L', [Key.MetaLeft, Key.KeyL]),
  combination('run', 'Win+R', [Key.MetaLeft, Key.KeyR]),
  combination('tty1', 'Ctrl+Alt+F1', [Key.ControlLeft, Key.AltLeft, Key.F1]),
  combination('tty2', 'Ctrl+Alt+F2', [Key.ControlLeft, Key.AltLeft, Key.F2]),
  combination('restart-x', 'Ctrl+Alt+Bksp', [Key.ControlLeft, Key.AltLeft, Key.Backspace]),
  combination('force-quit', 'Cmd+Opt+Esc', [Key.MetaLeft, Key.AltLeft, Key.Escape]),
]

function sysrq(letter: string, key: Key, destructive = false): KeyboardShortcut {
  return {
    id: `sysrq-${letter.toLowerCase()}`, label: letter,
    description: `sysrq-${letter.toLowerCase()}`, destructive,
    keys: [Key.AltLeft, Key.PrintScreen, key],
  }
}

export const sysrqDiagnostics = [
  sysrq('H', Key.KeyH), sysrq('F', Key.KeyF, true),
  sysrq('M', Key.KeyM), sysrq('T', Key.KeyT),
]
export const sysrqRecovery = [
  sysrq('R', Key.KeyR), sysrq('E', Key.KeyE, true), sysrq('I', Key.KeyI, true),
  sysrq('S', Key.KeyS), sysrq('U', Key.KeyU), sysrq('B', Key.KeyB, true),
]

export type ShortcutSender = (type: 'down' | 'up', key: Key, modifiers: number) => Promise<void>

function checkCancelled(signal?: AbortSignal) {
  if (signal?.aborted) throw signal.reason ?? new DOMException('Cancelled', 'AbortError')
}

function delay(ms: number, signal?: AbortSignal): Promise<void> {
  checkCancelled(signal)
  return new Promise((resolve, reject) => {
    const abort = () => {
      clearTimeout(timer)
      reject(signal?.reason ?? new DOMException('Cancelled', 'AbortError'))
    }
    const timer = setTimeout(() => {
      signal?.removeEventListener('abort', abort)
      resolve()
    }, ms)
    signal?.addEventListener('abort', abort, { once: true })
  })
}

/** Send on one ordered transport; always attempt to release every attempted key. */
export async function executeKeyboardShortcut(
  shortcut: KeyboardShortcut,
  send: ShortcutSender,
  options: {
    signal?: AbortSignal
    wait?: typeof delay
  } = {},
) {
  const wait = options.wait ?? delay
  checkCancelled(options.signal)
  const held: Key[] = []
  let mask = 0
  let failure: unknown
  try {
    for (const key of shortcut.keys) {
      checkCancelled(options.signal)
      // A failed send may still have reached the device, so track it first.
      held.push(key)
      mask = updateModifierMaskForKey(mask, key, true)
      await send('down', key, mask)
      await wait(40, options.signal)
    }
    await wait(100, options.signal)
  } catch (error) {
    failure = error
  } finally {
    // Cleanup must not inherit cancellation, and one failed release must not
    // prevent the remaining modifiers from being released.
    for (const key of held.reverse()) {
      mask = updateModifierMaskForKey(mask, key, false)
      try {
        await send('up', key, mask)
        await wait(20)
      } catch (error) {
        failure ??= error
      }
    }
  }
  if (failure) throw failure
  checkCancelled(options.signal)
}
