<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Loader2, Save, RefreshCw } from 'lucide-vue-next'
import { useConfigStore } from '@/stores/config'
import { useSystemStore } from '@/stores/system'
import { useHidConnection } from '@/composables/useHidConnection'
import { deviceRequest, matchesSelection, selectionFrom, supportsOtgFunctions, type HidSelection } from '@/lib/hidGuide'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import CardTitle from './SettingsCardTitle.vue'
import HidDriverForm from './HidDriverForm.vue'
import HidDriverDialog from './HidDriverDialog.vue'
import BluetoothHidSettings from './BluetoothHidSettings.vue'

const props = defineProps<{ active: boolean; dirty?: boolean }>()
const emit = defineEmits<{ applied: []; discard: [] }>()
const { t } = useI18n()
const store = useConfigStore(),
  system = useSystemStore()
const draft = ref<HidSelection>(selectionFrom(store.hid))
let baseline = JSON.stringify(draft.value)
const valid = ref(false),
  busy = ref(false),
  error = ref(''),
  uncertain = ref(false),
  repairOpen = ref(false)
const polling = computed(
  () => props.active && store.hid?.backend === 'bluetooth' && !repairOpen.value,
)
const backend = computed(() => store.hid?.backend)
const { bluetooth, restart } = useHidConnection(polling, backend)
watch(
  () => store.hid,
  (hid) => {
    if (JSON.stringify(draft.value) === baseline) draft.value = selectionFrom(hid)
    baseline = JSON.stringify(selectionFrom(hid))
  },
  { immediate: true },
)
watch(
  draft,
  () => {
    uncertain.value = false
  },
  { deep: true },
)
async function apply() {
  if (busy.value || props.dirty || !valid.value || uncertain.value) return
  busy.value = true
  error.value = ''
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 30000)
  try {
    await store.updateHid(deviceRequest(draft.value), controller.signal)
    baseline = JSON.stringify(selectionFrom(store.hid))
    draft.value = selectionFrom(store.hid)
    emit('applied')
    restart()
    toast.success(t('settingsUi.driverApplied'))
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    // A timed-out reset may already have applied. Re-read, never repeat it.
    try {
      const current = await store.refreshHid()
      if (matchesSelection(current, draft.value)) {
        uncertain.value = true
        error.value += ` ${t('hidGuide.uncertain')}`
      }
    } catch {
      uncertain.value = true
    }
  } finally {
    clearTimeout(timeout)
    busy.value = false
  }
}
async function reload() {
  busy.value = true
  error.value = ''
  try {
    await store.refreshHid()
    draft.value = selectionFrom(store.hid)
    baseline = JSON.stringify(draft.value)
    uncertain.value = false
    emit('applied')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}
</script>
<template>
  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settingsUi.driverHelp')">{{ t('hidGuide.deviceTitle') }}</CardTitle>
      <div class="settings-card-actions">
        <Button :disabled="busy || !valid || dirty || uncertain" @click="apply"
          ><Loader2 v-if="busy" class="size-4 animate-spin" /><Save v-else class="size-4" />{{
            t('settingsUi.applyDriver')
          }}</Button
        >
      </div>
    </CardHeader>
    <CardContent class="space-y-4">
      <HidDriverForm
        v-model="draft"
        class="hid-settings-form"
        :platform="system.platform"
        :locked="busy"
        @valid="valid = $event"
      />
      <p
        v-if="supportsOtgFunctions(store.hid?.backend) && !supportsOtgFunctions(draft.backend)"
        class="text-sm text-warning"
      >
        {{
          t('hidGuide.disableUsb', {
            functions: [t('hidGuide.msd'), t('hidGuide.audio'), t('hidGuide.network')].join(' / '),
          })
        }}
      </p>
      <p v-if="draft.backend === 'bluetooth'" class="text-sm text-warning">
        {{ t('hidGuide.resetWarning') }}
      </p>
      <div v-if="dirty" class="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
        <span>{{ t('settingsUi.saveFeaturesFirst') }}</span
        ><Button variant="outline" :disabled="busy" @click="emit('discard')">{{
          t('settingsUi.discardFeatures')
        }}</Button>
      </div>
      <div v-if="error" class="space-y-2">
        <p role="alert" class="text-sm text-destructive">{{ error }}</p>
        <Button v-if="uncertain" variant="outline" :disabled="busy" @click="reload"
          ><RefreshCw class="size-4" />{{ t('common.refresh') }}</Button
        >
      </div>
    </CardContent>
  </Card>
  <Card v-if="store.hid?.backend === 'bluetooth'">
    <CardHeader
      ><CardTitle :help="t('hidGuide.pairInstructions', { name: store.hid.bluetooth.name })">{{
        t('hidGuide.features')
      }}</CardTitle></CardHeader
    >
    <CardContent class="space-y-4"
      ><p class="text-sm text-muted-foreground">
        {{ t('hidGuide.pairInstructions', { name: store.hid.bluetooth.name }) }}
      </p>
      <BluetoothHidSettings
        :bluetooth="bluetooth"
        @refresh="restart"
        @reconfigure="repairOpen = true"
    /></CardContent>
  </Card>
  <HidDriverDialog
    v-if="repairOpen"
    :dirty="dirty"
    @close="repairOpen = false"
    @applied="emit('applied')"
    @discard="emit('discard')"
  />
</template>
