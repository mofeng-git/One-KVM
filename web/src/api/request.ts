import { toast } from 'vue-sonner'
import i18n from '@/i18n'

const API_BASE = '/api'

// Toast debounce mechanism - prevent toast spam (5 seconds)
const toastDebounceMap = new Map<string, number>()
const TOAST_DEBOUNCE_TIME = 5000

function shouldShowToast(key: string): boolean {
  const now = Date.now()
  const lastToastTime = toastDebounceMap.get(key)

  if (!lastToastTime || now - lastToastTime >= TOAST_DEBOUNCE_TIME) {
    toastDebounceMap.set(key, now)
    return true
  }

  return false
}

function t(key: string, params?: Record<string, unknown>): string {
  return String(i18n.global.t(key, params as any))
}

function hasTranslation(key: string): boolean {
  return i18n.global.te(key)
}

export class ApiError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

export interface ApiRequestConfig {
  /**
   * Enable toast notifications on errors.
   * Defaults to true to match existing behavior in api/index.ts.
   */
  toastOnError?: boolean
  /**
   * Toast debounce key. Defaults to `error_${endpoint}`.
   */
  toastKey?: string
}

function getToastKey(endpoint: string, config?: ApiRequestConfig): string {
  return config?.toastKey ?? `error_${endpoint}`
}

function getErrorMessage(data: unknown, fallback: string): string {
  if (data && typeof data === 'object') {
    const message = (data as any).message
    if (typeof message === 'string' && message.trim()) return localizeBackendErrorMessage(message)
  }
  return localizeBackendErrorMessage(fallback)
}

function extractCh9329Command(reason: string): string {
  const match = reason.match(/cmd 0x([0-9a-f]{2})/i)
  const cmd = match?.[1]
  return cmd ? `0x${cmd.toUpperCase()}` : ''
}

function localizeHidErrorMessage(raw: string): string | null {
  const match = raw.match(/^HID error \[([^\]]+)\]: (.*) \(code: ([^)]+)\)$/)
  if (!match) return null

  const backend = match[1] ?? ''
  const reason = match[2] ?? ''
  const code = match[3] ?? ''
  const command = extractCh9329Command(reason)

  const keyByCode: Record<string, string> = {
    udc_not_configured: 'hid.errorHints.udcNotConfigured',
    disabled: 'hid.errorHints.disabled',
    enoent: 'hid.errorHints.hidDeviceMissing',
    not_opened: 'hid.errorHints.notOpened',
    port_not_found: 'hid.errorHints.portNotFound',
    invalid_config: 'hid.errorHints.invalidConfig',
    no_response: command ? 'hid.errorHints.noResponseWithCmd' : 'hid.errorHints.noResponse',
    protocol_error: 'hid.errorHints.protocolError',
    invalid_response: 'hid.errorHints.protocolError',
    enxio: 'hid.errorHints.deviceDisconnected',
    enodev: 'hid.errorHints.deviceDisconnected',
    serial_error: 'hid.errorHints.serialError',
    init_failed: 'hid.errorHints.initFailed',
    shutdown: 'hid.errorHints.shutdown',
    reconnecting: 'hid.errorHints.reconnecting',
    worker_stopped: 'hid.errorHints.workerStopped',
  }

  const ioErrorCodes = new Set([
    'eio',
    'epipe',
    'eshutdown',
    'io_error',
    'write_failed',
    'read_failed',
    'device_unavailable',
  ])

  const key = keyByCode[code]
    ?? (ioErrorCodes.has(code)
      ? backend === 'otg'
        ? 'hid.errorHints.otgIoError'
        : backend === 'ch9329'
          ? 'hid.errorHints.ch9329IoError'
          : 'hid.errorHints.ioError'
      : '')

  if (key && hasTranslation(key)) {
    return t(key, { cmd: command })
  }

  return t('hid.errorHints.backendError', { backend })
}

function localizeBackendErrorMessage(raw: string): string {
  return localizeHidErrorMessage(raw) ?? raw
}

export async function request<T>(
  endpoint: string,
  options: RequestInit = {},
  config: ApiRequestConfig = {}
): Promise<T> {
  const url = `${API_BASE}${endpoint}`
  const toastOnError = config.toastOnError !== false
  const toastKey = getToastKey(endpoint, config)

  try {
    const response = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
      credentials: 'include',
    })

    const data = await response.json().catch(() => null)

    // Handle HTTP errors (in case backend returns non-2xx)
    if (!response.ok) {
      const message = getErrorMessage(data, `HTTP ${response.status}`)
      const normalized = message.toLowerCase()
      const isNotAuthenticated = normalized.includes('not authenticated')
      const isSessionExpired = normalized.includes('session expired')
      const isLoggedInElsewhere = normalized.includes('logged in elsewhere')
      const isAuthIssue = response.status === 401 && (isNotAuthenticated || isSessionExpired || isLoggedInElsewhere)
      if (toastOnError && shouldShowToast(toastKey) && !isAuthIssue) {
        toast.error(t('api.operationFailed'), {
          description: message,
          duration: 4000,
        })
      }
      throw new ApiError(response.status, message)
    }

    // Handle backend "success=false" convention (even when HTTP is 200)
    if (data && typeof (data as any).success === 'boolean' && !(data as any).success) {
      const message = getErrorMessage(data, t('api.operationFailedDesc'))

      if (toastOnError && shouldShowToast(toastKey)) {
        toast.error(t('api.operationFailed'), {
          description: message,
          duration: 4000,
        })
      }

      throw new ApiError(response.status, message)
    }

    // If response body isn't JSON (or empty), treat as failure for callers expecting JSON.
    if (data === null) {
      const message = t('api.parseResponseFailed')
      if (toastOnError && shouldShowToast(toastKey)) {
        toast.error(t('api.operationFailed'), {
          description: message,
          duration: 4000,
        })
      }
      throw new ApiError(response.status, message)
    }

    return data as T
  } catch (error) {
    if (error instanceof ApiError) throw error

    if (toastOnError && shouldShowToast('network_error')) {
      toast.error(t('api.networkError'), {
        description: t('api.networkErrorDesc'),
        duration: 4000,
      })
    }

    throw new ApiError(0, t('api.networkError'))
  }
}
