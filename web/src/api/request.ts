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
  code?: string

  constructor(status: number, message: string, code?: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
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
  /** Translation key used as the error toast title. */
  errorTitleKey?: string
}

function getToastKey(endpoint: string, config?: ApiRequestConfig): string {
  return config?.toastKey ?? `error_${endpoint}`
}

function isAuthenticationIssue(status: number, message: string): boolean {
  const normalized = message.toLowerCase()
  return status === 401 && (
    normalized.includes('not authenticated')
    || normalized.includes('session expired')
    || normalized.includes('logged in elsewhere')
  )
}

const msdErrorKeys: Record<string, string> = {
  MSD_UNAVAILABLE: 'msd.errors.unavailable',
  MSD_OPERATION_IN_PROGRESS: 'msd.errors.operationInProgress',
  MSD_OPERATION_FAILED: 'msd.errors.operationFailed',
  MSD_INVALID_REQUEST: 'msd.errors.invalidRequest',
  MSD_RESOURCE_NOT_FOUND: 'msd.errors.resourceNotFound',
  MSD_RESOURCE_ALREADY_EXISTS: 'msd.errors.resourceAlreadyExists',
  MSD_MEDIA_SLOTS_FULL: 'msd.errors.mediaSlotsFull',
  MSD_MEDIA_ALREADY_MOUNTED: 'msd.errors.mediaAlreadyMounted',
  MSD_MEDIA_IN_USE: 'msd.errors.mediaInUse',
  MSD_IMAGE_TOO_LARGE: 'msd.errors.imageTooLarge',
  MSD_INVALID_URL: 'msd.errors.invalidUrl',
  MSD_REMOTE_DOWNLOAD_FAILED: 'msd.errors.remoteDownloadFailed',
  MSD_DOWNLOAD_INCOMPLETE: 'msd.errors.downloadIncomplete',
  MSD_DRIVE_NOT_INITIALIZED: 'msd.errors.driveNotInitialized',
  MSD_DRIVE_CONNECTED: 'msd.errors.driveConnected',
  MSD_DRIVE_FILESYSTEM_UNSUPPORTED: 'msd.errors.driveFilesystemUnsupported',
  MSD_DRIVE_SIZE_INVALID: 'msd.errors.driveSizeInvalid',
  MSD_STORAGE_SPACE_UNAVAILABLE: 'msd.errors.storageSpaceUnavailable',
  MSD_STORAGE_FULL: 'msd.errors.storageFull',
  MSD_STORAGE_READ_ONLY: 'msd.errors.storageReadOnly',
  MSD_STORAGE_PERMISSION_DENIED: 'msd.errors.storagePermissionDenied',
  MSD_MEDIUM_REMOVAL_PREVENTED: 'msd.errors.mediumRemovalPrevented',
  MSD_DISCONNECT_FAILED: 'msd.errors.disconnectFailed',
}

export function localizeMsdErrorCode(code?: string, fallback?: string): string {
  const key = code ? msdErrorKeys[code] : undefined
  if (key && hasTranslation(key)) return t(key)
  return fallback ? localizeBackendErrorMessage(fallback) : t('msd.errors.operationFailed')
}

function getErrorDetails(data: unknown, fallback: string): { message: string; code?: string } {
  if (data && typeof data === 'object') {
    const code = (data as any).code
    const normalizedCode = typeof code === 'string' ? code : undefined
    if (normalizedCode && msdErrorKeys[normalizedCode]) {
      return { message: localizeMsdErrorCode(normalizedCode), code: normalizedCode }
    }

    const message = (data as any).message
    if (typeof message === 'string' && message.trim()) {
      return { message: localizeBackendErrorMessage(message), code: normalizedCode }
    }
  }
  return { message: localizeBackendErrorMessage(fallback) }
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
  const errorTitle = t(config.errorTitleKey ?? 'api.operationFailed')

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
      const { message, code } = getErrorDetails(data, `HTTP ${response.status}`)
      if (toastOnError && shouldShowToast(toastKey) && !isAuthenticationIssue(response.status, message)) {
        toast.error(errorTitle, {
          description: message,
          duration: 4000,
        })
      }
      throw new ApiError(response.status, message, code)
    }

    // Handle backend "success=false" convention (even when HTTP is 200)
    if (data && typeof (data as any).success === 'boolean' && !(data as any).success) {
      const { message, code } = getErrorDetails(data, t('api.operationFailedDesc'))

      if (toastOnError && shouldShowToast(toastKey)) {
        toast.error(errorTitle, {
          description: message,
          duration: 4000,
        })
      }

      throw new ApiError(response.status, message, code)
    }

    // If response body isn't JSON (or empty), treat as failure for callers expecting JSON.
    if (data === null) {
      const message = t('api.parseResponseFailed')
      if (toastOnError && shouldShowToast(toastKey)) {
        toast.error(errorTitle, {
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

export function uploadRequest<T>(
  endpoint: string,
  formData: FormData,
  onProgress?: (progress: number) => void,
  config: ApiRequestConfig = {},
): Promise<T> {
  const xhr = new XMLHttpRequest()
  xhr.open('POST', `${API_BASE}${endpoint}`)
  xhr.withCredentials = true

  return new Promise<T>((resolve, reject) => {
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable && onProgress) onProgress((event.loaded / event.total) * 100)
    }

    xhr.onload = () => {
      const data: unknown = (() => {
        try { return JSON.parse(xhr.responseText) } catch { return null }
      })()
      if (xhr.status >= 200 && xhr.status < 300 && data !== null) {
        resolve(data as T)
        return
      }

      const { message, code } = getErrorDetails(data, `HTTP ${xhr.status}`)
      const error = new ApiError(xhr.status, message, code)
      if (
        config.toastOnError !== false
        && shouldShowToast(getToastKey(endpoint, config))
        && !isAuthenticationIssue(xhr.status, message)
      ) {
        toast.error(t(config.errorTitleKey ?? 'api.operationFailed'), {
          description: message,
          duration: 4000,
        })
      }
      reject(error)
    }

    xhr.onerror = () => {
      if (config.toastOnError !== false && shouldShowToast('network_error')) {
        toast.error(t('api.networkError'), {
          description: t('api.networkErrorDesc'),
          duration: 4000,
        })
      }
      reject(new ApiError(0, t('api.networkError')))
    }
    xhr.send(formData)
  })
}
