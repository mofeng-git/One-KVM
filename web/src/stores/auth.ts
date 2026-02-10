import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { authApi, systemApi } from '@/api'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<string | null>(null)
  const isAuthenticated = ref(false)
  const initialized = ref(false)
  const needsSetup = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

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

  async function login(username: string, password: string) {
    loading.value = true
    error.value = null

    try {
      const result = await authApi.login(username, password)
      if (result.success) {
        isAuthenticated.value = true
        user.value = username
        return true
      } else {
        error.value = result.message || 'Login failed'
        return false
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Login failed'
      return false
    } finally {
      loading.value = false
    }
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
    login,
    logout,
    setup,
  }
})
