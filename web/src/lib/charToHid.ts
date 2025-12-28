// Character to JavaScript keyCode mapping for text paste functionality
// Maps printable ASCII characters to JavaScript keyCodes that the backend expects
// The backend (keymap.rs) will convert these JS keyCodes to USB HID keycodes

export interface CharKeyMapping {
  keyCode: number // JavaScript keyCode (same as KeyboardEvent.keyCode)
  shift: boolean // Whether Shift modifier is needed
}

// US QWERTY keyboard layout mapping
// Maps characters to their JavaScript keyCode and whether Shift is required
const charToKeyMap: Record<string, CharKeyMapping> = {
  // Lowercase letters (no shift) - JS keyCodes 65-90
  a: { keyCode: 65, shift: false },
  b: { keyCode: 66, shift: false },
  c: { keyCode: 67, shift: false },
  d: { keyCode: 68, shift: false },
  e: { keyCode: 69, shift: false },
  f: { keyCode: 70, shift: false },
  g: { keyCode: 71, shift: false },
  h: { keyCode: 72, shift: false },
  i: { keyCode: 73, shift: false },
  j: { keyCode: 74, shift: false },
  k: { keyCode: 75, shift: false },
  l: { keyCode: 76, shift: false },
  m: { keyCode: 77, shift: false },
  n: { keyCode: 78, shift: false },
  o: { keyCode: 79, shift: false },
  p: { keyCode: 80, shift: false },
  q: { keyCode: 81, shift: false },
  r: { keyCode: 82, shift: false },
  s: { keyCode: 83, shift: false },
  t: { keyCode: 84, shift: false },
  u: { keyCode: 85, shift: false },
  v: { keyCode: 86, shift: false },
  w: { keyCode: 87, shift: false },
  x: { keyCode: 88, shift: false },
  y: { keyCode: 89, shift: false },
  z: { keyCode: 90, shift: false },

  // Uppercase letters (with shift) - same keyCodes, just with Shift
  A: { keyCode: 65, shift: true },
  B: { keyCode: 66, shift: true },
  C: { keyCode: 67, shift: true },
  D: { keyCode: 68, shift: true },
  E: { keyCode: 69, shift: true },
  F: { keyCode: 70, shift: true },
  G: { keyCode: 71, shift: true },
  H: { keyCode: 72, shift: true },
  I: { keyCode: 73, shift: true },
  J: { keyCode: 74, shift: true },
  K: { keyCode: 75, shift: true },
  L: { keyCode: 76, shift: true },
  M: { keyCode: 77, shift: true },
  N: { keyCode: 78, shift: true },
  O: { keyCode: 79, shift: true },
  P: { keyCode: 80, shift: true },
  Q: { keyCode: 81, shift: true },
  R: { keyCode: 82, shift: true },
  S: { keyCode: 83, shift: true },
  T: { keyCode: 84, shift: true },
  U: { keyCode: 85, shift: true },
  V: { keyCode: 86, shift: true },
  W: { keyCode: 87, shift: true },
  X: { keyCode: 88, shift: true },
  Y: { keyCode: 89, shift: true },
  Z: { keyCode: 90, shift: true },

  // Numbers (no shift) - JS keyCodes 48-57
  '0': { keyCode: 48, shift: false },
  '1': { keyCode: 49, shift: false },
  '2': { keyCode: 50, shift: false },
  '3': { keyCode: 51, shift: false },
  '4': { keyCode: 52, shift: false },
  '5': { keyCode: 53, shift: false },
  '6': { keyCode: 54, shift: false },
  '7': { keyCode: 55, shift: false },
  '8': { keyCode: 56, shift: false },
  '9': { keyCode: 57, shift: false },

  // Shifted number row symbols (US layout)
  ')': { keyCode: 48, shift: true }, // Shift + 0
  '!': { keyCode: 49, shift: true }, // Shift + 1
  '@': { keyCode: 50, shift: true }, // Shift + 2
  '#': { keyCode: 51, shift: true }, // Shift + 3
  $: { keyCode: 52, shift: true }, // Shift + 4
  '%': { keyCode: 53, shift: true }, // Shift + 5
  '^': { keyCode: 54, shift: true }, // Shift + 6
  '&': { keyCode: 55, shift: true }, // Shift + 7
  '*': { keyCode: 56, shift: true }, // Shift + 8
  '(': { keyCode: 57, shift: true }, // Shift + 9

  // Punctuation and symbols (no shift) - US layout JS keyCodes
  '-': { keyCode: 189, shift: false }, // Minus
  '=': { keyCode: 187, shift: false }, // Equal
  '[': { keyCode: 219, shift: false }, // Left bracket
  ']': { keyCode: 221, shift: false }, // Right bracket
  '\\': { keyCode: 220, shift: false }, // Backslash
  ';': { keyCode: 186, shift: false }, // Semicolon
  "'": { keyCode: 222, shift: false }, // Apostrophe/Quote
  '`': { keyCode: 192, shift: false }, // Grave/Backtick
  ',': { keyCode: 188, shift: false }, // Comma
  '.': { keyCode: 190, shift: false }, // Period
  '/': { keyCode: 191, shift: false }, // Slash

  // Shifted punctuation and symbols (US layout)
  _: { keyCode: 189, shift: true }, // Shift + Minus = Underscore
  '+': { keyCode: 187, shift: true }, // Shift + Equal = Plus
  '{': { keyCode: 219, shift: true }, // Shift + [ = {
  '}': { keyCode: 221, shift: true }, // Shift + ] = }
  '|': { keyCode: 220, shift: true }, // Shift + \ = |
  ':': { keyCode: 186, shift: true }, // Shift + ; = :
  '"': { keyCode: 222, shift: true }, // Shift + ' = "
  '~': { keyCode: 192, shift: true }, // Shift + ` = ~
  '<': { keyCode: 188, shift: true }, // Shift + , = <
  '>': { keyCode: 190, shift: true }, // Shift + . = >
  '?': { keyCode: 191, shift: true }, // Shift + / = ?

  // Whitespace and control characters
  ' ': { keyCode: 32, shift: false }, // Space
  '\t': { keyCode: 9, shift: false }, // Tab
  '\n': { keyCode: 13, shift: false }, // Enter (LF)
  '\r': { keyCode: 13, shift: false }, // Enter (CR)
}

/**
 * Get the JavaScript keyCode and modifier state for a character
 * @param char - Single character to convert
 * @returns CharKeyMapping or null if character is not mappable
 */
export function charToKey(char: string): CharKeyMapping | null {
  if (char.length !== 1) return null
  return charToKeyMap[char] || null
}

/**
 * Check if a character can be typed via HID
 * @param char - Single character to check
 */
export function isTypableChar(char: string): boolean {
  return char.length === 1 && char in charToKeyMap
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
