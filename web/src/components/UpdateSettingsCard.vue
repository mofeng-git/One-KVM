<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { RefreshCw, ExternalLink } from 'lucide-vue-next'
import { updateApi, type UpdateChannel, type UpdateOverviewResponse, type UpdateStatusResponse } from '@/api'
import { ApiError } from '@/api/request'
import { useAuthStore } from '@/stores/auth'
import { useSystemStore } from '@/stores/system'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Card, CardHeader, CardContent, CardFooter } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from '@/components/ui/select'

const { t } = useI18n()
const router = useRouter()
const authStore = useAuthStore()
const systemStore = useSystemStore()
const onlineUpgradeSupported = computed(() => systemStore.platform?.mode === 'linux')
const updateChannel = ref<UpdateChannel>('stable')
const updateOverview = ref<UpdateOverviewResponse | null>(null)
const updateStatus = ref<UpdateStatusResponse | null>(null)
const updateLoading = ref(false)
const updateSawRestarting = ref(false)
const updateSawRequestFailure = ref(false)
const updateAutoReloadTriggered = ref(false)
const updateRunning = computed(() => {
  const phase = updateStatus.value?.phase
  return (
    phase === 'checking' ||
    phase === 'downloading' ||
    phase === 'verifying' ||
    phase === 'installing' ||
    phase === 'restarting'
  )
})
let updateStatusTimer: number | null = null
let disposed = false
async function loadUpdateOverview() {
  updateLoading.value = true
  try {
    updateOverview.value = await updateApi.overview(updateChannel.value)
  } catch (e) {
    const message = e instanceof Error ? e.message : t('settings.updateOverviewLoadFailed')
    toast.error(t('settings.updateOverviewLoadFailed'), {
      description: message,
      duration: 4000,
    })
  } finally {
    updateLoading.value = false
  }
}

async function refreshUpdateStatus() {
  try {
    updateStatus.value = await updateApi.status()

    if (updateSawRestarting.value && !updateAutoReloadTriggered.value) {
      if (updateSawRequestFailure.value || updateStatus.value.phase === 'idle') {
        updateAutoReloadTriggered.value = true
        router.replace('/login')
      }
    }
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) {
      updateAutoReloadTriggered.value = true
      authStore.isAuthenticated = false
      authStore.user = null
      stopUpdatePolling()
      router.replace('/login')
      return
    }
    if (updateSawRestarting.value) {
      updateSawRequestFailure.value = true
    }
  }
}

function stopUpdatePolling() {
  if (updateStatusTimer !== null) {
    window.clearTimeout(updateStatusTimer)
    updateStatusTimer = null
  }
}

function startUpdatePolling() {
  if (disposed || updateStatusTimer !== null) return
  updateStatusTimer = window.setTimeout(async () => {
    updateStatusTimer = null
    if (disposed) return
    await refreshUpdateStatus()
    if (updateStatus.value?.phase === 'restarting') {
      updateSawRestarting.value = true
    }
    if (disposed) return
    if (updateRunning.value) startUpdatePolling()
    else await loadUpdateOverview()
  }, 1000)
}

async function startOnlineUpgrade() {
  if (!onlineUpgradeSupported.value) return
  try {
    updateSawRestarting.value = false
    updateSawRequestFailure.value = false
    updateAutoReloadTriggered.value = false
    await updateApi.upgrade({ channel: updateChannel.value })
    await refreshUpdateStatus()
    startUpdatePolling()
  } catch {}
}

async function cancelUpgrade() {
  try { await updateApi.cancel(); await refreshUpdateStatus() } catch {}
}

function updatePhaseText(phase?: string): string {
  switch (phase) {
    case 'idle':
      return t('settings.updatePhaseIdle')
    case 'checking':
      return t('settings.updatePhaseChecking')
    case 'downloading':
      return t('settings.updatePhaseDownloading')
    case 'verifying':
      return t('settings.updatePhaseVerifying')
    case 'installing':
      return t('settings.updatePhaseInstalling')
    case 'restarting':
      return t('settings.updatePhaseRestarting')
    case 'success':
      return t('settings.updatePhaseSuccess')
    case 'failed':
      return t('settings.updatePhaseFailed')
    default:
      return t('common.unknown')
  }
}

function localizeUpdateMessage(message?: string): string | null {
  if (!message) return null

  if (message === 'Checking for updates') return t('settings.updateMsgChecking')
  if (message.startsWith('Downloading binary')) {
    return message.replace('Downloading binary', t('settings.updateMsgDownloading'))
  }
  if (message === 'Verifying sha256') return t('settings.updateMsgVerifying')
  if (message === 'Replacing binary') return t('settings.updateMsgInstalling')
  if (message === 'Restarting service') return t('settings.updateMsgRestarting')

  return message
}

