<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import QrcodeVue from 'qrcode.vue'
import { authApi } from '@/api'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { AlertTriangle, KeyRound, Loader2, ShieldCheck, ShieldOff } from 'lucide-vue-next'

const { t, locale } = useI18n()

const enabled = ref(false)
const statusLoading = ref(true)
const statusError = ref('')
const now = ref(Date.now())
const serverOffsetMs = ref(0)
const nextSyncAt = ref(0)

const enrollmentOpen = ref(false)
const enrollmentStep = ref<'password' | 'verify'>('password')
const enrollmentLoading = ref(false)
const enrollmentError = ref('')
const enrollmentPassword = ref('')
const enrollmentCode = ref('')
const enrollmentId = ref('')
const enrollmentSecret = ref('')
const enrollmentUri = ref('')
const enrollmentExpiresAt = ref(0)

const disableOpen = ref(false)
const disableLoading = ref(false)
const disableError = ref('')
const disablePassword = ref('')
const disableCode = ref('')

const serverTime = computed(() => formatDate(now.value + serverOffsetMs.value, true))
const localTime = computed(() => formatDate(now.value, false))
const clockOffsetSeconds = computed(() => Math.round(serverOffsetMs.value / 1000))
const clockDriftWarning = computed(() => Math.abs(serverOffsetMs.value) >= 30_000)

let timer: ReturnType<typeof setInterval> | undefined
let statusSyncing = false

function formatDate(timestamp: number, utc: boolean) {
  return new Intl.DateTimeFormat(locale.value, {
    dateStyle: 'medium',
    timeStyle: 'medium',
    ...(utc ? { timeZone: 'UTC' } : {}),
  }).format(new Date(timestamp))
}

function applyServerTime(serverTimeUnixMs: number, requestStartedAt: number) {
  const localMidpoint = Math.round((requestStartedAt + Date.now()) / 2)
  serverOffsetMs.value = serverTimeUnixMs - localMidpoint
  nextSyncAt.value = Date.now() + 60_000
}

async function loadStatus() {
  if (statusSyncing) return
  statusSyncing = true
  const startedAt = Date.now()
  try {
    const status = await authApi.totpStatus()
    enabled.value = status.enabled
    applyServerTime(status.server_time_unix_ms, startedAt)
    statusError.value = ''
  } catch {
    statusError.value = t('settings.totp.loadFailed')
    nextSyncAt.value = Date.now() + 60_000
  } finally {
    statusSyncing = false
    statusLoading.value = false
  }
}

function normalizeCode(value: string) {
  return value.replace(/\D/g, '').slice(0, 6)
}

function localizedError(error: unknown) {
  const raw = error instanceof Error ? error.message : ''
  if (raw.includes('Current password is incorrect')) return t('settings.totp.currentPasswordIncorrect')
  if (raw.includes('Invalid TOTP code')) return t('settings.totp.invalidCode')
  if (raw.includes('enrollment expired')) return t('settings.totp.enrollmentExpired')
  if (raw.includes('Too many attempts')) return t('settings.totp.rateLimited')
  if (raw.includes('already enabled')) return t('settings.totp.alreadyEnabled')
  if (raw.includes('not enabled')) return t('settings.totp.notEnabled')
  return raw || t('settings.totp.operationFailed')
}

async function beginEnrollment() {
  if (!enrollmentPassword.value) {
    enrollmentError.value = t('auth.enterPassword')
    return
  }
  enrollmentLoading.value = true
  enrollmentError.value = ''
  const startedAt = Date.now()
  try {
    const result = await authApi.beginTotpEnrollment(enrollmentPassword.value)
    enrollmentPassword.value = ''
    enrollmentId.value = result.enrollment_id
    enrollmentSecret.value = result.secret
    enrollmentUri.value = result.otpauth_uri
    enrollmentExpiresAt.value = result.expires_at_unix_ms
    applyServerTime(result.server_time_unix_ms, startedAt)
    enrollmentStep.value = 'verify'
  } catch (error) {
    enrollmentError.value = localizedError(error)
  } finally {
    enrollmentLoading.value = false
  }
}

