<script setup lang="ts">
import { computed, ref, watch, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { HoverCard, HoverCardContent, HoverCardTrigger } from '@/components/ui/hover-card'
import { Cable, RefreshCw } from 'lucide-vue-next'
import { configApi } from '@/api'
import { request } from '@/api/request'
import type { BluetoothAdapter } from '@/types/bluetooth'
import { selectDevice, validName, type HidSelection, type Driver } from '@/lib/hidGuide'
import HidWiringDiagram from './HidWiringDiagram.vue'
const props = defineProps<{ modelValue: HidSelection; locked?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [HidSelection]; valid: [boolean] }>()
const { t } = useI18n()
const EMPTY_SELECT_VALUE = '__one-kvm-empty-select-value__'
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
    <div class="space-y-1 text-sm">
      <div class="flex min-h-8 items-center justify-between gap-2">
        <label for="hid-driver">{{ t('hidGuide.driver') }}</label>
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
            :disabled="loading"
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
        @update:model-value="value => emit('update:modelValue', { ...modelValue, backend: String(value) as Driver })"
      >
        <SelectTrigger id="hid-driver" class="w-full">
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
    <label v-if="modelValue.backend !== 'none'" class="block space-y-1 text-sm">
      <span>{{ t(`hidGuide.device_${modelValue.backend}`) }}</span>
      <Select
        :model-value="selected || EMPTY_SELECT_VALUE"
        :disabled="loading"
        @update:model-value="value => device(value === EMPTY_SELECT_VALUE ? '' : String(value))"
      >
        <SelectTrigger class="w-full">
          <SelectValue :placeholder="t('hidGuide.selectDevice')" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem :value="EMPTY_SELECT_VALUE" disabled>{{ t('hidGuide.selectDevice') }}</SelectItem>
          <SelectItem v-for="option in options" :key="option.value" :value="option.value">
            {{ option.label }}
          </SelectItem>
        </SelectContent>
      </Select>
    </label>
    <label v-if="modelValue.backend === 'ch9329'" class="block space-y-1 text-sm">
      <span>{{ t('actionbar.baudrate') }}</span>
      <Select
        :model-value="String(modelValue.ch9329_baudrate)"
        @update:model-value="value => emit('update:modelValue', { ...modelValue, ch9329_baudrate: Number(value) })"
      >
        <SelectTrigger class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="rate in [9600, 19200, 38400, 57600, 115200]" :key="rate" :value="String(rate)">
            {{ rate }}
          </SelectItem>
        </SelectContent>
      </Select>
    </label>
    <label v-if="modelValue.backend === 'bluetooth'" class="block space-y-1 text-sm">
      <span>{{ t('bluetoothHid.name') }}</span>
      <Input :model-value="modelValue.bluetooth.name" @update:model-value="emit('update:modelValue', { ...modelValue, bluetooth: { ...modelValue.bluetooth, name: String($event) } })" />
      <span v-if="!validName(modelValue.bluetooth.name)" class="text-destructive text-xs">{{ t('hidGuide.nameInvalid') }}</span>
    </label>
    <p v-if="missing" class="text-sm text-warning">{{ missing }}</p>
    <p v-if="error" role="alert" class="text-sm text-destructive break-words">{{ error }}</p>
  </fieldset>
</template>
