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
    if (typeof message === 'string' && message.trim()) return message
  }
  return fallback
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
