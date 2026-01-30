import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { toast } from 'vue-sonner'
import i18n from '@/i18n'
import { ApiError } from '@/api/request'
import { useAuthStore } from '@/stores/auth'

const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/LoginView.vue'),
    meta: { requiresAuth: false },
  },
  {
    path: '/setup',
    name: 'Setup',
    component: () => import('@/views/SetupView.vue'),
    meta: { requiresAuth: false },
  },
  {
    path: '/',
    name: 'Console',
    component: () => import('@/views/ConsoleView.vue'),
    meta: { requiresAuth: true },
  },
  {
    path: '/settings',
    name: 'Settings',
    component: () => import('@/views/SettingsView.vue'),
    meta: { requiresAuth: true },
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

let sessionExpiredNotified = false

function t(key: string, params?: Record<string, unknown>): string {
  return String(i18n.global.t(key, params as any))
}

// Navigation guard
router.beforeEach(async (to, _from, next) => {
  const authStore = useAuthStore()

  // Prevent access to setup after initialization
  const shouldCheckSetup = to.name === 'Setup' || !authStore.initialized
  if (shouldCheckSetup) {
    try {
      await authStore.checkSetupStatus()
    } catch {
      // Continue anyway
    }
  }

  if (authStore.needsSetup) {
    if (to.name !== 'Setup') {
      return next({ name: 'Setup' })
    }
  } else if (authStore.initialized && to.name === 'Setup') {
    if (!authStore.isAuthenticated) {
      try {
        await authStore.checkAuth()
      } catch {
        // Not authenticated
      }
    }

    return next({ name: authStore.isAuthenticated ? 'Console' : 'Login' })
  }

  // Check authentication for protected routes
  if (to.meta.requiresAuth !== false) {
    if (!authStore.isAuthenticated) {
      try {
        await authStore.checkAuth()
      } catch (e) {
        // Not authenticated
        if (e instanceof ApiError && e.status === 401 && !sessionExpiredNotified) {
          const normalized = e.message.toLowerCase()
          const isLoggedInElsewhere = normalized.includes('logged in elsewhere')
          const isSessionExpired = normalized.includes('session expired')
          if (isLoggedInElsewhere || isSessionExpired) {
            sessionExpiredNotified = true
            const titleKey = isLoggedInElsewhere ? 'auth.loggedInElsewhere' : 'auth.sessionExpired'
            toast.error(t(titleKey), {
              description: e.message,
              duration: 3000,
            })
          }
        }
      }

      if (!authStore.isAuthenticated) {
        return next({ name: 'Login', query: { redirect: to.fullPath } })
      }
    }

  }

  next()
})

export default router
