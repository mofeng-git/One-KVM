<script setup lang="ts">
import 'vue-sonner/style.css'
import '@/sonner-overrides.css'
import { computed, KeepAlive, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useSystemStore } from '@/stores/system'
import { Toaster } from '@/components/ui/sonner'
import { useTheme } from '@/composables/useTheme'
import { startSessionWatch, useSessionGuard } from '@/composables/useSessionGuard'
import { Button } from '@/components/ui/button'

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
const route = useRoute()
const isLoginPage = computed(() => route.path.startsWith('/login'))
const authStore = useAuthStore()
const systemStore = useSystemStore()
const { isDark } = useTheme()
const { sessionEndReason, clearSessionEnd } = useSessionGuard()
startSessionWatch()
// 一进入登录页就清掉弹窗状态，避免重复提示
watch(isLoginPage, (onLogin) => {
  if (onLogin) clearSessionEnd()
}, { immediate: true })

function goToLogin() {
  clearSessionEnd()
  // 会话已被服务端作废：整页跳到登录页，顺便清掉所有内存态，避免任何守卫把用户弹回控制台
  window.location.href = '/login'
}
onMounted(async () => {
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
</script>

<template>
  <RouterView v-slot="{ Component, route }">
    <KeepAlive v-if="authStore.isAuthenticated">
      <component :is="Component" v-if="route.name === 'Console'" />
    </KeepAlive>
    <component :is="Component" v-if="route.name !== 'Console' || !authStore.isAuthenticated" />
  </RouterView>
  <div
    v-if="sessionEndReason && !isLoginPage"
    class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm"
  >
    <div class="w-full max-w-sm rounded-xl border bg-background p-5 shadow-2xl">
      <h2 class="text-base font-semibold">
        {{ sessionEndReason === 'taken_over'
          ? t('auth.sessionTakenOver')
          : t('auth.sessionExpiredTitle') }}
      </h2>
      <p class="mt-2 text-sm leading-6 text-muted-foreground">
        {{ sessionEndReason === 'taken_over'
          ? t('auth.sessionTakenOverDesc')
          : t('auth.sessionExpiredDesc') }}
      </p>
      <div class="mt-4 flex justify-end">
        <Button size="sm" @click="goToLogin()">{{ t('auth.signInAgain') }}</Button>
      </div>
    </div>
  </div>
  <Toaster
    rich-colors
    close-button
    expand
    position="top-center"
    close-button-position="top-right"
    :theme="isDark ? 'dark' : 'light'"
    :duration="4000"
    :gap="14"
    :offset="{ top: '1rem', right: '1rem', left: '1rem', bottom: '1rem' }"
    :mobile-offset="{ top: 'max(1rem, env(safe-area-inset-top))', bottom: 'max(1rem, env(safe-area-inset-bottom))', left: '1rem', right: '1rem' }"
    :toast-options="toasterToastOptions"
    :container-aria-label="t('toast.notificationsRegion')"
  />
</template>
