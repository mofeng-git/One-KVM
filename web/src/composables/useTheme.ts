import { readonly, ref } from 'vue'

export type Theme = 'light' | 'dark' | 'system'

const theme = ref<Theme>('system')
const isDark = ref(false)
let initialized = false
let mediaQuery: MediaQueryList | null = null

function applyTheme() {
  const dark = theme.value === 'dark' || (theme.value === 'system' && mediaQuery?.matches === true)
  isDark.value = dark
  document.documentElement.classList.toggle('dark', dark)
}

function handleSystemThemeChange() {
  if (theme.value === 'system') applyTheme()
}

function initializeTheme() {
  if (initialized || typeof window === 'undefined') return

  const storedTheme = localStorage.getItem('theme')
  if (storedTheme === 'light' || storedTheme === 'dark' || storedTheme === 'system') {
    theme.value = storedTheme
  }

  mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  mediaQuery.addEventListener('change', handleSystemThemeChange)
  initialized = true
  applyTheme()
}

function setTheme(value: Theme) {
  initializeTheme()
  theme.value = value
  localStorage.setItem('theme', value)
  applyTheme()
}

function toggleTheme() {
  initializeTheme()
  setTheme(isDark.value ? 'light' : 'dark')
}

export function useTheme() {
  initializeTheme()
  return {
    theme: readonly(theme),
    isDark: readonly(isDark),
    setTheme,
    toggleTheme,
  }
}
