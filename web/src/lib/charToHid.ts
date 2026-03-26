// Character to HID usage mapping for text paste functionality.
// The table follows US QWERTY layout semantics.

import { type CanonicalKey } from '@/types/generated'
import { keys } from '@/lib/keyboardMappings'

export interface CharKeyMapping {
  key: CanonicalKey
  shift: boolean // Whether Shift modifier is needed
}

const charToKeyMap: Record<string, CharKeyMapping> = {
  // Lowercase letters
  a: { key: keys.KeyA, shift: false },
  b: { key: keys.KeyB, shift: false },
  c: { key: keys.KeyC, shift: false },
  d: { key: keys.KeyD, shift: false },
  e: { key: keys.KeyE, shift: false },
  f: { key: keys.KeyF, shift: false },
  g: { key: keys.KeyG, shift: false },
  h: { key: keys.KeyH, shift: false },
  i: { key: keys.KeyI, shift: false },
  j: { key: keys.KeyJ, shift: false },
  k: { key: keys.KeyK, shift: false },
  l: { key: keys.KeyL, shift: false },
  m: { key: keys.KeyM, shift: false },
  n: { key: keys.KeyN, shift: false },
  o: { key: keys.KeyO, shift: false },
  p: { key: keys.KeyP, shift: false },
  q: { key: keys.KeyQ, shift: false },
  r: { key: keys.KeyR, shift: false },
  s: { key: keys.KeyS, shift: false },
  t: { key: keys.KeyT, shift: false },
  u: { key: keys.KeyU, shift: false },
  v: { key: keys.KeyV, shift: false },
  w: { key: keys.KeyW, shift: false },
  x: { key: keys.KeyX, shift: false },
  y: { key: keys.KeyY, shift: false },
  z: { key: keys.KeyZ, shift: false },

  // Uppercase letters
  A: { key: keys.KeyA, shift: true },
  B: { key: keys.KeyB, shift: true },
  C: { key: keys.KeyC, shift: true },
  D: { key: keys.KeyD, shift: true },
  E: { key: keys.KeyE, shift: true },
  F: { key: keys.KeyF, shift: true },
  G: { key: keys.KeyG, shift: true },
  H: { key: keys.KeyH, shift: true },
  I: { key: keys.KeyI, shift: true },
  J: { key: keys.KeyJ, shift: true },
  K: { key: keys.KeyK, shift: true },
  L: { key: keys.KeyL, shift: true },
  M: { key: keys.KeyM, shift: true },
  N: { key: keys.KeyN, shift: true },
  O: { key: keys.KeyO, shift: true },
  P: { key: keys.KeyP, shift: true },
  Q: { key: keys.KeyQ, shift: true },
  R: { key: keys.KeyR, shift: true },
  S: { key: keys.KeyS, shift: true },
  T: { key: keys.KeyT, shift: true },
  U: { key: keys.KeyU, shift: true },
  V: { key: keys.KeyV, shift: true },
  W: { key: keys.KeyW, shift: true },
  X: { key: keys.KeyX, shift: true },
  Y: { key: keys.KeyY, shift: true },
  Z: { key: keys.KeyZ, shift: true },

  // Number row
  '0': { key: keys.Digit0, shift: false },
  '1': { key: keys.Digit1, shift: false },
  '2': { key: keys.Digit2, shift: false },
  '3': { key: keys.Digit3, shift: false },
  '4': { key: keys.Digit4, shift: false },
  '5': { key: keys.Digit5, shift: false },
  '6': { key: keys.Digit6, shift: false },
  '7': { key: keys.Digit7, shift: false },
  '8': { key: keys.Digit8, shift: false },
  '9': { key: keys.Digit9, shift: false },

  // Shifted number row symbols
  ')': { key: keys.Digit0, shift: true },
  '!': { key: keys.Digit1, shift: true },
  '@': { key: keys.Digit2, shift: true },
  '#': { key: keys.Digit3, shift: true },
  '$': { key: keys.Digit4, shift: true },
  '%': { key: keys.Digit5, shift: true },
  '^': { key: keys.Digit6, shift: true },
  '&': { key: keys.Digit7, shift: true },
  '*': { key: keys.Digit8, shift: true },
  '(': { key: keys.Digit9, shift: true },

  // Punctuation and symbols
  '-': { key: keys.Minus, shift: false },
  '=': { key: keys.Equal, shift: false },
  '[': { key: keys.BracketLeft, shift: false },
  ']': { key: keys.BracketRight, shift: false },
  '\\': { key: keys.Backslash, shift: false },
  ';': { key: keys.Semicolon, shift: false },
  "'": { key: keys.Quote, shift: false },
  '`': { key: keys.Backquote, shift: false },
  ',': { key: keys.Comma, shift: false },
  '.': { key: keys.Period, shift: false },
  '/': { key: keys.Slash, shift: false },

  // Shifted punctuation and symbols
  _: { key: keys.Minus, shift: true },
  '+': { key: keys.Equal, shift: true },
  '{': { key: keys.BracketLeft, shift: true },
  '}': { key: keys.BracketRight, shift: true },
  '|': { key: keys.Backslash, shift: true },
  ':': { key: keys.Semicolon, shift: true },
  '"': { key: keys.Quote, shift: true },
  '~': { key: keys.Backquote, shift: true },
  '<': { key: keys.Comma, shift: true },
  '>': { key: keys.Period, shift: true },
  '?': { key: keys.Slash, shift: true },

  // Whitespace and control
  ' ': { key: keys.Space, shift: false },
  '\t': { key: keys.Tab, shift: false },
  '\n': { key: keys.Enter, shift: false },
  '\r': { key: keys.Enter, shift: false },
}

/**
 * Get canonical key and modifier state for a character
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
