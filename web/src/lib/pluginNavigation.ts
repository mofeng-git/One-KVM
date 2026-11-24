import type { LocalizedText, Plugin, PluginMenu } from '@/api/plugins'

export function pluginText(text: LocalizedText | undefined, locale: string): string {
  if (typeof text === 'string') return text
  const translated = Object.entries(text ?? {}).filter(([, value]) => value.trim())
  const requested = locale.toLowerCase()
  return translated.find(([key]) => key.toLowerCase() === requested)?.[1]
    ?? translated.find(([key]) => key.toLowerCase().split('-')[0] === requested.split('-')[0])?.[1]
    ?? translated.find(([key]) => key.toLowerCase() === 'en-us')?.[1]
    ?? translated[0]?.[1]
    ?? ''
}

const pluginSettingsMenus = [
  { id: 'third_party_access', section: 'third-party-access', labelKey: 'extensions.thirdPartyAccess.title' },
  { id: 'remote_access', section: 'ext-remote-access', labelKey: 'extensions.remoteAccess.title' },
  { id: 'appearance', section: 'appearance', labelKey: 'settings.appearance' },
  { id: 'account', section: 'account', labelKey: 'settings.account' },
  { id: 'network', section: 'network', labelKey: 'settings.network' },
  { id: 'video', section: 'video', labelKey: 'settings.video' },
  { id: 'hid', section: 'hid', labelKey: 'settings.hid' },
  { id: 'atx', section: 'atx', labelKey: 'settings.atx' },
  { id: 'environment', section: 'environment', labelKey: 'settings.environment' },
  { id: 'other', section: 'other', labelKey: 'settings.other' },
  { id: 'ext-ttyd', section: 'ext-ttyd', labelKey: 'extensions.ttyd.title' },
  { id: 'mcp', section: 'mcp', labelKey: 'mcp.title' },
  { id: 'about', section: 'about', labelKey: 'settings.about' },
] as const

export const consoleToolbarMenu = 'console_toolbar'

function declaredMenuId(menu: PluginMenu) {
  const id = typeof menu === 'string' ? menu : menu.id
  return pluginSettingsMenus.find(menu => menu.section === id)?.id ?? id
}

export function pluginMenuId(plugin: Plugin) {
  return declaredMenuId(plugin.manifest.menu)
}

export function pluginSettingsSection(menuId: string): string | undefined {
  if (menuId === consoleToolbarMenu) return undefined
  return pluginSettingsMenus.find(menu => menu.id === menuId)?.section ?? `plugin-menu-${menuId}`
}

export function pluginMenuLabel(plugins: Plugin[], id: string, locale: string, t: (key: string) => string) {
  const builtIn = pluginSettingsMenus.find(menu => menu.id === id)
  if (builtIn) return t(builtIn.labelKey)
  for (const plugin of plugins) {
    const menu = plugin.manifest.menu
    if (typeof menu !== 'string' && declaredMenuId(menu) === id) {
      const label = pluginText(menu.label, locale)
      if (label) return label
    }
  }
  return id.split(/[-_]+/).filter(Boolean).map(part => part.charAt(0).toUpperCase() + part.slice(1)).join(' ')
}
