<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, Loader2, Save } from 'lucide-vue-next'
import { authApi, type AuthConfig } from '@/api'
import { useAuthStore } from '@/stores/auth'
import { useConfigStore } from '@/stores/config'
import CardTitle from '@/components/SettingsCardTitle.vue'
import TotpSettingsCard from '@/components/TotpSettingsCard.vue'
import { Card, CardHeader, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
const props = defineProps<{ active: boolean }>()
const { t } = useI18n()
const authStore = useAuthStore()
const configStore = useConfigStore()
const usernameInput = ref('')
const usernamePassword = ref('')
const usernameSaving = ref(false)
const usernameSaved = ref(false)
const usernameError = ref('')
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordSaving = ref(false)
const passwordSaved = ref(false)
const passwordError = ref('')
const authConfig = ref<AuthConfig>({
  session_timeout_secs: 3600 * 24,
  single_user_allow_multiple_sessions: false,
})
const authConfigLoading = ref(false)

async function changeUsername() {
  usernameError.value = ''
  usernameSaved.value = false

  if (usernameInput.value.length < 2) {
    usernameError.value = t('auth.enterUsername')
    return
  }
  if (!usernamePassword.value) {
    usernameError.value = t('auth.enterPassword')
    return
  }

  usernameSaving.value = true
  try {
    await authApi.changeUsername(usernameInput.value, usernamePassword.value)
    usernameSaved.value = true
    usernamePassword.value = ''
    await authStore.checkAuth()
    usernameInput.value = authStore.user || usernameInput.value
    setTimeout(() => {
      usernameSaved.value = false
    }, 2000)
  } catch (e) {
    usernameError.value = t('auth.invalidPassword')
  } finally {
    usernameSaving.value = false
  }
}

async function changePassword() {
  passwordError.value = ''
  passwordSaved.value = false

  if (!currentPassword.value) {
    passwordError.value = t('auth.enterPassword')
    return
  }
  if (newPassword.value.length < 4) {
    passwordError.value = t('setup.passwordHint')
    return
  }
  if (newPassword.value !== confirmPassword.value) {
    passwordError.value = t('setup.passwordMismatch')
    return
  }

  passwordSaving.value = true
  try {
    await authApi.changePassword(currentPassword.value, newPassword.value)
    currentPassword.value = ''
    newPassword.value = ''
    confirmPassword.value = ''
    passwordSaved.value = true
    setTimeout(() => {
      passwordSaved.value = false
    }, 2000)
  } catch (e) {
    passwordError.value = t('auth.invalidPassword')
  } finally {
    passwordSaving.value = false
  }
}

async function loadAuthConfig() {
  authConfigLoading.value = true
  try {
    authConfig.value = await configStore.refreshAuth()
  } catch {
  } finally {
    authConfigLoading.value = false
  }
}

async function saveAuthConfig() {
  authConfigLoading.value = true
  try {
    authConfig.value = await configStore.updateAuth({
      single_user_allow_multiple_sessions: authConfig.value.single_user_allow_multiple_sessions,
    })
  } catch {
  } finally {
    authConfigLoading.value = false
  }
}

watch(() => props.active, async (active) => {
  if (!active) return
  usernameInput.value = authStore.user || ''
  await loadAuthConfig()
}, { immediate: true })
watch(
  () => authStore.user,
  (value) => {
    if (value) {
      usernameInput.value = value
    }
  },
)

</script>

<template>
<div v-show="active" class="space-y-4">
  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.usernameDesc')">{{
        t('settings.username')
      }}</CardTitle>
      <div class="settings-card-actions">
        <div class="settings-save-action">
          <Button @click="changeUsername" :disabled="usernameSaving">
            <Loader2 v-if="usernameSaving" class="size-4 mr-2 animate-spin" />
            <Save v-else class="size-4 mr-2" />
            {{ t('common.save') }}
          </Button>
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-field space-y-2">
        <Label for="account-username">{{ t('settings.username') }}</Label>
        <Input id="account-username" v-model="usernameInput" autocomplete="username" />
      </div>
      <div class="settings-field space-y-2">
        <Label for="account-username-password">{{ t('settings.currentPassword') }}</Label>
        <Input
          id="account-username-password"
          v-model="usernamePassword"
          type="password"
          autocomplete="current-password"
        />
      </div>
      <p v-if="usernameError" class="text-xs text-destructive">{{ usernameError }}</p>
      <p v-else-if="usernameSaved" class="flex items-center gap-1.5 text-xs text-success">
        <Check class="size-3.5" />{{ t('common.success') }}
      </p>
    </CardContent>
  </Card>

  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.passwordDesc')">{{
        t('settings.changePassword')
      }}</CardTitle>
      <div class="settings-card-actions">
        <div class="settings-save-action">
          <Button @click="changePassword" :disabled="passwordSaving">
            <Loader2 v-if="passwordSaving" class="size-4 mr-2 animate-spin" />
            <Save v-else class="size-4 mr-2" />
            {{ t('common.save') }}
          </Button>
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-field space-y-2">
        <Label for="account-current-password">{{ t('settings.currentPassword') }}</Label>
        <Input
          id="account-current-password"
          v-model="currentPassword"
          type="password"
          autocomplete="current-password"
        />
      </div>
      <div class="settings-field space-y-2">
        <Label for="account-new-password">{{ t('settings.newPassword') }}</Label>
        <Input
          id="account-new-password"
          v-model="newPassword"
          type="password"
          autocomplete="new-password"
        />
      </div>
      <div class="settings-field space-y-2">
        <Label for="account-confirm-password">{{ t('auth.confirmPassword') }}</Label>
        <Input
          id="account-confirm-password"
          v-model="confirmPassword"
          type="password"
          autocomplete="new-password"
        />
      </div>
      <p v-if="passwordError" class="text-xs text-destructive">{{ passwordError }}</p>
      <p v-else-if="passwordSaved" class="flex items-center gap-1.5 text-xs text-success">
        <Check class="size-3.5" />{{ t('common.success') }}
      </p>
    </CardContent>
  </Card>

  <TotpSettingsCard />

  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.authSettingsDesc')">{{
        t('settings.authSettings')
      }}</CardTitle>
      <div class="settings-card-actions">
        <div class="settings-save-action">
          <Button @click="saveAuthConfig" :disabled="authConfigLoading">
            <Loader2 v-if="authConfigLoading" class="size-4 mr-2 animate-spin" />
            <Save v-else class="size-4 mr-2" />
            {{ t('common.save') }}
          </Button>
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-toggle flex items-start justify-between gap-4">
        <Label>{{ t('settings.allowMultipleSessions') }}</Label>
        <Switch
          v-model="authConfig.single_user_allow_multiple_sessions"
          :disabled="authConfigLoading"
        />
      </div>
    </CardContent>
  </Card>
</div>

</template>