async function confirmEnrollment() {
  enrollmentCode.value = normalizeCode(enrollmentCode.value)
  if (enrollmentCode.value.length !== 6) {
    enrollmentError.value = t('settings.totp.enterSixDigitCode')
    return
  }
  enrollmentLoading.value = true
  enrollmentError.value = ''
  try {
    await authApi.confirmTotpEnrollment(enrollmentId.value, enrollmentCode.value)
    enabled.value = true
    enrollmentOpen.value = false
  } catch (error) {
    enrollmentError.value = localizedError(error)
    enrollmentCode.value = ''
    if (error instanceof Error && error.message.includes('enrollment expired')) {
      clearEnrollmentSecret()
      enrollmentStep.value = 'password'
    }
  } finally {
    enrollmentLoading.value = false
  }
}

async function disableTotp() {
  disableCode.value = normalizeCode(disableCode.value)
  if (!disablePassword.value) {
    disableError.value = t('auth.enterPassword')
    return
  }
  if (disableCode.value.length !== 6) {
    disableError.value = t('settings.totp.enterSixDigitCode')
    return
  }
  disableLoading.value = true
  disableError.value = ''
  try {
    await authApi.disableTotp(disablePassword.value, disableCode.value)
    enabled.value = false
    disableOpen.value = false
  } catch (error) {
    disableError.value = localizedError(error)
    disableCode.value = ''
  } finally {
    disableLoading.value = false
  }
}

function clearEnrollmentSecret() {
  enrollmentId.value = ''
  enrollmentSecret.value = ''
  enrollmentUri.value = ''
  enrollmentExpiresAt.value = 0
  enrollmentCode.value = ''
}

function resetEnrollment() {
  clearEnrollmentSecret()
  enrollmentPassword.value = ''
  enrollmentError.value = ''
  enrollmentStep.value = 'password'
}

function resetDisable() {
  disablePassword.value = ''
  disableCode.value = ''
  disableError.value = ''
}

watch(enrollmentOpen, (open) => {
  if (!open) resetEnrollment()
})
watch(disableOpen, (open) => {
  if (!open) resetDisable()
})

