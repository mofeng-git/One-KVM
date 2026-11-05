export interface VideoDeviceLabelSource {
  name?: string | null
  path: string
}

const WINDOWS_USB_ID_RE = /vid[_-]([0-9a-f]{4}).*pid[_-]([0-9a-f]{4})/i
const WINDOWS_SYMBOLIC_LINK_RE = /^\\\\\?\\/
const WINDOWS_DIRECTSHOW_RE = /^dshow:/i

function shortDevicePath(path: string): string {
  const normalized = path.replace(/\\/g, '/')
  const parts = normalized.split('/').filter(Boolean)
  return parts[parts.length - 1] || path
}

function isWindowsSymbolicLink(path: string): boolean {
  return WINDOWS_SYMBOLIC_LINK_RE.test(path)
}

function isWindowsDirectShowPath(path: string): boolean {
  return WINDOWS_DIRECTSHOW_RE.test(path)
}

function cleanWindowsDirectShowLabel(value: string): string {
  return value.replace(WINDOWS_DIRECTSHOW_RE, '').trim()
}

function fallbackDeviceName(path: string): string {
  if (isWindowsDirectShowPath(path)) return cleanWindowsDirectShowLabel(path) || 'Windows Capture Device'
  if (isWindowsSymbolicLink(path)) return 'Windows Capture Device'
  return shortDevicePath(path)
}

export function formatVideoDeviceLabel(device: VideoDeviceLabelSource): string {
  const path = device.path.trim()
  const rawName = device.name?.trim() || fallbackDeviceName(path)
  const name = isWindowsDirectShowPath(rawName) ? cleanWindowsDirectShowLabel(rawName) : rawName
  const usbId = path.match(WINDOWS_USB_ID_RE)

  if (usbId?.[1] && usbId[2]) {
    return `${name} (${usbId[1].toLowerCase()}:${usbId[2].toLowerCase()})`
  }

  if (!path) return name

  if (isWindowsDirectShowPath(path)) return cleanWindowsDirectShowLabel(name) || fallbackDeviceName(path)

  if (isWindowsSymbolicLink(path)) return name

  return `${name} (${shortDevicePath(path)})`
}
