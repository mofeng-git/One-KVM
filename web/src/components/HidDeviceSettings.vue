<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useConfigStore } from '@/stores/config'
import { useHidConnection } from '@/composables/useHidConnection'
import { Button } from '@/components/ui/button'
import HidDeviceOverview from './HidDeviceOverview.vue'
import HidDriverDialog from './HidDriverDialog.vue'
import BluetoothHidSettings from './BluetoothHidSettings.vue'
const props = defineProps<{ active: boolean; dirty?: boolean }>()
const emit = defineEmits<{ applied: []; discard: [] }>()
const { t } = useI18n(), store = useConfigStore(), open = ref(false)
const active = computed(() => props.active && !open.value)
const backend = computed(() => store.hid?.backend)
const { status, bluetooth, error, restart } = useHidConnection(active, backend)
</script>
<template>
  <section class="rounded-lg border p-5 space-y-4">
    <h3 class="font-semibold">{{ t('hidGuide.deviceTitle') }}</h3>
    <HidDeviceOverview :hid="store.hid" :status="status" :bluetooth="bluetooth" :error="error" />
    <Button variant="outline" @click="open = true">{{ t(store.hid ? 'hidGuide.reconfigure' : 'hidGuide.configure') }}</Button>
  </section>
  <section v-if="store.hid?.backend === 'bluetooth'" class="rounded-lg border p-5 space-y-4">
    <h3 class="font-semibold">{{ t('hidGuide.features') }}</h3>
    <BluetoothHidSettings :bluetooth="bluetooth" @refresh="restart" @reconfigure="open = true" />
  </section>
  <HidDriverDialog v-if="open" :dirty="dirty" @close="open = false" @applied="emit('applied')" @discard="emit('discard')" />
</template>
