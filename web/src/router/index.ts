import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
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
    meta: { requiresAuth: true, requiresAdmin: true },
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

// Navigation guard
router.beforeEach(async (to, _from, next) => {
  const authStore = useAuthStore()

  // Check if system needs setup
  if (!authStore.initialized && to.name !== 'Setup') {
    try {
      await authStore.checkSetupStatus()
      if (authStore.needsSetup) {
        return next({ name: 'Setup' })
      }
    } catch {
      // Continue anyway
    }
  }

  // Check authentication for protected routes
  if (to.meta.requiresAuth !== false) {
    if (!authStore.isAuthenticated) {
      try {
        await authStore.checkAuth()
      } catch {
        // Not authenticated
      }

      if (!authStore.isAuthenticated) {
        return next({ name: 'Login', query: { redirect: to.fullPath } })
      }
    }

    // Check admin requirement
    if (to.meta.requiresAdmin && !authStore.isAdmin) {
      // Redirect non-admin users to console
      return next({ name: 'Console' })
    }
  }

  next()
})

export default router
