export function isDebugLogEnabled(): boolean {
  if (typeof window === 'undefined') return false
  return new URLSearchParams(window.location.search).get('log') === 'debug'
}

export function videoDebugLog(message: string, details?: unknown): void {
  if (!isDebugLogEnabled()) return

  const timestamp = new Date().toISOString()
  if (details === undefined) {
    console.log(`[VideoDebug ${timestamp}] ${message}`)
  } else {
    console.log(`[VideoDebug ${timestamp}] ${message}`, details)
  }
}
