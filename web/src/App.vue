<script setup lang="ts">
import 'vue-sonner/style.css'
import '@/sonner-overrides.css'
import { computed, KeepAlive, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterView, useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useSystemStore } from '@/stores/system'
import { Toaster } from '@/components/ui/sonner'

const { t } = useI18n()

/** Defaults merged into every toast; duration also set on `<Toaster>` for clarity */
const toasterToastOptions = computed(() => ({
  closeButtonAriaLabel: t('toast.closeNotification'),
  classes: {
    title: 'text-sm font-semibold leading-snug tracking-tight text-popover-foreground',
    description: 'text-sm leading-relaxed text-muted-foreground',
  },
}))

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
  <Toaster
    rich-colors
    close-button
    position="top-center"
    close-button-position="top-right"
    theme="system"
    :duration="4000"
    :gap="14"
    :visible-toasts="3"
    :offset="{ top: '1rem', right: '1rem', left: '1rem', bottom: '1rem' }"
    :mobile-offset="{ top: 'max(1rem, env(safe-area-inset-top))', bottom: 'max(1rem, env(safe-area-inset-bottom))', left: '1rem', right: '1rem' }"
    :toast-options="toasterToastOptions"
    :container-aria-label="t('toast.notificationsRegion')"
  />
</template>
