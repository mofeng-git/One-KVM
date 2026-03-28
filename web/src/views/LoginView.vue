<script setup lang="ts">
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import {
  setLanguage,
  getCurrentLanguage,
  type SupportedLocale,
} from '@/i18n'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Monitor, Lock, Eye, EyeOff, User } from 'lucide-vue-next'

const { t } = useI18n()
const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()

const currentLanguage = ref<SupportedLocale>(getCurrentLanguage())
const username = ref('')
const password = ref('')
const showPassword = ref(false)
const loading = ref(false)
const error = ref('')

function handleLanguageChange(lang: SupportedLocale) {
  currentLanguage.value = lang
  setLanguage(lang)
}

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
</script>

<template>
  <div class="min-h-screen min-h-dvh flex items-center justify-center bg-background p-4">
    <Card class="relative w-full max-w-sm">
      <div class="absolute top-4 right-4 flex gap-2">
        <Button
          :variant="currentLanguage === 'zh-CN' ? 'default' : 'outline'"
          size="sm"
          @click="handleLanguageChange('zh-CN')"
        >
          中文
        </Button>
        <Button
          :variant="currentLanguage === 'en-US' ? 'default' : 'outline'"
          size="sm"
          @click="handleLanguageChange('en-US')"
        >
          English
        </Button>
      </div>

      <CardHeader class="space-y-2 pt-10 text-center sm:pt-12">
        <div class="inline-flex h-16 w-16 items-center justify-center rounded-full bg-primary/10 mx-auto">
          <Monitor class="w-8 h-8 text-primary" />
        </div>
        <CardTitle class="text-xl sm:text-2xl">One-KVM</CardTitle>
        <CardDescription>{{ t('auth.login') }}</CardDescription>
      </CardHeader>

      <CardContent>
        <form class="space-y-4" @submit.prevent="handleLogin">
          <div class="space-y-2">
            <Label for="username">{{ t('auth.username') }}</Label>
            <div class="relative">
              <User class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                id="username"
                v-model="username"
                type="text"
                :placeholder="t('auth.username')"
                class="pl-10"
              />
            </div>
          </div>

          <div class="space-y-2">
            <Label for="password">{{ t('auth.password') }}</Label>
            <div class="relative">
              <Lock class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                id="password"
                v-model="password"
                :type="showPassword ? 'text' : 'password'"
                :placeholder="t('auth.password')"
                class="pl-10 pr-10"
              />
              <button
                type="button"
                class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                :aria-label="showPassword ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
                @click="showPassword = !showPassword"
              >
                <Eye v-if="!showPassword" class="w-4 h-4" />
                <EyeOff v-else class="w-4 h-4" />
              </button>
            </div>
          </div>

          <p v-if="error" class="text-center text-sm text-destructive">{{ error }}</p>

          <Button type="submit" class="w-full" :disabled="loading">
            <span v-if="loading">{{ t('common.loading') }}</span>
            <span v-else>{{ t('auth.login') }}</span>
          </Button>
        </form>
      </CardContent>
    </Card>
  </div>
</template>