function updateStatusBadgeText(): string {
  return (
    localizeUpdateMessage(updateStatus.value?.message) || updatePhaseText(updateStatus.value?.phase)
  )
}

onMounted(async () => {
  await Promise.all([loadUpdateOverview(), refreshUpdateStatus()])
  if (updateRunning.value) startUpdatePolling()
})
onUnmounted(() => { disposed = true; stopUpdatePolling() })
watch(updateChannel, loadUpdateOverview)
</script>

<template>
<Card class="update-card">
  <CardHeader
    ><CardTitle :help="t('settings.onlineUpgradeDesc')">{{
      t('settings.onlineUpgrade')
    }}</CardTitle></CardHeader
  >
  <CardContent class="space-y-5">
    <Button v-if="['checking', 'downloading', 'verifying'].includes(updateStatus?.phase ?? '')" variant="outline" @click="cancelUpgrade">{{ t('common.cancel') }}</Button>
    <div class="settings-field">
      <Label>{{ t('settings.updateChannel') }}</Label
      ><Select v-model="updateChannel" :disabled="updateRunning"
        ><SelectTrigger class="w-full"><SelectValue /></SelectTrigger
        ><SelectContent
          ><SelectItem value="stable">Stable</SelectItem
          ><SelectItem value="beta">Beta</SelectItem></SelectContent
        ></Select
      >
    </div>
    <div class="settings-field">
      <Label>{{ t('settings.latestVersion') }}</Label>
      <div class="flex min-w-0 items-center justify-between gap-4">
        <span class="text-sm text-muted-foreground">{{
          updateOverview?.latest_version || t('common.unknown')
        }}</span
        ><Button
          variant="outline"
          :disabled="updateRunning || updateLoading"
          @click="loadUpdateOverview"
          ><RefreshCw class="size-4" :class="{ 'animate-spin': updateLoading }" />{{
            t('settingsUi.checkUpdates')
          }}</Button
        >
      </div>
    </div>
    <div
      v-if="
        updateRunning ||
        updateStatus?.phase === 'failed' ||
        updateStatus?.phase === 'success'
      "
      class="space-y-2"
      role="status"
    >
      <p class="text-sm">{{ updateStatusBadgeText() }}</p>
      <div class="h-2 overflow-hidden rounded bg-muted">
        <div
          class="h-full bg-primary transition-all"
          :style="{
            width: `${Math.max(0, Math.min(100, updateStatus?.progress || 0))}%`,
          }"
        />
      </div>
      <p v-if="updateStatus?.last_error" class="text-sm text-destructive">
        {{ updateStatus.last_error }}
      </p>
    </div>
    <section class="update-release-notes">
      <div class="flex items-center justify-between gap-4">
        <h3 class="text-sm font-semibold">{{ t('settings.releaseNotes') }}</h3>
        <a
          href="https://github.com/mofeng-git/One-KVM/releases"
          target="_blank"
          rel="noopener noreferrer"
          class="inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:underline"
          >{{ t('settingsUi.project') }}<ExternalLink class="size-3.5"
        /></a>
      </div>
      <p v-if="updateLoading" class="text-sm text-muted-foreground">
        {{ t('common.loading') }}
      </p>
      <p
        v-else-if="!updateOverview?.notes_between?.length"
        class="text-sm text-muted-foreground"
      >
        {{ t('settings.noUpdates') }}
      </p>
      <div v-else class="max-h-80 space-y-4 overflow-y-auto">
        <article
          v-for="item in updateOverview.notes_between"
          :key="item.version"
          class="space-y-2 border-b pb-4 last:border-0"
        >
          <div class="flex items-center justify-between gap-4">
            <span class="text-sm font-medium">v{{ item.version }}</span
            ><span class="text-sm text-muted-foreground">{{ item.published_at }}</span>
          </div>
          <ul class="list-disc space-y-1 pl-5 text-sm">
            <li v-for="(note, idx) in item.notes" :key="idx">{{ note }}</li>
          </ul>
        </article>
      </div>
    </section>
    <p v-if="!onlineUpgradeSupported" class="text-sm text-muted-foreground">
      {{ t('settings.onlineUpgradeUnsupported') }}
    </p>
  </CardContent>
  <CardFooter v-if="onlineUpgradeSupported" class="justify-end border-t"
    ><Button
      :disabled="updateRunning || !updateOverview?.upgrade_available"
      @click="startOnlineUpgrade"
      ><RefreshCw v-if="updateRunning" class="size-4 animate-spin" />{{
        t('settings.startUpgrade')
      }}</Button
    ></CardFooter
  >
</Card>
</template>
