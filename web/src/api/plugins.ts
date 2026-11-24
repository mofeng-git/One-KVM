import { request } from './request'

export type LocalizedText = string | Record<string, string>
export type PluginMenu = string | { id: string; label: LocalizedText }

export interface PluginStatusField {
  key: string
  label: LocalizedText
  secret?: boolean
  available_offline?: boolean
  value_labels?: Record<string, LocalizedText>
  format?: 'percent'
}
export type ConfigValue = string | number | boolean | string[]

export interface PluginField {
  key: string
  label: LocalizedText
  kind: 'text' | 'password' | 'textarea' | 'integer' | 'boolean' | 'select' | 'string_list'
  default: ConfigValue
  choices: string[]
  choice_labels?: Record<string, LocalizedText>
  min: number | null
  max: number | null
  when?: { key: string; equals: ConfigValue } | null
}

export interface Plugin {
  manifest: {
    id: string
    name: LocalizedText
    version: string
    author: string
    description: LocalizedText
    menu: PluginMenu
    toolbar_icon?: 'scan_line' | 'wrench'
    config_schema: PluginField[]
    status_fields: PluginStatusField[]
    actions: {
      id: string
      label: LocalizedText
      confirm?: boolean
      when_status?: { key: string; equals: ConfigValue }[]
    }[]
    files: boolean
  }
  enabled: boolean
  autostart: boolean
  config: Record<string, ConfigValue>
  runtime: {
    phase: 'starting' | 'running' | 'stopped' | 'backoff'
    error?: string | null
    fields: Record<string, unknown>
  }
}

const headers = { 'x-one-kvm-plugin-management': '1' }
const path = (id: string) => `/plugins/${encodeURIComponent(id)}`
const filePath = (id: string, name: string) => `${path(id)}/files/${encodeURIComponent(name)}`

export const pluginsApi = {
  list: () => request<Plugin[]>('/plugins', {}, { toastOnError: false }),
  upload: (file: File) => request<Plugin>('/plugins', {
    method: 'POST', headers: { ...headers, 'Content-Type': 'application/octet-stream' }, body: file,
  }),
  install: (url: string) => request<Plugin>('/plugins/install-url', {
    method: 'POST', headers, body: JSON.stringify({ url }),
  }),
  configure: (id: string, values: { enabled?: boolean; autostart?: boolean; config?: Record<string, ConfigValue> }) =>
    request<Plugin>(path(id), { method: 'PATCH', headers, body: JSON.stringify(values) }),
  uninstall: (id: string) => request<{ success: boolean }>(path(id), { method: 'DELETE', headers }),
  logs: (id: string) => request<string[]>(`${path(id)}/logs`),
  action: (id: string, action: string) => request<{ result: unknown }>(
    `${path(id)}/actions/${encodeURIComponent(action)}`, { method: 'POST', headers },
  ),
  downloadUrl: (id: string, name: string) => `/api${filePath(id, name)}`,
  deleteFile: (id: string, name: string) => request<{ success: boolean }>(
    filePath(id, name), { method: 'DELETE', headers },
  ),
}
