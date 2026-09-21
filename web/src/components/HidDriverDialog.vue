<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useConfigStore } from '@/stores/config'
import { request } from '@/api/request'
import type { HidConfig, MsdConfig } from '@/types/generated'
import { useHidConnection } from '@/composables/useHidConnection'
import { deviceRequest, selectionFrom, matchesSelection, writePendingHid, pendingHidKey, type PendingHid, type HidSelection } from '@/lib/hidGuide'
import HidDriverForm from './HidDriverForm.vue'
import HidWiringDiagram from './HidWiringDiagram.vue'
import HidDeviceOverview from './HidDeviceOverview.vue'
const props = defineProps<{ dirty?: boolean; pending?: PendingHid | null }>()
const emit = defineEmits<{ close: []; applied: []; discard: [] }>()
const { t } = useI18n(), store = useConfigStore()
const draft = ref<HidSelection>(props.pending ? JSON.parse(JSON.stringify(props.pending.selection)) : selectionFrom(store.hid))
const valid = ref(false), busy = ref(false), error = ref(''), applied = ref(false), loaded = ref(false)
const acknowledged = ref(!props.dirty), uncertain = ref(false)
const pairingStarted = ref(false), autoPair = ref(false), hasApplied = ref(false)
const connectionStarted = ref(Date.now()), connectionTimedOut = ref(false)
const disabledUsb = ref<string[]>([])
const autoSubmitPending = ref(props.pending?.phase === 'selected')
let disposed = false, closed = false
async function cleanupPairing() {
  if (hasApplied.value && store.hid?.backend === 'bluetooth') {
    await request('/hid/bluetooth', { method: 'POST', body: JSON.stringify({ action: 'close' }) }, { toastOnError: false }).catch(() => undefined)
  }
}
onUnmounted(() => {
  disposed = true; autoPair.value = false; autoSubmitPending.value = false
  if (!closed && !busy.value) void cleanupPairing()
})
const backend = computed(() => applied.value ? store.hid?.backend : undefined)
const active = computed(() => applied.value && !busy.value)
const { status, bluetooth, error: statusError, restart } = useHidConnection(active, backend)
watch(status, () => { connectionTimedOut.value = Date.now() - connectionStarted.value >= 120000 })
const ready = computed(() => store.hid?.backend === 'none' || (store.hid?.backend === 'bluetooth' ? bluetooth.value?.ready : status.value?.online))
function remember(phase: PendingHid['phase']) { if (props.pending) writePendingHid({ selection: draft.value, phase }) }
async function readConfig<T>(path: string): Promise<T> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 5000)
  try { return await request<T>(path, { signal: controller.signal }, { toastOnError: false }) }
  finally { clearTimeout(timeout) }
}
async function refreshHid() {
  const hid = await readConfig<HidConfig>('/config/hid')
  if (!disposed) store.hid = hid
  return hid
}
async function refreshMsd() {
  const msd = await readConfig<MsdConfig>('/config/msd')
  if (!disposed) store.msd = msd
  return msd
}
async function refreshConfigs() {
  const results = await Promise.allSettled([refreshHid(), refreshMsd(), readConfig<{ enabled: boolean }>('/config/otg-network'), readConfig<{ enabled: boolean }>('/config/uac')])
  emit('applied')
  const failure = results.find(r => r.status === 'rejected')
  if (failure?.status === 'rejected') throw failure.reason
}
async function apply() {
  if (disposed || busy.value || !valid.value || !loaded.value) return
  autoSubmitPending.value = false
  busy.value = true; error.value = ''; uncertain.value = false
  remember('applying')
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 30000)
  try {
    await store.updateHid(deviceRequest(draft.value), controller.signal)
    applied.value = true; hasApplied.value = true; connectionStarted.value = Date.now(); remember('applied'); autoPair.value = draft.value.backend === 'bluetooth'
    await refreshConfigs()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    // Never repeat an ambiguous reset automatically, including after a browser refresh.
    if (!applied.value) {
      try {
        const current = await refreshHid()
        if (matchesSelection(current, draft.value)) {
          uncertain.value = true
          error.value += ` ${t('hidGuide.uncertain')}`
        }
      } catch { uncertain.value = true }
    }
  } finally { clearTimeout(timeout); busy.value = false; if (disposed && !closed) void cleanupPairing() }
}
async function action(action: 'pair' | 'close') {
  if (disposed || busy.value) return
  busy.value = true; error.value = ''
  try {
    await request('/hid/bluetooth', { method: 'POST', body: JSON.stringify({ action, seconds: 120 }) })
    if (action === 'pair') pairingStarted.value = true
  } catch (e) { error.value = e instanceof Error ? e.message : String(e) }
  finally { busy.value = false; if (disposed && !closed) void cleanupPairing(); else restart() }
}
watch(() => bluetooth.value?.initialized, initialized => {
  if (initialized && autoPair.value && !pairingStarted.value && !busy.value) {
    autoPair.value = false
    void action('pair')
  }
})
function applyPending() {
  if (!autoSubmitPending.value || !valid.value || !loaded.value || disposed) return
  if (props.pending && JSON.stringify(deviceRequest(draft.value)) !== JSON.stringify(deviceRequest(props.pending.selection))) {
    autoSubmitPending.value = false
    error.value = t('hidGuide.resumeRetry')
    return
  }
  void apply()
}
watch(valid, applyPending)
async function edit() {
  if (store.hid?.backend === 'bluetooth' && bluetooth.value?.initialized) {
    await action('close')
    if (error.value) return
  }
  applied.value = false; autoPair.value = false; pairingStarted.value = false; uncertain.value = false
}
async function close() {
  if (busy.value) return
  autoPair.value = false
  if (hasApplied.value && store.hid?.backend === 'bluetooth' && (bluetooth.value?.initialized || pairingStarted.value)) {
    await action('close')
    if (error.value) return
  }
  if (props.pending) sessionStorage.removeItem(pendingHidKey)
  closed = true
  emit('close')
}
async function load() {
  loaded.value = false; error.value = ''
  try {
    await refreshHid()
    if (!props.pending) draft.value = selectionFrom(store.hid)
    if (store.hid?.backend === 'otg') {
      const [msd, network, audio] = await Promise.all([refreshMsd(), readConfig<{ enabled: boolean }>('/config/otg-network'), readConfig<{ enabled: boolean }>('/config/uac')])
      disabledUsb.value = [msd.enabled ? t('hidGuide.msd') : '', network.enabled ? t('hidGuide.network') : '', audio.enabled ? t('hidGuide.audio') : ''].filter(Boolean)
    }
    if (props.pending && props.pending.phase !== 'selected') {
      if (store.hid && matchesSelection(store.hid, props.pending.selection)) {
        applied.value = true; hasApplied.value = true; remember('applied')
        // The previous reset may have succeeded. Resume observation, never clear again.
      } else error.value = t('hidGuide.resumeRetry')
    }
    loaded.value = true
    applyPending()
  } catch (e) { error.value = e instanceof Error ? e.message : String(e) }
}
onMounted(load)
</script>
<template>
  <Dialog :open="true" @update:open="value => { if (!value) void close() }">
    <DialogContent :show-close-button="!busy" class="w-[calc(100vw-2rem)] sm:max-w-[520px] max-h-[calc(100dvh-2rem)] overflow-y-auto" @escape-key-down="event => { if (busy) event.preventDefault() }" @interact-outside="event => { if (busy) event.preventDefault() }">
      <DialogHeader>
        <DialogTitle>{{ t('hidGuide.configure') }}</DialogTitle>
        <DialogDescription>{{ t(applied ? 'hidGuide.appliedHelp' : 'hidGuide.draftHelp') }}</DialogDescription>
      </DialogHeader>
      <div v-if="!acknowledged" class="space-y-4">
        <p>{{ t('hidGuide.dirty') }}</p>
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" @click="emit('close')">{{ t('hidGuide.returnSave') }}</Button>
          <Button @click="acknowledged = true; emit('discard')">{{ t('hidGuide.discard') }}</Button>
        </div>
      </div>
      <template v-else>
        <HidDriverForm v-if="!applied" v-model="draft" :locked="busy || !loaded" @valid="valid = $event" />
        <template v-else>
          <HidWiringDiagram :backend="draft.backend" />
          <HidDeviceOverview :hid="store.hid" :status="status" :bluetooth="bluetooth" :error="statusError" />
          <template v-if="draft.backend === 'bluetooth'">
            <p class="text-sm">{{ t('hidGuide.pairInstructions', { name: store.hid?.bluetooth.name }) }}</p>
            <p v-if="pairingStarted && !bluetooth?.pairing_seconds && !bluetooth?.peer && !ready" class="text-sm">{{ t('hidGuide.pairTimeout') }}</p>
            <Button v-if="!ready && !bluetooth?.pairing_seconds" variant="outline" :disabled="busy || !bluetooth?.initialized" @click="action('pair')">{{ t('hidGuide.reopenPairing') }}</Button>
          </template>
          <p v-if="connectionTimedOut && !ready && draft.backend !== 'bluetooth'" class="text-sm text-warning">{{ t('hidGuide.connectionTimeout') }}</p>
        </template>
        <p v-if="!applied && store.hid?.backend === 'otg' && draft.backend !== 'otg' && disabledUsb.length" class="text-sm text-warning">{{ t('hidGuide.disableUsb', { functions: disabledUsb.join('、') }) }}</p>
        <p v-if="!applied && draft.backend === 'bluetooth'" class="text-sm text-warning">{{ t('hidGuide.resetWarning') }}</p>
        <p v-if="error" role="alert" class="text-sm text-destructive break-words">{{ error }}</p>
        <DialogFooter class="gap-2">
          <Button variant="outline" :disabled="busy" @click="close">{{ t(applied ? ready ? 'hidGuide.done' : 'hidGuide.later' : props.pending ? 'hidGuide.configureLater' : 'common.cancel') }}</Button>
          <Button v-if="uncertain && !applied" variant="outline" :disabled="busy" @click="applied = true; hasApplied = true; remember('applied'); error = ''">{{ t('hidGuide.checkConnection') }}</Button>
          <Button v-if="!loaded" variant="outline" @click="load">{{ t('common.refresh') }}</Button>
          <Button v-if="!applied" :disabled="busy || !loaded || !valid" @click="apply">{{ t(busy ? 'actionbar.applying' : 'common.apply') }}</Button>
          <Button v-else-if="!ready" variant="outline" :disabled="busy" @click="edit()">{{ t('hidGuide.reconfigure') }}</Button>
        </DialogFooter>
      </template>
    </DialogContent>
  </Dialog>
</template>
