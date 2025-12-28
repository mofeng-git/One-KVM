<script setup lang="ts">
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { Monitor, Lock, Eye, EyeOff, User } from 'lucide-vue-next'

const { t } = useI18n()
const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()

const username = ref('')
const password = ref('')
const showPassword = ref(false)
const loading = ref(false)
const error = ref('')

async function handleLogin() {
  if (!username.value) {
    error.value = t('auth.enterUsername')
    return
  }
  if (!password.value) {
    error.value = t('auth.enterPassword')
    return
  }

  loading.value = true
  error.value = ''

  const success = await authStore.login(username.value, password.value)

  if (success) {
    const redirect = route.query.redirect as string
    router.push(redirect || '/')
  } else {
    error.value = authStore.error || t('auth.loginFailed')
  }

  loading.value = false
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    handleLogin()
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-background p-4">
    <div class="w-full max-w-sm space-y-6">
      <!-- Logo and Title -->
      <div class="text-center space-y-2">
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-primary/10">
          <Monitor class="w-8 h-8 text-primary" />
        </div>
        <h1 class="text-2xl font-bold text-foreground">One-KVM</h1>
        <p class="text-sm text-muted-foreground">{{ t('auth.loginPrompt') }}</p>
      </div>

      <!-- Login Form -->
      <div class="space-y-4">
        <!-- Username Input -->
        <div class="relative">
          <div class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
            <User class="w-4 h-4" />
          </div>
          <input
            v-model="username"
            type="text"
            :placeholder="t('auth.username')"
            class="w-full h-10 pl-10 pr-4 rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
            @keydown="handleKeydown"
          />
        </div>

        <!-- Password Input -->
        <div class="relative">
          <div class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
            <Lock class="w-4 h-4" />
          </div>
          <input
            v-model="password"
            :type="showPassword ? 'text' : 'password'"
            :placeholder="t('auth.password')"
            class="w-full h-10 pl-10 pr-10 rounded-md border border-input bg-background text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
            @keydown="handleKeydown"
          />
          <button
            type="button"
            class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            @click="showPassword = !showPassword"
          >
            <Eye v-if="!showPassword" class="w-4 h-4" />
            <EyeOff v-else class="w-4 h-4" />
          </button>
        </div>

        <button
          class="w-full h-10 rounded-md bg-primary text-primary-foreground font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          :disabled="loading"
          @click="handleLogin"
        >
          <span v-if="loading">{{ t('common.loading') }}</span>
          <span v-else>{{ t('auth.login') }}</span>
        </button>

        <p v-if="error" class="text-sm text-destructive text-center">{{ error }}</p>
      </div>
    </div>
  </div>
</template>
