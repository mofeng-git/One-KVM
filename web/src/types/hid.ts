// HID (Human Interface Device) type definitions
// Shared between WebRTC DataChannel and WebSocket HID channels

/** Keyboard event for HID input */
export interface HidKeyboardEvent {
  type: 'keydown' | 'keyup'
  key: number
  modifiers?: {
    ctrl?: boolean
    shift?: boolean
    alt?: boolean
    meta?: boolean
  }
}

/** Mouse event for HID input */
export interface HidMouseEvent {
  type: 'move' | 'moveabs' | 'down' | 'up' | 'scroll'
  x?: number
  y?: number
  button?: number // 0=left, 1=middle, 2=right
  scroll?: number
}

/** Consumer control event for HID input (multimedia keys) */
export interface HidConsumerEvent {
  usage: number // Consumer Control Usage code (e.g., 0x00CD for Play/Pause)
}

// Binary message constants (must match datachannel.rs / ws_hid.rs)
export const MSG_KEYBOARD = 0x01
export const MSG_MOUSE = 0x02
export const MSG_CONSUMER = 0x03

// Keyboard event types
export const KB_EVENT_DOWN = 0x00
export const KB_EVENT_UP = 0x01

// Mouse event types
export const MS_EVENT_MOVE = 0x00
export const MS_EVENT_MOVE_ABS = 0x01
export const MS_EVENT_DOWN = 0x02
export const MS_EVENT_UP = 0x03
export const MS_EVENT_SCROLL = 0x04

// Response codes from server
export const RESP_OK = 0x00
export const RESP_ERR_HID_UNAVAILABLE = 0x01
export const RESP_ERR_INVALID_MESSAGE = 0x02

/** Encode keyboard event to binary format (4 bytes) */
export function encodeKeyboardEvent(event: HidKeyboardEvent): ArrayBuffer {
  const buffer = new ArrayBuffer(4)
  const view = new DataView(buffer)

  view.setUint8(0, MSG_KEYBOARD)
  view.setUint8(1, event.type === 'keydown' ? KB_EVENT_DOWN : KB_EVENT_UP)
  view.setUint8(2, event.key & 0xff)

  // Build modifiers bitmask
  let modifiers = 0
  if (event.modifiers?.ctrl) modifiers |= 0x01 // Left Ctrl
  if (event.modifiers?.shift) modifiers |= 0x02 // Left Shift
  if (event.modifiers?.alt) modifiers |= 0x04 // Left Alt
  if (event.modifiers?.meta) modifiers |= 0x08 // Left Meta
  view.setUint8(3, modifiers)

  return buffer
}

/** Encode mouse event to binary format (7 bytes) */
export function encodeMouseEvent(event: HidMouseEvent): ArrayBuffer {
  const buffer = new ArrayBuffer(7)
  const view = new DataView(buffer)

  view.setUint8(0, MSG_MOUSE)

  // Event type
  let eventType = MS_EVENT_MOVE
  switch (event.type) {
    case 'move':
      eventType = MS_EVENT_MOVE
      break
    case 'moveabs':
      eventType = MS_EVENT_MOVE_ABS
      break
    case 'down':
      eventType = MS_EVENT_DOWN
      break
    case 'up':
      eventType = MS_EVENT_UP
      break
    case 'scroll':
      eventType = MS_EVENT_SCROLL
      break
  }
  view.setUint8(1, eventType)

  // X coordinate (i16 LE)
  view.setInt16(2, event.x ?? 0, true)

  // Y coordinate (i16 LE)
  view.setInt16(4, event.y ?? 0, true)

  // Button or scroll delta
  if (event.type === 'down' || event.type === 'up') {
    view.setUint8(6, event.button ?? 0)
  } else if (event.type === 'scroll') {
    view.setInt8(6, event.scroll ?? 0)
  } else {
    view.setUint8(6, 0)
  }

  return buffer
}

/** Encode consumer control event to binary format (3 bytes) */
export function encodeConsumerEvent(event: HidConsumerEvent): ArrayBuffer {
  const buffer = new ArrayBuffer(3)
  const view = new DataView(buffer)

  view.setUint8(0, MSG_CONSUMER)
  // Usage code as u16 LE
  view.setUint16(1, event.usage, true)

  return buffer
}
