<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { request } from '@/api/request'
import type { BluetoothStatus } from '@/types/bluetooth'
defineProps<{ bluetooth: BluetoothStatus | null }>()
const emit = defineEmits<{ reconfigure: []; refresh: [] }>()
const { t } = useI18n()
const busy = ref(false), error = ref('')
async function action(action: string) {
  busy.value = true; error.value = ''
  try { await request('/hid/bluetooth', { method: 'POST', body: JSON.stringify({ action, seconds: 120 }) }) }
  catch (e) { error.value = e instanceof Error ? e.message : String(e) }
  finally { busy.value = false; emit('refresh') }
}
</script>
<template>
  <div class="space-y-3">
    <div class="flex flex-wrap gap-2">
      <Button size="sm" :disabled="busy || !bluetooth?.initialized || bluetooth?.connected" @click="action('pair')">{{ t('bluetoothHid.openPairing') }}</Button>
      <Button v-if="bluetooth?.pairing_seconds" size="sm" variant="outline" :disabled="busy" @click="action('close')">{{ t('bluetoothHid.closePairing') }}</Button>
      <Button size="sm" variant="outline" :disabled="busy || !bluetooth?.connected" @click="action('disconnect')">{{ t('bluetoothHid.disconnect') }}</Button>
      <Button size="sm" variant="outline" :disabled="busy" @click="emit('reconfigure')">{{ t('hidGuide.repair') }}</Button>
    </div>
    <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
  </div>
</template>
