<script setup lang="ts">
import { computed, ref, watch, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { configApi } from '@/api'
import { request } from '@/api/request'
import type { BluetoothAdapter } from '@/types/bluetooth'
import { selectDevice, validName, type HidSelection, type Driver } from '@/lib/hidGuide'
import HidWiringDiagram from './HidWiringDiagram.vue'
const props = defineProps<{ modelValue: HidSelection; locked?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [HidSelection]; valid: [boolean] }>()
const { t } = useI18n()
const options = ref<Array<{ value: string; label: string }>>([])
const loading = ref(false), error = ref(''), missing = ref('')
let generation = 0
const drivers: Driver[] = ['otg', 'ch9329', 'bluetooth', 'none']
const selected = computed(() => props.modelValue.backend === 'otg' ? props.modelValue.otg_udc : props.modelValue.backend === 'ch9329' ? props.modelValue.ch9329_port : props.modelValue.bluetooth.adapter)
function device(value: string) {
  const draft = { ...props.modelValue, bluetooth: { ...props.modelValue.bluetooth } }
  if (draft.backend === 'otg') draft.otg_udc = value
  if (draft.backend === 'ch9329') draft.ch9329_port = value
  if (draft.backend === 'bluetooth') draft.bluetooth.adapter = value
  emit('update:modelValue', draft)
}
async function refresh() {
  const own = ++generation
  options.value = []; error.value = ''; missing.value = ''; loading.value = true
  try {
    if (props.modelValue.backend === 'none') return
    let next: typeof options.value
    if (props.modelValue.backend === 'bluetooth') {
      next = (await request<BluetoothAdapter[]>('/hid/bluetooth/adapters')).map(a => ({ value: a.name, label: `${a.name} · ${a.address}` }))
    } else {
      const devices = await configApi.listDevices()
      next = props.modelValue.backend === 'otg' ? devices.udc.map(d => ({ value: d.name, label: d.name })) : devices.serial.map(d => ({ value: d.path, label: `${d.name} · ${d.path}` }))
    }
    if (own !== generation) return
    options.value = next
    if (selected.value && !next.some(d => d.value === selected.value)) missing.value = t('hidGuide.missingDevice', { device: selected.value })
    device(selectDevice(selected.value, next.map(d => d.value)))
    if (!next.length) error.value = t('hidGuide.noDevices')
  } catch (e) { if (own === generation) error.value = e instanceof Error ? e.message : String(e) }
  finally { if (own === generation) loading.value = false }
}
const valid = computed(() => props.modelValue.backend === 'none' || (!loading.value && !error.value && options.value.some(d => d.value === selected.value)
  && (props.modelValue.backend !== 'bluetooth' || validName(props.modelValue.bluetooth.name))
  && (props.modelValue.backend !== 'ch9329' || [9600, 19200, 38400, 57600, 115200].includes(props.modelValue.ch9329_baudrate))))
watch(valid, value => emit('valid', value), { immediate: true })
watch(() => props.modelValue.backend, refresh, { immediate: true })
onUnmounted(() => generation++)
</script>
<template>
  <fieldset :disabled="locked" class="space-y-4 min-w-0">
    <label class="block space-y-1 text-sm">
      <span>{{ t('hidGuide.driver') }}</span>
      <select class="w-full rounded-md border bg-background px-3 py-2" :value="modelValue.backend" @change="emit('update:modelValue', { ...modelValue, backend: ($event.target as HTMLSelectElement).value as Driver })">
        <option v-for="driver in drivers" :key="driver" :value="driver">{{ t(`hidGuide.driver_${driver}`) }}</option>
      </select>
    </label>
    <HidWiringDiagram :backend="modelValue.backend" />
    <label v-if="modelValue.backend !== 'none'" class="block space-y-1 text-sm">
      <span>{{ t(`hidGuide.device_${modelValue.backend}`) }}</span>
      <select class="w-full rounded-md border bg-background px-3 py-2" :value="selected" :disabled="loading" @change="device(($event.target as HTMLSelectElement).value)">
        <option value="" disabled>{{ t('hidGuide.selectDevice') }}</option>
        <option v-for="option in options" :key="option.value" :value="option.value">{{ option.label }}</option>
      </select>
    </label>
    <label v-if="modelValue.backend === 'ch9329'" class="block space-y-1 text-sm">
      <span>{{ t('actionbar.baudrate') }}</span>
      <select class="w-full rounded-md border bg-background px-3 py-2" :value="modelValue.ch9329_baudrate" @change="emit('update:modelValue', { ...modelValue, ch9329_baudrate: Number(($event.target as HTMLSelectElement).value) })">
        <option v-for="rate in [9600, 19200, 38400, 57600, 115200]" :key="rate">{{ rate }}</option>
      </select>
    </label>
    <label v-if="modelValue.backend === 'bluetooth'" class="block space-y-1 text-sm">
      <span>{{ t('bluetoothHid.name') }}</span>
      <Input :model-value="modelValue.bluetooth.name" @update:model-value="emit('update:modelValue', { ...modelValue, bluetooth: { ...modelValue.bluetooth, name: String($event) } })" />
      <span v-if="!validName(modelValue.bluetooth.name)" class="text-destructive text-xs">{{ t('hidGuide.nameInvalid') }}</span>
    </label>
    <p v-if="missing" class="text-sm text-warning">{{ missing }}</p>
    <p v-if="error" role="alert" class="text-sm text-destructive break-words">{{ error }}</p>
    <Button v-if="modelValue.backend !== 'none'" type="button" variant="outline" size="sm" :disabled="loading" @click="refresh">{{ t('common.refresh') }}</Button>
  </fieldset>
</template>
