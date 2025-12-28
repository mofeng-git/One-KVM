<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { RouterView, useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useSystemStore } from '@/stores/system'

const router = useRouter()
const authStore = useAuthStore()
const systemStore = useSystemStore()

// Check for dark mode preference
function initTheme() {
  const stored = localStorage.getItem('theme')
  if (stored === 'dark' || (!stored && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
    document.documentElement.classList.add('dark')
  }
}

// Initialize on mount
onMounted(async () => {
  initTheme()

  // Check setup status
  try {
    await authStore.checkSetupStatus()
    if (authStore.needsSetup) {
      router.push('/setup')
      return
    }
  } catch {
    // Continue anyway
  }

  // Check auth status
  try {
    await authStore.checkAuth()
    if (authStore.isAuthenticated) {
      // Fetch system info
      await systemStore.fetchSystemInfo()
    }
  } catch {
    // Not authenticated
  }
})

// Listen for dark mode changes
watch(
  () => window.matchMedia('(prefers-color-scheme: dark)').matches,
  (dark) => {
    const stored = localStorage.getItem('theme')
    if (!stored) {
      document.documentElement.classList.toggle('dark', dark)
    }
  }
)
</script>

<template>
  <RouterView />
</template>
