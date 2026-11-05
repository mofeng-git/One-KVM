<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Power, RotateCw, Loader2 } from 'lucide-vue-next'
import { systemApi } from '@/api'
import { useSystemStore } from '@/stores/system'
import { Button } from '@/components/ui/button'
import { Card, CardHeader, CardContent } from '@/components/ui/card'
import CardTitle from './SettingsCardTitle.vue'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
} from '@/components/ui/alert-dialog'

type PowerAction = 'reboot' | 'poweroff'
const { t } = useI18n()
const system = useSystemStore()
const selected = ref<PowerAction>('reboot')
const dialogOpen = ref(false)
const accepted = ref<PowerAction | null>(null)
const busy = ref(false)
const error = ref('')
const actionLabel = computed(() =>
  t(selected.value === 'poweroff' ? 'settingsUi.poweroff' : 'settingsUi.reboot'),
)
function ask(action: PowerAction) {
  if (busy.value || accepted.value) return
  error.value = ''
  selected.value = action
  dialogOpen.value = true
}
function setOpen(open: boolean) {
  if (!busy.value) dialogOpen.value = open
}
async function confirm() {
  if (!dialogOpen.value || busy.value) return
  const action = selected.value
  busy.value = true
  error.value = ''
  try {
    await systemApi.hostPower(action)
    accepted.value = action
    dialogOpen.value = false
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : t('settingsUi.deviceActionFailed')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Card v-if="system.platform?.mode === 'linux'" class="system-device-controls">
    <CardHeader class="settings-card-heading">
      <CardTitle :help="t('settingsUi.deviceControlsHelp')">{{
        t('settingsUi.deviceControls')
      }}</CardTitle>
      <div class="settings-card-actions">
        <Button
          data-device-action="reboot"
          variant="outline"
          :disabled="busy || !!accepted"
          @click="ask('reboot')"
        >
          <RotateCw class="size-4" />{{ t('settingsUi.reboot') }}
        </Button>
        <Button
          data-device-action="poweroff"
          variant="outline"
          class="text-destructive hover:text-destructive"
          :disabled="busy || !!accepted"
          @click="ask('poweroff')"
        >
          <Power class="size-4" />{{ t('settingsUi.poweroff') }}
        </Button>
      </div>
    </CardHeader>
    <CardContent>
      <p class="text-sm text-muted-foreground">{{ t('settingsUi.deviceControlsHelp') }}</p>
      <p v-if="accepted" role="status" class="mt-2 text-sm">
        {{ t(accepted === 'reboot' ? 'settingsUi.rebootSent' : 'settingsUi.poweroffSent') }}
      </p>
    </CardContent>
    <AlertDialog :open="dialogOpen" @update:open="setOpen">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{{
            t(selected === 'poweroff' ? 'settingsUi.poweroffConfirm' : 'settingsUi.rebootConfirm')
          }}</AlertDialogTitle>
          <AlertDialogDescription>{{
            t(
              selected === 'poweroff'
                ? 'settingsUi.poweroffDescription'
                : 'settingsUi.rebootDescription',
            )
          }}</AlertDialogDescription>
        </AlertDialogHeader>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <AlertDialogFooter>
          <AlertDialogCancel :disabled="busy">{{ t('common.cancel') }}</AlertDialogCancel>
          <Button
            data-confirm-device-action
            :variant="selected === 'poweroff' ? 'destructive' : 'default'"
            :disabled="busy"
            @click="confirm"
          >
            <Loader2 v-if="busy" class="size-4 animate-spin" />{{ actionLabel }}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </Card>
</template>
