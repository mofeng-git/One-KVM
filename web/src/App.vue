<script setup lang="ts">
import 'vue-sonner/style.css'
import { KeepAlive, onMounted, watch } from 'vue'
import { RouterView, useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useSystemStore } from '@/stores/system'
import { Toaster } from '@/components/ui/sonner'

const router = useRouter()
const authStore = useAuthStore()
const systemStore = useSystemStore()

function initTheme() {
  const stored = localStorage.getItem('theme')
  if (stored === 'dark' || (!stored && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
    document.documentElement.classList.add('dark')
  }
}

onMounted(async () => {
  initTheme()

  try {
    await authStore.checkSetupStatus()
    if (authStore.needsSetup) {
      router.push('/setup')
      return
    }
  } catch {
  }

  try {
    await authStore.checkAuth()
    if (authStore.isAuthenticated) {
      await systemStore.fetchSystemInfo()
    }
  } catch {
  }
})

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
  <RouterView v-slot="{ Component, route }">
    <KeepAlive v-if="authStore.isAuthenticated">
      <component :is="Component" v-if="route.name === 'Console'" />
    </KeepAlive>
    <component :is="Component" v-if="route.name !== 'Console' || !authStore.isAuthenticated" />
  </RouterView>
  <Toaster rich-colors close-button position="top-center" />
</template>
