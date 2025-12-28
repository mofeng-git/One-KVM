import { createI18n } from 'vue-i18n'
import zhCN from './zh-CN'
import enUS from './en-US'

// Supported languages
export const supportedLanguages = [
  { code: 'zh-CN', name: '中文', flag: '🇨🇳' },
  { code: 'en-US', name: 'English', flag: '🇺🇸' },
] as const

export type SupportedLocale = (typeof supportedLanguages)[number]['code']

// Detect browser language with improved logic
function detectLanguage(): SupportedLocale {
  // 1. Check localStorage for saved preference
  const stored = localStorage.getItem('language')
  if (stored && supportedLanguages.some((l) => l.code === stored)) {
    return stored as SupportedLocale
  }

  // 2. Check browser language list (navigator.languages is more comprehensive)
  const languages = navigator.languages || [navigator.language]
  for (const lang of languages) {
    const normalizedLang = lang.toLowerCase()
    // Check for Chinese variants (zh, zh-CN, zh-TW, zh-HK, etc.)
    if (normalizedLang.startsWith('zh')) {
      return 'zh-CN'
    }
    // Check for English variants
    if (normalizedLang.startsWith('en')) {
      return 'en-US'
    }
  }

  // 3. Default to English
  return 'en-US'
}

// Initialize language and set HTML lang attribute
function initializeLanguage(): SupportedLocale {
  const lang = detectLanguage()
  document.documentElement.setAttribute('lang', lang)
  return lang
}

const i18n = createI18n({
  legacy: false,
  locale: initializeLanguage(),
  fallbackLocale: 'en-US',
  messages: {
    'zh-CN': zhCN,
    'en-US': enUS,
  },
})

export function setLanguage(lang: SupportedLocale) {
  i18n.global.locale.value = lang
  localStorage.setItem('language', lang)
  document.documentElement.setAttribute('lang', lang)
}

export function getCurrentLanguage(): SupportedLocale {
  return i18n.global.locale.value as SupportedLocale
}

export function getLanguageInfo(code: SupportedLocale) {
  return supportedLanguages.find((l) => l.code === code)
}

export default i18n
