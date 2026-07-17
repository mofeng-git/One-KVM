<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field'
import LanguageToggleButton from '@/components/LanguageToggleButton.vue'
import BrandMark from '@/components/BrandMark.vue'
import { AlertCircle, ArrowLeft, KeyRound, Lock, Eye, EyeOff, User, CircleHelp } from 'lucide-vue-next'

const { t } = useI18n()
const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()

/** Map backend English messages to locale strings (API returns fixed English copy). */
function localizedLoginError(raw: string | null): string {
  if (!raw) return t('auth.loginFailed')
  if (raw.includes('Invalid username or password')) return t('auth.invalidPassword')
  if (raw.includes('System not initialized')) return t('auth.systemNotInitialized')
  if (raw.includes('Invalid TOTP code')) return t('auth.invalidTotpCode')
  if (raw.includes('challenge expired')) return t('auth.totpChallengeExpired')
  if (raw.includes('Too many attempts')) return t('auth.totpRateLimited')
  return raw
}

const username = ref('')
const password = ref('')
const showPassword = ref(false)
const loading = ref(false)
const error = ref('')
const step = ref<'password' | 'totp'>('password')
const challengeId = ref('')
const challengeExpiresAt = ref(0)
const totpCode = ref('')

function focusTotpInput() {
  document.querySelector<HTMLInputElement>('#totp-code')?.focus()
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

  const result = await authStore.beginLogin(username.value, password.value)

  if (result?.next === 'authenticated') {
    const redirect = route.query.redirect as string
    router.push(redirect || '/')
  } else if (result?.next === 'totp' && result.challenge_id && result.expires_at_unix_ms) {
    password.value = ''
    challengeId.value = result.challenge_id
    challengeExpiresAt.value = result.expires_at_unix_ms
    step.value = 'totp'
    await nextTick()
    focusTotpInput()
  } else {
    error.value = localizedLoginError(authStore.error)
  }

  loading.value = false
}

function normalizeTotpCode() {
  totpCode.value = totpCode.value.replace(/\D/g, '').slice(0, 6)
}

async function handleTotpLogin() {
  normalizeTotpCode()
  if (Date.now() >= challengeExpiresAt.value) {
    error.value = t('auth.totpChallengeExpired')
    returnToPassword(true)
    return
  }
  if (totpCode.value.length !== 6) {
    error.value = t('auth.enterTotpCode')
    return
  }
  loading.value = true
  error.value = ''
  const success = await authStore.completeTotpLogin(challengeId.value, totpCode.value)
  if (success) {
    const redirect = route.query.redirect as string
    router.push(redirect || '/')
  } else {
    error.value = localizedLoginError(authStore.error)
    if (authStore.error?.includes('challenge expired') || authStore.error?.includes('Too many attempts')) {
      returnToPassword(true)
    } else {
      totpCode.value = ''
      await nextTick()
      focusTotpInput()
    }
  }
  loading.value = false
}

function returnToPassword(keepError = false) {
  step.value = 'password'
  challengeId.value = ''
  challengeExpiresAt.value = 0
  totpCode.value = ''
  authStore.cancelPendingLogin()
  if (!keepError) error.value = ''
}
</script>

<template>
  <div class="min-h-screen min-h-dvh flex items-center justify-center bg-background p-4">
    <Card class="relative w-full max-w-sm">
      <div class="absolute top-4 right-4">
        <LanguageToggleButton />
      </div>

      <CardHeader class="space-y-2 pt-10 text-center sm:pt-12">
        <div class="mx-auto flex justify-center">
          <BrandMark size="xl" />
        </div>
        <CardTitle class="text-xl sm:text-2xl">One-KVM</CardTitle>
        <CardDescription>{{ step === 'password' ? t('auth.login') : t('auth.totpPrompt') }}</CardDescription>
      </CardHeader>

      <CardContent>
        <form v-if="step === 'password'" @submit.prevent="handleLogin">
          <FieldGroup>
          <Field>
            <FieldLabel for="username">{{ t('auth.username') }}</FieldLabel>
            <div class="relative">
              <User class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                id="username"
                v-model="username"
                type="text"
                autocomplete="username"
                :placeholder="t('auth.username')"
                class="pl-10"
              />
            </div>
          </Field>

          <Field>
            <div class="flex items-center justify-between gap-2">
              <FieldLabel for="password">{{ t('auth.password') }}</FieldLabel>
              <Popover>
                <PopoverTrigger as-child>
                  <Button
                    type="button"
                    variant="link"
                    size="sm"
                    class="h-auto gap-1 p-0 text-xs text-muted-foreground"
                  >
                    {{ t('auth.forgotPassword') }}
                    <CircleHelp class="h-3.5 w-3.5" />
                  </Button>
                </PopoverTrigger>
                <PopoverContent class="w-80 p-3" align="end">
                  <p class="text-xs text-muted-foreground">
                    {{ t('auth.forgotPasswordHint') }}
                  </p>
                </PopoverContent>
              </Popover>
            </div>
            <div class="relative">
              <Lock class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                id="password"
                v-model="password"
                :type="showPassword ? 'text' : 'password'"
                autocomplete="current-password"
                :placeholder="t('auth.password')"
                class="pl-10 pr-10"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground"
                :aria-label="showPassword ? t('extensions.rustdesk.hidePassword') : t('extensions.rustdesk.showPassword')"
                @click="showPassword = !showPassword"
              >
                <Eye v-if="!showPassword" class="w-4 h-4" />
                <EyeOff v-else class="w-4 h-4" />
              </Button>
            </div>
          </Field>

          <Alert v-if="error" variant="destructive">
            <AlertCircle />
            <AlertDescription>{{ error }}</AlertDescription>
          </Alert>

          <Button type="submit" class="w-full" :disabled="loading">
            <span v-if="loading">{{ t('common.loading') }}</span>
            <span v-else>{{ t('auth.login') }}</span>
          </Button>

          </FieldGroup>
        </form>
        <form v-else @submit.prevent="handleTotpLogin">
          <FieldGroup>
            <Field>
              <FieldLabel for="totp-code">{{ t('auth.totpCode') }}</FieldLabel>
              <div class="relative">
                <KeyRound class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  id="totp-code"
                  v-model="totpCode"
                  type="text"
                  inputmode="numeric"
                  autocomplete="one-time-code"
                  maxlength="6"
                  class="pl-10 font-mono text-lg"
                  :placeholder="t('auth.totpCodePlaceholder')"
                  @input="normalizeTotpCode"
                />
              </div>
            </Field>

            <Alert v-if="error" variant="destructive">
              <AlertCircle />
              <AlertDescription>{{ error }}</AlertDescription>
            </Alert>

            <Button type="submit" class="w-full" :disabled="loading">
              <span v-if="loading">{{ t('common.loading') }}</span>
              <span v-else>{{ t('auth.verifyAndLogin') }}</span>
            </Button>
            <Button type="button" variant="ghost" class="w-full" :disabled="loading" @click="returnToPassword()">
              <ArrowLeft class="h-4 w-4" />
              {{ t('auth.backToPassword') }}
            </Button>
          </FieldGroup>
        </form>
      </CardContent>
    </Card>
  </div>
</template>
