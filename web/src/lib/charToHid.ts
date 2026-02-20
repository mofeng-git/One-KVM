// Character to HID usage mapping for text paste functionality.
// The table follows US QWERTY layout semantics.

import { keys } from '@/lib/keyboardMappings'

export interface CharKeyMapping {
  hidCode: number // USB HID usage code
  shift: boolean // Whether Shift modifier is needed
}

const charToKeyMap: Record<string, CharKeyMapping> = {
  // Lowercase letters
  a: { hidCode: keys.KeyA, shift: false },
  b: { hidCode: keys.KeyB, shift: false },
  c: { hidCode: keys.KeyC, shift: false },
  d: { hidCode: keys.KeyD, shift: false },
  e: { hidCode: keys.KeyE, shift: false },
  f: { hidCode: keys.KeyF, shift: false },
  g: { hidCode: keys.KeyG, shift: false },
  h: { hidCode: keys.KeyH, shift: false },
  i: { hidCode: keys.KeyI, shift: false },
  j: { hidCode: keys.KeyJ, shift: false },
  k: { hidCode: keys.KeyK, shift: false },
  l: { hidCode: keys.KeyL, shift: false },
  m: { hidCode: keys.KeyM, shift: false },
  n: { hidCode: keys.KeyN, shift: false },
  o: { hidCode: keys.KeyO, shift: false },
  p: { hidCode: keys.KeyP, shift: false },
  q: { hidCode: keys.KeyQ, shift: false },
  r: { hidCode: keys.KeyR, shift: false },
  s: { hidCode: keys.KeyS, shift: false },
  t: { hidCode: keys.KeyT, shift: false },
  u: { hidCode: keys.KeyU, shift: false },
  v: { hidCode: keys.KeyV, shift: false },
  w: { hidCode: keys.KeyW, shift: false },
  x: { hidCode: keys.KeyX, shift: false },
  y: { hidCode: keys.KeyY, shift: false },
  z: { hidCode: keys.KeyZ, shift: false },

  // Uppercase letters
  A: { hidCode: keys.KeyA, shift: true },
  B: { hidCode: keys.KeyB, shift: true },
  C: { hidCode: keys.KeyC, shift: true },
  D: { hidCode: keys.KeyD, shift: true },
  E: { hidCode: keys.KeyE, shift: true },
  F: { hidCode: keys.KeyF, shift: true },
  G: { hidCode: keys.KeyG, shift: true },
  H: { hidCode: keys.KeyH, shift: true },
  I: { hidCode: keys.KeyI, shift: true },
  J: { hidCode: keys.KeyJ, shift: true },
  K: { hidCode: keys.KeyK, shift: true },
  L: { hidCode: keys.KeyL, shift: true },
  M: { hidCode: keys.KeyM, shift: true },
  N: { hidCode: keys.KeyN, shift: true },
  O: { hidCode: keys.KeyO, shift: true },
  P: { hidCode: keys.KeyP, shift: true },
  Q: { hidCode: keys.KeyQ, shift: true },
  R: { hidCode: keys.KeyR, shift: true },
  S: { hidCode: keys.KeyS, shift: true },
  T: { hidCode: keys.KeyT, shift: true },
  U: { hidCode: keys.KeyU, shift: true },
  V: { hidCode: keys.KeyV, shift: true },
  W: { hidCode: keys.KeyW, shift: true },
  X: { hidCode: keys.KeyX, shift: true },
  Y: { hidCode: keys.KeyY, shift: true },
  Z: { hidCode: keys.KeyZ, shift: true },

  // Number row
  '0': { hidCode: keys.Digit0, shift: false },
  '1': { hidCode: keys.Digit1, shift: false },
  '2': { hidCode: keys.Digit2, shift: false },
  '3': { hidCode: keys.Digit3, shift: false },
  '4': { hidCode: keys.Digit4, shift: false },
  '5': { hidCode: keys.Digit5, shift: false },
  '6': { hidCode: keys.Digit6, shift: false },
  '7': { hidCode: keys.Digit7, shift: false },
  '8': { hidCode: keys.Digit8, shift: false },
  '9': { hidCode: keys.Digit9, shift: false },

  // Shifted number row symbols
  ')': { hidCode: keys.Digit0, shift: true },
  '!': { hidCode: keys.Digit1, shift: true },
  '@': { hidCode: keys.Digit2, shift: true },
  '#': { hidCode: keys.Digit3, shift: true },
  '$': { hidCode: keys.Digit4, shift: true },
  '%': { hidCode: keys.Digit5, shift: true },
  '^': { hidCode: keys.Digit6, shift: true },
  '&': { hidCode: keys.Digit7, shift: true },
  '*': { hidCode: keys.Digit8, shift: true },
  '(': { hidCode: keys.Digit9, shift: true },

  // Punctuation and symbols
  '-': { hidCode: keys.Minus, shift: false },
  '=': { hidCode: keys.Equal, shift: false },
  '[': { hidCode: keys.BracketLeft, shift: false },
  ']': { hidCode: keys.BracketRight, shift: false },
  '\\': { hidCode: keys.Backslash, shift: false },
  ';': { hidCode: keys.Semicolon, shift: false },
  "'": { hidCode: keys.Quote, shift: false },
  '`': { hidCode: keys.Backquote, shift: false },
  ',': { hidCode: keys.Comma, shift: false },
  '.': { hidCode: keys.Period, shift: false },
  '/': { hidCode: keys.Slash, shift: false },

  // Shifted punctuation and symbols
  _: { hidCode: keys.Minus, shift: true },
  '+': { hidCode: keys.Equal, shift: true },
  '{': { hidCode: keys.BracketLeft, shift: true },
  '}': { hidCode: keys.BracketRight, shift: true },
  '|': { hidCode: keys.Backslash, shift: true },
  ':': { hidCode: keys.Semicolon, shift: true },
  '"': { hidCode: keys.Quote, shift: true },
  '~': { hidCode: keys.Backquote, shift: true },
  '<': { hidCode: keys.Comma, shift: true },
  '>': { hidCode: keys.Period, shift: true },
  '?': { hidCode: keys.Slash, shift: true },

  // Whitespace and control
  ' ': { hidCode: keys.Space, shift: false },
  '\t': { hidCode: keys.Tab, shift: false },
  '\n': { hidCode: keys.Enter, shift: false },
  '\r': { hidCode: keys.Enter, shift: false },
}

/**
 * Get HID usage code and modifier state for a character
 * @param char - Single character to convert
 * @returns CharKeyMapping or null if character is not mappable
 */
export function charToKey(char: string): CharKeyMapping | null {
  if (char.length !== 1) return null
  return charToKeyMap[char] ?? null
}

/**
 * Check if a character can be typed via HID
 * @param char - Single character to check
 */
export function isTypableChar(char: string): boolean {
  return charToKey(char) !== null
}

/**
 * Get statistics about text typeability
 * @param text - Text to analyze
 * @returns Object with total, typable, and untypable character counts
 */
export function analyzeText(text: string): {
  total: number
  typable: number
  untypable: number
  untypableChars: string[]
} {
  const untypableChars: string[] = []
  let typable = 0
  let untypable = 0

  for (const char of text) {
    if (isTypableChar(char)) {
      typable++
    } else {
      untypable++
      if (!untypableChars.includes(char)) {
        untypableChars.push(char)
      }
    }
  }

  return {
    total: text.length,
    typable,
    untypable,
    untypableChars,
  }
}
