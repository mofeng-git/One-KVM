import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { authApi, systemApi, type AuthLoginResponse } from '@/api'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<string | null>(null)
  const isAuthenticated = ref(false)
  const initialized = ref(false)
  const needsSetup = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)
  let pendingUsername: string | null = null

  const isLoggedIn = computed(() => isAuthenticated.value && user.value !== null)

  async function checkSetupStatus() {
    try {
      const status = await systemApi.setupStatus()
      initialized.value = status.initialized
      needsSetup.value = status.needs_setup
      return status
    } catch (e) {
      console.error('Failed to check setup status:', e)
      throw e
    }
  }

  async function checkAuth() {
    try {
      const result = await authApi.check()
      isAuthenticated.value = result.authenticated
      user.value = result.user || null
      return result
    } catch (e) {
      isAuthenticated.value = false
      user.value = null
      error.value = e instanceof Error ? e.message : 'Not authenticated'
      if (e instanceof Error) {
        throw e
      }
      throw new Error('Not authenticated')
    }
  }

  async function beginLogin(username: string, password: string): Promise<AuthLoginResponse | null> {
    loading.value = true
    error.value = null

    try {
      const result = await authApi.login(username, password)
      if (result.next === 'authenticated') {
        isAuthenticated.value = true
        user.value = username
        pendingUsername = null
      } else {
        isAuthenticated.value = false
        user.value = null
        pendingUsername = username
      }
      return result
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Login failed'
      pendingUsername = null
      return null
    } finally {
      loading.value = false
    }
  }

  async function completeTotpLogin(challengeId: string, code: string) {
    loading.value = true
    error.value = null
    try {
      const result = await authApi.loginTotp(challengeId, code)
      if (result.next !== 'authenticated') {
        error.value = 'Login failed'
        return false
      }
      isAuthenticated.value = true
      user.value = pendingUsername
      pendingUsername = null
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Login failed'
      return false
    } finally {
      loading.value = false
    }
  }

  async function login(username: string, password: string) {
    const result = await beginLogin(username, password)
    return result?.next === 'authenticated'
  }

  function cancelPendingLogin() {
    pendingUsername = null
    error.value = null
  }

  async function logout() {
    try {
      await authApi.logout()
    } finally {
      isAuthenticated.value = false
      user.value = null
    }
  }

  async function setup(data: {
    username: string
    password: string
    video_device?: string
    video_format?: string
    video_width?: number
    video_height?: number
    video_fps?: number
    hid_backend?: string
    hid_ch9329_port?: string
    hid_ch9329_baudrate?: number
    hid_otg_udc?: string
    hid_otg_profile?: string
    hid_otg_keyboard_leds?: boolean
    msd_enabled?: boolean
    encoder_backend?: string
    audio_device?: string
    ttyd_enabled?: boolean
    rustdesk_enabled?: boolean
  }) {
    loading.value = true
    error.value = null

    try {
      const result = await systemApi.setup(data)
      if (result.success) {
        initialized.value = true
        needsSetup.value = false
        return true
      } else {
        error.value = result.message || 'Setup failed'
        return false
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Setup failed'
      return false
    } finally {
      loading.value = false
    }
  }

  return {
    user,
    isAuthenticated,
    initialized,
    needsSetup,
    loading,
    error,
    isLoggedIn,
    checkSetupStatus,
    checkAuth,
    beginLogin,
    completeTotpLogin,
    cancelPendingLogin,
    login,
    logout,
    setup,
  }
})
