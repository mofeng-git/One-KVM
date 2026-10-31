import { ref } from 'vue'

export type SessionEndReason = 'taken_over' | 'expired'

const sessionEndReason = ref<SessionEndReason | null>(null)

/** 会话失效（被别处登录挤掉 / 过期）时调用，App.vue 会弹出遮罩对话框 */
export function notifySessionEnd(reason: SessionEndReason) {
  sessionEndReason.value = reason
}

export function clearSessionEnd() {
  sessionEndReason.value = null
}

export function useSessionGuard() {
  return { sessionEndReason, notifySessionEnd, clearSessionEnd }
}

let watchTimer: number | null = null

/**
 * 轮询 /api/auth/check：会话被别处登录挤掉（后端返回 401 + Logged in elsewhere）时弹窗。
 * WebSocket 不会因会话撤销而断开，所以必须主动探测。
 */
export function startSessionWatch(intervalMs = 5000) {
  if (watchTimer !== null) return
  watchTimer = window.setInterval(async () => {
    if (sessionEndReason.value) return
    if (window.location.pathname.startsWith('/login')) return
    try {
      const res = await fetch('/api/auth/check', { credentials: 'include' })
      // 会话恢复正常（重新登录成功）就自动收起弹窗
      if (res.ok) {
        if (sessionEndReason.value) sessionEndReason.value = null
        return
      }
      if (res.status === 401) {
        const text = (await res.text()).toLowerCase()
        // 请求期间可能已经跳到登录页，这里再判一次，避免登录页还在弹窗
        if (window.location.pathname.startsWith('/login')) return
        notifySessionEnd(text.includes('logged in elsewhere') ? 'taken_over' : 'expired')
      }
    } catch {
      // 网络抖动忽略，下一轮再试
    }
  }, intervalMs)
}
