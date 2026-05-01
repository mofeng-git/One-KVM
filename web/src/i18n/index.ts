import { createI18n } from 'vue-i18n'
import zhCN from './zh-CN'
import enUS from './en-US'

export const supportedLanguages = [
  { code: 'zh-CN', name: '中文', flag: '🇨🇳' },
  { code: 'en-US', name: 'English', flag: '🇺🇸' },
] as const

export type SupportedLocale = (typeof supportedLanguages)[number]['code']

function detectLanguage(): SupportedLocale {
  const stored = localStorage.getItem('language')
  if (stored && supportedLanguages.some((l) => l.code === stored)) {
    return stored as SupportedLocale
  }

  const languages = navigator.languages || [navigator.language]
  for (const lang of languages) {
    const normalizedLang = lang.toLowerCase()
    if (normalizedLang.startsWith('zh')) {
      return 'zh-CN'
    }
    if (normalizedLang.startsWith('en')) {
      return 'en-US'
    }
  }

  return 'en-US'
}

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
