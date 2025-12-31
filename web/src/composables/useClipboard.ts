import { ref } from 'vue'

export function useClipboard() {
  const copied = ref(false)

  // 检测是否支持原生 Clipboard API (需要安全上下文 + API 存在)
  function canUseClipboardApi(): boolean {
    return !!(
      typeof navigator !== 'undefined' &&
      navigator.clipboard &&
      typeof navigator.clipboard.writeText === 'function' &&
      window.isSecureContext
    )
  }

  // Fallback: 使用 execCommand (兼容 HTTP 环境)
  function fallbackCopy(text: string): boolean {
    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.style.cssText = 'position:fixed;top:0;left:0;opacity:0;pointer-events:none'
    document.body.appendChild(textarea)
    textarea.focus()
    textarea.select()

    let success = false
    try {
      success = document.execCommand('copy')
    } catch {
      success = false
    }

    document.body.removeChild(textarea)
    return success
  }

  async function copy(text: string): Promise<boolean> {
    if (!text) return false

    try {
      if (canUseClipboardApi()) {
        await navigator.clipboard.writeText(text)
      } else {
        if (!fallbackCopy(text)) {
          return false
        }
      }

      copied.value = true
      setTimeout(() => (copied.value = false), 2000)
      return true
    } catch (e) {
      // Clipboard API 失败时尝试 fallback
      console.warn('Clipboard API failed, trying fallback:', e)
      if (fallbackCopy(text)) {
        copied.value = true
        setTimeout(() => (copied.value = false), 2000)
        return true
      }
      console.error('Copy failed:', e)
      return false
    }
  }

  return { copy, copied }
}
