// Key codes and modifiers correspond to definitions in the
// [Linux USB HID gadget driver](https://www.kernel.org/doc/Documentation/usb/gadget_hid.txt)
// [Universal Serial Bus HID Usage Tables: Section 10](https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf)

export const keys = {
  // Letters
  KeyA: 0x04,
  KeyB: 0x05,
  KeyC: 0x06,
  KeyD: 0x07,
  KeyE: 0x08,
  KeyF: 0x09,
  KeyG: 0x0a,
  KeyH: 0x0b,
  KeyI: 0x0c,
  KeyJ: 0x0d,
  KeyK: 0x0e,
  KeyL: 0x0f,
  KeyM: 0x10,
  KeyN: 0x11,
  KeyO: 0x12,
  KeyP: 0x13,
  KeyQ: 0x14,
  KeyR: 0x15,
  KeyS: 0x16,
  KeyT: 0x17,
  KeyU: 0x18,
  KeyV: 0x19,
  KeyW: 0x1a,
  KeyX: 0x1b,
  KeyY: 0x1c,
  KeyZ: 0x1d,

  // Numbers
  Digit1: 0x1e,
  Digit2: 0x1f,
  Digit3: 0x20,
  Digit4: 0x21,
  Digit5: 0x22,
  Digit6: 0x23,
  Digit7: 0x24,
  Digit8: 0x25,
  Digit9: 0x26,
  Digit0: 0x27,

  // Control keys
  Enter: 0x28,
  Escape: 0x29,
  Backspace: 0x2a,
  Tab: 0x2b,
  Space: 0x2c,

  // Symbols
  Minus: 0x2d,
  Equal: 0x2e,
  BracketLeft: 0x2f,
  BracketRight: 0x30,
  Backslash: 0x31,
  Semicolon: 0x33,
  Quote: 0x34,
  Backquote: 0x35,
  Comma: 0x36,
  Period: 0x37,
  Slash: 0x38,

  // Lock keys
  CapsLock: 0x39,

  // Function keys
  F1: 0x3a,
  F2: 0x3b,
  F3: 0x3c,
  F4: 0x3d,
  F5: 0x3e,
  F6: 0x3f,
  F7: 0x40,
  F8: 0x41,
  F9: 0x42,
  F10: 0x43,
  F11: 0x44,
  F12: 0x45,

  // Control cluster
  PrintScreen: 0x46,
  ScrollLock: 0x47,
  Pause: 0x48,
  Insert: 0x49,
  Home: 0x4a,
  PageUp: 0x4b,
  Delete: 0x4c,
  End: 0x4d,
  PageDown: 0x4e,

  // Arrow keys
  ArrowRight: 0x4f,
  ArrowLeft: 0x50,
  ArrowDown: 0x51,
  ArrowUp: 0x52,

  // Numpad
  NumLock: 0x53,
  NumpadDivide: 0x54,
  NumpadMultiply: 0x55,
  NumpadSubtract: 0x56,
  NumpadAdd: 0x57,
  NumpadEnter: 0x58,
  Numpad1: 0x59,
  Numpad2: 0x5a,
  Numpad3: 0x5b,
  Numpad4: 0x5c,
  Numpad5: 0x5d,
  Numpad6: 0x5e,
  Numpad7: 0x5f,
  Numpad8: 0x60,
  Numpad9: 0x61,
  Numpad0: 0x62,
  NumpadDecimal: 0x63,

  // Non-US keys
  IntlBackslash: 0x64,
  ContextMenu: 0x65,
  Menu: 0x65,
  Application: 0x65,

  // Extended function keys
  F13: 0x68,
  F14: 0x69,
  F15: 0x6a,
  F16: 0x6b,
  F17: 0x6c,
  F18: 0x6d,
  F19: 0x6e,
  F20: 0x6f,
  F21: 0x70,
  F22: 0x71,
  F23: 0x72,
  F24: 0x73,

  // Modifiers (these are special - HID codes 0xE0-0xE7)
  ControlLeft: 0xe0,
  ShiftLeft: 0xe1,
  AltLeft: 0xe2,
  MetaLeft: 0xe3,
  ControlRight: 0xe4,
  ShiftRight: 0xe5,
  AltRight: 0xe6,
  AltGr: 0xe6,
  MetaRight: 0xe7,
} as const

export type KeyName = keyof typeof keys

// Consumer Control Usage codes (for multimedia keys)
// These are sent via a separate Consumer Control HID report
export const consumerKeys = {
  PlayPause: 0x00cd,
  Stop: 0x00b7,
  NextTrack: 0x00b5,
  PrevTrack: 0x00b6,
  Mute: 0x00e2,
  VolumeUp: 0x00e9,
  VolumeDown: 0x00ea,
} as const

export type ConsumerKeyName = keyof typeof consumerKeys

// Modifier bitmasks for HID report byte 0
export const modifiers = {
  ControlLeft: 0x01,
  ShiftLeft: 0x02,
  AltLeft: 0x04,
  MetaLeft: 0x08,
  ControlRight: 0x10,
  ShiftRight: 0x20,
  AltRight: 0x40,
  AltGr: 0x40,
  MetaRight: 0x80,
} as const

export type ModifierName = keyof typeof modifiers

// Map HID key codes to modifier bitmasks
export const hidKeyToModifierMask: Record<number, number> = {
  0xe0: 0x01, // ControlLeft
  0xe1: 0x02, // ShiftLeft
  0xe2: 0x04, // AltLeft
  0xe3: 0x08, // MetaLeft
  0xe4: 0x10, // ControlRight
  0xe5: 0x20, // ShiftRight
  0xe6: 0x40, // AltRight
  0xe7: 0x80, // MetaRight
}

// Keys that latch (toggle state) instead of being held
export const latchingKeys = ['CapsLock', 'ScrollLock', 'NumLock'] as const

// Modifier key names
export const modifierKeyNames = [
  'ControlLeft',
  'ControlRight',
  'ShiftLeft',
  'ShiftRight',
  'AltLeft',
  'AltRight',
  'AltGr',
  'MetaLeft',
  'MetaRight',
] as const

// Check if a key is a modifier
export function isModifierKey(keyName: string): keyName is ModifierName {
  return keyName in modifiers
}

// Get modifier bitmask for a key name
export function getModifierMask(keyName: string): number {
  if (keyName in modifiers) {
    return modifiers[keyName as ModifierName]
  }
  return 0
}

// Decode modifier byte into individual states
export function decodeModifiers(modifier: number) {
  return {
    isShiftActive: (modifier & 0x22) !== 0, // ShiftLeft | ShiftRight
    isControlActive: (modifier & 0x11) !== 0, // ControlLeft | ControlRight
    isAltActive: (modifier & 0x44) !== 0, // AltLeft | AltRight
    isMetaActive: (modifier & 0x88) !== 0, // MetaLeft | MetaRight
  }
}
