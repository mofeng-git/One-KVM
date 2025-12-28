// Keyboard layout definitions for virtual keyboard

export interface KeyboardLayout {
  id: string
  name: string
  // Key display labels
  keyLabels: Record<string, string>
  // Shift variant labels (key in parentheses)
  shiftLabels: Record<string, string>
  // Virtual keyboard layout rows
  layout: {
    main: {
      macros: string[]
      functionRow: string[]
      default: string[][]
      shift: string[][]
    }
    control: string[][]
    arrows: string[][]
  }
}

// English US Layout
export const enUSLayout: KeyboardLayout = {
  id: 'en-US',
  name: 'English (US)',
  keyLabels: {
    // Macros
    CtrlAltDelete: 'Ctrl+Alt+Del',
    AltMetaEscape: 'Alt+Meta+Esc',
    CtrlAltBackspace: 'Ctrl+Alt+Back',

    // Modifiers
    ControlLeft: 'Ctrl',
    ControlRight: 'Ctrl',
    ShiftLeft: 'Shift',
    ShiftRight: 'Shift',
    AltLeft: 'Alt',
    AltRight: 'Alt',
    AltGr: 'AltGr',
    MetaLeft: 'Meta',
    MetaRight: 'Meta',

    // Special keys
    Escape: 'Esc',
    Backspace: 'Back',
    Tab: 'Tab',
    CapsLock: 'Caps',
    Enter: 'Enter',
    Space: ' ',
    Menu: 'Menu',

    // Navigation
    Insert: 'Ins',
    Delete: 'Del',
    Home: 'Home',
    End: 'End',
    PageUp: 'PgUp',
    PageDown: 'PgDn',

    // Arrows
    ArrowUp: '\u2191',
    ArrowDown: '\u2193',
    ArrowLeft: '\u2190',
    ArrowRight: '\u2192',

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

    // Numbers
    Digit1: '1', Digit2: '2', Digit3: '3', Digit4: '4', Digit5: '5',
    Digit6: '6', Digit7: '7', Digit8: '8', Digit9: '9', Digit0: '0',

    // Symbols
    Minus: '-',
    Equal: '=',
    BracketLeft: '[',
    BracketRight: ']',
    Backslash: '\\',
    Semicolon: ';',
    Quote: "'",
    Backquote: '`',
    Comma: ',',
    Period: '.',
    Slash: '/',
  },
  shiftLabels: {
    // Capital letters
    KeyA: 'A', KeyB: 'B', KeyC: 'C', KeyD: 'D', KeyE: 'E',
    KeyF: 'F', KeyG: 'G', KeyH: 'H', KeyI: 'I', KeyJ: 'J',
    KeyK: 'K', KeyL: 'L', KeyM: 'M', KeyN: 'N', KeyO: 'O',
    KeyP: 'P', KeyQ: 'Q', KeyR: 'R', KeyS: 'S', KeyT: 'T',
    KeyU: 'U', KeyV: 'V', KeyW: 'W', KeyX: 'X', KeyY: 'Y',
    KeyZ: 'Z',

    // Shifted numbers
    Digit1: '!', Digit2: '@', Digit3: '#', Digit4: '$', Digit5: '%',
    Digit6: '^', Digit7: '&', Digit8: '*', Digit9: '(', Digit0: ')',

    // Shifted symbols
    Minus: '_',
    Equal: '+',
    BracketLeft: '{',
    BracketRight: '}',
    Backslash: '|',
    Semicolon: ':',
    Quote: '"',
    Backquote: '~',
    Comma: '<',
    Period: '>',
    Slash: '?',
  },
  layout: {
    main: {
      macros: ['CtrlAltDelete', 'AltMetaEscape', 'CtrlAltBackspace'],
      functionRow: ['Escape', 'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12'],
      default: [
        ['Backquote', 'Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5', 'Digit6', 'Digit7', 'Digit8', 'Digit9', 'Digit0', 'Minus', 'Equal', 'Backspace'],
        ['Tab', 'KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyT', 'KeyY', 'KeyU', 'KeyI', 'KeyO', 'KeyP', 'BracketLeft', 'BracketRight', 'Backslash'],
        ['CapsLock', 'KeyA', 'KeyS', 'KeyD', 'KeyF', 'KeyG', 'KeyH', 'KeyJ', 'KeyK', 'KeyL', 'Semicolon', 'Quote', 'Enter'],
        ['ShiftLeft', 'KeyZ', 'KeyX', 'KeyC', 'KeyV', 'KeyB', 'KeyN', 'KeyM', 'Comma', 'Period', 'Slash', 'ShiftRight'],
        ['ControlLeft', 'MetaLeft', 'AltLeft', 'Space', 'AltRight', 'MetaRight', 'Menu', 'ControlRight'],
      ],
      shift: [
        ['Backquote', 'Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5', 'Digit6', 'Digit7', 'Digit8', 'Digit9', 'Digit0', 'Minus', 'Equal', 'Backspace'],
        ['Tab', 'KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyT', 'KeyY', 'KeyU', 'KeyI', 'KeyO', 'KeyP', 'BracketLeft', 'BracketRight', 'Backslash'],
        ['CapsLock', 'KeyA', 'KeyS', 'KeyD', 'KeyF', 'KeyG', 'KeyH', 'KeyJ', 'KeyK', 'KeyL', 'Semicolon', 'Quote', 'Enter'],
        ['ShiftLeft', 'KeyZ', 'KeyX', 'KeyC', 'KeyV', 'KeyB', 'KeyN', 'KeyM', 'Comma', 'Period', 'Slash', 'ShiftRight'],
        ['ControlLeft', 'MetaLeft', 'AltLeft', 'Space', 'AltRight', 'MetaRight', 'Menu', 'ControlRight'],
      ],
    },
    control: [
      ['PrintScreen', 'ScrollLock', 'Pause'],
      ['Insert', 'Home', 'PageUp'],
      ['Delete', 'End', 'PageDown'],
    ],
    arrows: [
      ['ArrowUp'],
      ['ArrowLeft', 'ArrowDown', 'ArrowRight'],
    ],
  },
}

// All available layouts
export const keyboardLayouts: Record<string, KeyboardLayout> = {
  'en-US': enUSLayout,
}

// Get layout by ID or return default
export function getKeyboardLayout(id: string): KeyboardLayout {
  return keyboardLayouts[id] || enUSLayout
}

// Get key label for display
export function getKeyLabel(layout: KeyboardLayout, keyName: string, isShift: boolean): string {
  if (isShift && layout.shiftLabels[keyName]) {
    return layout.shiftLabels[keyName]
  }
  return layout.keyLabels[keyName] || keyName
}