onMounted(async () => {
  await loadStatus()
  timer = setInterval(() => {
    now.value = Date.now()
    if (enrollmentId.value && now.value >= enrollmentExpiresAt.value) {
      clearEnrollmentSecret()
      enrollmentStep.value = 'password'
      enrollmentError.value = t('settings.totp.enrollmentExpired')
    }
    if (now.value >= nextSyncAt.value) void loadStatus()
  }, 1000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
  resetEnrollment()
  resetDisable()
})
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-start justify-between gap-4 space-y-0">
      <div class="space-y-1.5">
        <CardTitle>{{ t('settings.totp.title') }}</CardTitle>
        <CardDescription>{{ t('settings.totp.description') }}</CardDescription>
      </div>
      <Badge :variant="enabled ? 'default' : 'secondary'">
        {{ enabled ? t('common.enabled') : t('common.disabled') }}
      </Badge>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="grid gap-3 text-sm sm:grid-cols-2">
        <div class="space-y-1">
          <p class="text-xs text-muted-foreground">{{ t('settings.totp.serverTime') }}</p>
          <p class="font-mono">{{ serverTime }} UTC</p>
        </div>
        <div class="space-y-1">
          <p class="text-xs text-muted-foreground">{{ t('settings.totp.localTime') }}</p>
          <p class="font-mono">{{ localTime }}</p>
        </div>
      </div>
      <p class="text-xs text-muted-foreground">
        {{ t('settings.totp.clockOffset', { seconds: clockOffsetSeconds }) }}
      </p>
      <Alert v-if="clockDriftWarning" variant="destructive">
        <AlertTriangle />
        <AlertDescription>{{ t('settings.totp.clockDriftWarning') }}</AlertDescription>
      </Alert>
      <Alert v-if="statusError" variant="destructive">
        <AlertTriangle />
        <AlertDescription>{{ statusError }}</AlertDescription>
      </Alert>
    </CardContent>
    <CardFooter class="border-t pt-4 justify-end">
      <Button v-if="!enabled" :disabled="statusLoading" @click="enrollmentOpen = true">
        <ShieldCheck class="size-4" />
        {{ t('settings.totp.enable') }}
      </Button>
      <Button v-else variant="destructive" :disabled="statusLoading" @click="disableOpen = true">
        <ShieldOff class="size-4" />
        {{ t('settings.totp.disable') }}
      </Button>
    </CardFooter>
  </Card>

  <Dialog v-model:open="enrollmentOpen">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{ t('settings.totp.enableTitle') }}</DialogTitle>
      </DialogHeader>

      <div v-if="enrollmentStep === 'password'" class="space-y-4">
        <div class="space-y-2">
          <Label for="totp-enrollment-password">{{ t('settings.currentPassword') }}</Label>
          <Input id="totp-enrollment-password" v-model="enrollmentPassword" type="password" autocomplete="current-password" />
        </div>
      </div>
      <div v-else class="space-y-4">
        <div class="flex justify-center rounded-md border bg-white p-3">
          <QrcodeVue :value="enrollmentUri" :size="200" level="M" />
        </div>
        <div class="space-y-2">
          <Label>{{ t('settings.totp.manualSecret') }}</Label>
          <Input :model-value="enrollmentSecret" readonly class="font-mono" />
          <p class="text-xs text-muted-foreground">{{ t('settings.totp.secretOneTime') }}</p>
        </div>
        <div class="space-y-2">
          <Label for="totp-enrollment-code">{{ t('auth.totpCode') }}</Label>
          <div class="relative">
            <KeyRound class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input id="totp-enrollment-code" v-model="enrollmentCode" inputmode="numeric" autocomplete="one-time-code" maxlength="6" class="pl-10 font-mono" @input="enrollmentCode = normalizeCode(enrollmentCode)" />
          </div>
        </div>
      </div>

      <Alert v-if="enrollmentError" variant="destructive">
        <AlertTriangle />
        <AlertDescription>{{ enrollmentError }}</AlertDescription>
      </Alert>
      <DialogFooter>
        <Button variant="outline" :disabled="enrollmentLoading" @click="enrollmentOpen = false">{{ t('common.cancel') }}</Button>
        <Button v-if="enrollmentStep === 'password'" :disabled="enrollmentLoading" @click="beginEnrollment">
          <Loader2 v-if="enrollmentLoading" class="size-4 animate-spin" />
          {{ t('common.next') }}
        </Button>
        <Button v-else :disabled="enrollmentLoading" @click="confirmEnrollment">
          <Loader2 v-if="enrollmentLoading" class="size-4 animate-spin" />
          {{ t('common.confirm') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog v-model:open="disableOpen">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{ t('settings.totp.disableTitle') }}</DialogTitle>
      </DialogHeader>
      <div class="space-y-4">
        <div class="space-y-2">
          <Label for="totp-disable-password">{{ t('settings.currentPassword') }}</Label>
          <Input id="totp-disable-password" v-model="disablePassword" type="password" autocomplete="current-password" />
        </div>
        <div class="space-y-2">
          <Label for="totp-disable-code">{{ t('auth.totpCode') }}</Label>
          <Input id="totp-disable-code" v-model="disableCode" inputmode="numeric" autocomplete="one-time-code" maxlength="6" class="font-mono" @input="disableCode = normalizeCode(disableCode)" />
        </div>
        <Alert v-if="disableError" variant="destructive">
          <AlertTriangle />
          <AlertDescription>{{ disableError }}</AlertDescription>
        </Alert>
      </div>
      <DialogFooter>
        <Button variant="outline" :disabled="disableLoading" @click="disableOpen = false">{{ t('common.cancel') }}</Button>
        <Button variant="destructive" :disabled="disableLoading" @click="disableTotp">
          <Loader2 v-if="disableLoading" class="size-4 animate-spin" />
          {{ t('settings.totp.disable') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
