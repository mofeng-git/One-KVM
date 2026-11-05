<script setup lang="ts">
import { computed, ref, watch, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { HoverCard, HoverCardContent, HoverCardTrigger } from '@/components/ui/hover-card'
import { Cable, RefreshCw } from 'lucide-vue-next'
import { configApi, type PlatformCapabilities } from '@/api'
import { request } from '@/api/request'
import type { BluetoothAdapter } from '@/types/bluetooth'
import { selectDevice, validName, usesCh9329, supportsOtgFunctions, type HidSelection, type Driver } from '@/lib/hidGuide'
import HidWiringDiagram from './HidWiringDiagram.vue'
const props = withDefaults(defineProps<{ modelValue: HidSelection; platform: PlatformCapabilities | null; locked?: boolean; deviceOnly?: boolean; idPrefix?: string }>(), { idPrefix: 'hid' })
const emit = defineEmits<{ 'update:modelValue': [HidSelection]; valid: [boolean] }>()
const { t } = useI18n()
const EMPTY_SELECT_VALUE = '__one-kvm-empty-select-value__'
type DeviceField = 'serial' | 'udc' | 'bluetooth'
type DeviceOption = { value: string; label: string }
const options = ref<Record<DeviceField, DeviceOption[]>>({ serial: [], udc: [], bluetooth: [] })
const loading = ref(false), error = ref('')
let generation = 0
const drivers = computed(() => (['otg', 'ch9329', 'ch9329_otg', 'bluetooth', 'none'] as Driver[])
  .filter(driver => props.platform?.hid.backends.includes(driver)))
const fields = computed<DeviceField[]>(() => {
  if (props.modelValue.backend === 'bluetooth') return ['bluetooth']
  const result: DeviceField[] = []
  if (usesCh9329(props.modelValue.backend)) result.push('serial')
  if (supportsOtgFunctions(props.modelValue.backend)) result.push('udc')
  return result
})
function selected(field: DeviceField): string {
  return field === 'serial' ? props.modelValue.ch9329_port : field === 'udc' ? props.modelValue.otg_udc : props.modelValue.bluetooth.adapter
}
function deviceLabel(field: DeviceField): string {
  return options.value[field].find(option => option.value === selected(field))?.label
    || selected(field) || t('hidGuide.selectDevice')
}
function assign(draft: HidSelection, field: DeviceField, value: string) {
  if (field === 'serial') draft.ch9329_port = value
  if (field === 'udc') draft.otg_udc = value
  if (field === 'bluetooth') draft.bluetooth.adapter = value
}
function device(field: DeviceField, value: string) {
  if (value === selected(field)) return
  const draft = { ...props.modelValue, bluetooth: { ...props.modelValue.bluetooth } }
  assign(draft, field, value)
  emit('update:modelValue', draft)
}
function fieldLabel(field: DeviceField): string {
  return field === 'udc' && props.deviceOnly ? t('hidPage.udcLabel') : t(`hidGuide.device_${field === 'serial' ? 'ch9329' : field === 'udc' ? 'otg' : 'bluetooth'}`)
}
async function refresh() {
  const own = ++generation
  options.value = { serial: [], udc: [], bluetooth: [] }; error.value = ''; loading.value = true
  try {
    if (!drivers.value.includes(props.modelValue.backend) || props.modelValue.backend === 'none') return
    const next: Record<DeviceField, DeviceOption[]> = { serial: [], udc: [], bluetooth: [] }
    if (props.modelValue.backend === 'bluetooth') {
      next.bluetooth = (await request<BluetoothAdapter[]>('/hid/bluetooth/adapters')).map(a => ({ value: a.name, label: `${a.name} · ${a.address}` }))
    } else {
      const devices = await configApi.listDevices()
      next.udc = devices.udc.map(d => ({ value: d.name, label: d.name }))
      next.serial = devices.serial.map(d => ({ value: d.path, label: `${d.name} · ${d.path}` }))
    }
    if (own !== generation) return
    options.value = next
    const draft = { ...props.modelValue, bluetooth: { ...props.modelValue.bluetooth } }
    for (const field of fields.value) assign(draft, field, selectDevice(selected(field), next[field].map(d => d.value)))
    if (JSON.stringify(draft) !== JSON.stringify(props.modelValue)) emit('update:modelValue', draft)
  } catch (e) { if (own === generation) error.value = e instanceof Error ? e.message : String(e) }
  finally { if (own === generation) loading.value = false }
}
const valid = computed(() => drivers.value.includes(props.modelValue.backend) && (props.modelValue.backend === 'none' || (!loading.value && !error.value
  && fields.value.every(field => options.value[field].some(d => d.value === selected(field)))
  && (props.modelValue.backend !== 'bluetooth' || validName(props.modelValue.bluetooth.name))
  && (!usesCh9329(props.modelValue.backend) || [9600, 19200, 38400, 57600, 115200].includes(props.modelValue.ch9329_baudrate)))))
watch(valid, value => emit('valid', value), { immediate: true })
watch([() => props.modelValue.backend, () => drivers.value.includes(props.modelValue.backend)], refresh, { immediate: true })
onUnmounted(() => generation++)
defineExpose({ refresh, loading })
</script>
<template>
  <fieldset :disabled="loading || locked" class="space-y-4 min-w-0">
    <div v-if="!deviceOnly" class="space-y-1 text-sm">
      <div class="flex min-h-8 items-center justify-between gap-2">
        <label :for="`${idPrefix}-driver`">{{ t('hidGuide.driver') }}</label>
        <div class="flex items-center gap-1">
          <HoverCard v-if="modelValue.backend !== 'none'" :open-delay="150" :close-delay="100">
            <HoverCardTrigger as-child>
              <Button type="button" variant="ghost" size="sm" class="h-8 gap-1.5 px-2 text-muted-foreground" :aria-label="t('hidGuide.wiringHelp')">
                <Cable class="size-4" />
                <span>{{ t('hidGuide.wiringHelp') }}</span>
              </Button>
            </HoverCardTrigger>
            <HoverCardContent side="right" align="start" class="w-[min(480px,calc(100vw-2rem))] p-3">
              <HidWiringDiagram :backend="modelValue.backend" />
            </HoverCardContent>
          </HoverCard>
          <Button
            v-if="modelValue.backend !== 'none'"
            type="button"
            variant="ghost"
            size="icon-sm"
            :disabled="loading || locked"
            :aria-label="t('hidGuide.refreshDevices')"
            :title="t('hidGuide.refreshDevices')"
            @click="refresh"
          >
            <RefreshCw class="size-4" :class="{ 'animate-spin': loading }" />
          </Button>
        </div>
      </div>
      <Select
        :model-value="modelValue.backend"
        :disabled="loading || locked"
        @update:model-value="value => emit('update:modelValue', { ...modelValue, backend: String(value) as Driver })"
      >
        <SelectTrigger :id="`${idPrefix}-driver`" class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="driver in drivers" :key="driver" :value="driver">
            {{ t(`hidGuide.driver_${driver}`) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
    <p v-if="modelValue.backend === 'none'" class="text-sm text-muted-foreground">{{ t('hidGuide.disabledHelp') }}</p>
    <p v-if="!deviceOnly && modelValue.backend === 'ch9329_otg'" class="text-sm text-muted-foreground">{{ t('hidGuide.compositeHelp') }}</p>
    <div v-for="field in fields" :key="field" class="grid min-w-0 gap-4"
      :class="{ 'grid-cols-[minmax(0,1fr)_7rem]': field === 'serial' }">
      <div class="min-w-0 space-y-1 text-sm">
        <label :for="`${idPrefix}-device-${field}`">{{ fieldLabel(field) }}</label>
        <Select :model-value="selected(field) || EMPTY_SELECT_VALUE" :disabled="loading || locked"
          @update:model-value="value => device(field, value === EMPTY_SELECT_VALUE ? '' : String(value))">
          <SelectTrigger :id="`${idPrefix}-device-${field}`" class="w-full">
            <SelectValue :placeholder="t('hidGuide.selectDevice')">{{ deviceLabel(field) }}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="EMPTY_SELECT_VALUE" disabled>{{ t('hidGuide.selectDevice') }}</SelectItem>
            <SelectItem v-if="selected(field) && !options[field].some(d => d.value === selected(field))" :value="selected(field)" disabled>{{ selected(field) }}</SelectItem>
            <SelectItem v-for="option in options[field]" :key="option.value" :value="option.value">{{ option.label }}</SelectItem>
          </SelectContent>
        </Select>
        <p v-if="!loading && selected(field) && !options[field].some(d => d.value === selected(field))" class="text-warning">{{ t('hidGuide.missingDevice', { device: selected(field) }) }}</p>
        <p v-if="!loading && !options[field].length" class="text-destructive">{{ fieldLabel(field) }}: {{ t('hidGuide.noDevices') }}</p>
      </div>
      <div v-if="field === 'serial'" class="min-w-0 space-y-1 text-sm">
        <label :for="`${idPrefix}-baudrate`">{{ t('actionbar.baudrate') }}</label>
        <Select
          :model-value="String(modelValue.ch9329_baudrate)"
          :disabled="loading || locked"
          @update:model-value="value => emit('update:modelValue', { ...modelValue, ch9329_baudrate: Number(value) })"
        >
          <SelectTrigger :id="`${idPrefix}-baudrate`" class="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="rate in [9600, 19200, 38400, 57600, 115200]" :key="rate" :value="String(rate)">
              {{ rate }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
    <label v-if="modelValue.backend === 'bluetooth'" :for="`${idPrefix}-bluetooth-name`" class="block space-y-1 text-sm">
      <span>{{ t('bluetoothHid.name') }}</span>
      <Input :id="`${idPrefix}-bluetooth-name`" :disabled="loading || locked" :model-value="modelValue.bluetooth.name" @update:model-value="emit('update:modelValue', { ...modelValue, bluetooth: { ...modelValue.bluetooth, name: String($event) } })" />
      <span v-if="!validName(modelValue.bluetooth.name)" class="text-destructive text-xs">{{ t('hidGuide.nameInvalid') }}</span>
    </label>
    <p v-if="error" role="alert" class="text-sm text-destructive break-words">{{ error }}</p>
  </fieldset>
</template>
