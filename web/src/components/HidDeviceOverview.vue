<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { HidConfig } from '@/types/generated'
import type { BluetoothStatus } from '@/types/bluetooth'
import { hidDeviceError, hidDeviceStage, otgDeviceStage, usesCh9329, supportsOtgFunctions, type HidDeviceStatus } from '@/lib/hidGuide'
const props = defineProps<{ hid: HidConfig | null; status?: HidDeviceStatus | null; bluetooth?: BluetoothStatus | null; error?: string }>()
const { t } = useI18n()
const connectionError = computed(() => props.error || hidDeviceError(props.status) || props.bluetooth?.error)
const stage = computed(() => hidDeviceStage(props.status, props.bluetooth ?? null, props.error))
</script>
<template>
  <div class="space-y-2 text-sm">
    <p class="font-medium">{{ !hid ? t('hidGuide.unconfigured') : hid.backend === 'none' ? t('hidGuide.disabled') : t(`hidGuide.driver_${hid.backend}`) }}</p>
    <dl v-if="hid && hid.backend !== 'none'" class="space-y-1 break-words">
      <div v-if="supportsOtgFunctions(hid.backend)"><dt class="inline text-muted-foreground">UDC: </dt><dd class="inline">{{ hid.otg_udc || status?.backend === 'otg' && t('hidGuide.legacyAuto') || '—' }}</dd></div>
      <template v-if="usesCh9329(hid.backend)">
        <div><dt class="inline text-muted-foreground">{{ t('hidGuide.device_ch9329') }}: </dt><dd class="inline">{{ hid.ch9329_port || '—' }}</dd></div>
        <div><dt class="inline text-muted-foreground">{{ t('actionbar.baudrate') }}: </dt><dd class="inline">{{ hid.ch9329_baudrate }}</dd></div>
      </template>
      <template v-if="hid.backend === 'bluetooth'">
        <div><dt class="inline text-muted-foreground">{{ t('bluetoothHid.adapter') }}: </dt><dd class="inline">{{ hid.bluetooth.adapter }} <span v-if="bluetooth?.adapter_address">· {{ bluetooth.adapter_address }}</span></dd></div>
        <div><dt class="inline text-muted-foreground">{{ t('bluetoothHid.name') }}: </dt><dd class="inline">{{ hid.bluetooth.name }}</dd></div>
        <div v-if="bluetooth?.peer"><dt class="inline text-muted-foreground">{{ t('hidGuide.host') }}: </dt><dd class="inline">{{ bluetooth.devices.find(d => d.address === bluetooth?.peer)?.name }} · {{ bluetooth.peer }}</dd></div>
      </template>
    </dl>
    <p v-if="hid && hid.backend !== 'none'" role="status" :class="!connectionError && stage === 'ready' ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'">
      <span v-if="hid.backend === 'ch9329_otg'">{{ t('hidGuide.keyboardChannel') }}: </span>{{ t(`hidGuide.${stage}`) }}
      <span v-if="bluetooth?.pairing_seconds"> · {{ bluetooth.pairing_seconds }}s</span>
    </p>
    <template v-if="hid?.backend === 'ch9329_otg'">
      <p role="status" class="text-muted-foreground">{{ t('hidGuide.otgChannel') }}: {{ t(`hidGuide.${otgDeviceStage(status?.otg)}`) }}</p>
      <p v-if="status?.otg?.error" role="alert" class="text-destructive break-words">{{ status.otg.error }}</p>
    </template>
    <p v-if="connectionError" role="alert" class="text-destructive break-words">{{ connectionError }}</p>
  </div>
</template>
