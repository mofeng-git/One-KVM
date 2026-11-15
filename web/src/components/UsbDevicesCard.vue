<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { RefreshCw, Monitor } from 'lucide-vue-next'
import { usbApi } from '@/api'
import { useSystemStore } from '@/stores/system'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Card, CardHeader, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Empty, EmptyHeader, EmptyMedia, EmptyDescription } from '@/components/ui/empty'
import { Table, TableHeader, TableHead, TableRow, TableBody, TableCell } from '@/components/ui/table'
import { AlertDialog, AlertDialogContent, AlertDialogHeader, AlertDialogTitle,
  AlertDialogDescription, AlertDialogFooter, AlertDialogCancel, AlertDialogAction } from '@/components/ui/alert-dialog'
const props = defineProps<{ active: boolean }>()
const { t } = useI18n()
const systemStore = useSystemStore()
const usbResetSupported = computed(() => systemStore.platform?.mode === 'linux')
let refreshTimer: number | undefined
const usbDevices = ref<import('@/api').UsbDeviceInfo[]>([])
const usbDevicesLoading = ref(false)
const usbDevicesError = ref('')
const usbResetTarget = ref<import('@/api').UsbDeviceInfo | null>(null)
const usbResetLoading = ref(false)

async function fetchUsbDevices() {
  if (!usbResetSupported.value) {
    usbDevices.value = []
    return
  }
  usbDevicesLoading.value = true
  usbDevicesError.value = ''
  try {
    usbDevices.value = await usbApi.listDevices()
  } catch {
    usbDevicesError.value = t('settings.usbDevices.loadFailed')
  } finally {
    usbDevicesLoading.value = false
  }
}

async function confirmUsbReset() {
  if (!usbResetSupported.value || !usbResetTarget.value) return
  usbResetLoading.value = true
  try {
    await usbApi.resetDevice(usbResetTarget.value.bus_num, usbResetTarget.value.dev_num)
  } catch {
  } finally {
    usbResetLoading.value = false
    usbResetTarget.value = null
    refreshTimer = window.setTimeout(() => { if (props.active) void fetchUsbDevices() }, 1500)
  }
}

function usbSpeedLabel(speed?: string): string {
  if (!speed) return '-'
  const map: Record<string, string> = {
    '1.5': '1.5 Mbps',
    '12': '12 Mbps',
    '480': '480 Mbps',
    '5000': '5 Gbps',
    '10000': '10 Gbps',
  }
  return map[speed] || `${speed} Mbps`
}

watch(() => props.active && usbResetSupported.value, (active) => {
  if (active) void fetchUsbDevices()
}, { immediate: true })
onUnmounted(() => window.clearTimeout(refreshTimer))
</script>

<template>
<Card v-if="usbResetSupported">
  <CardHeader class="settings-card-heading"
    ><CardTitle :help="t('settings.usbDevices.desc')">{{
      t('settings.usbDevices.title')
    }}</CardTitle>
    <div class="settings-card-actions">
      <Button
        variant="outline"
        size="sm"
        :disabled="usbDevicesLoading"
        @click="fetchUsbDevices()"
      >
        <RefreshCw class="size-4 mr-2" :class="{ 'animate-spin': usbDevicesLoading }" />
        {{ t('settings.usbDevices.refresh') }}
      </Button>
    </div></CardHeader
  >
  <CardContent class="space-y-3">
    <p v-if="usbDevicesError" class="text-xs text-destructive">
      {{ usbDevicesError }}
    </p>

    <template v-if="usbDevices.length > 0">
      <div class="rounded-md border overflow-hidden">
        <Table class="min-w-[540px]">
          <TableHeader>
            <TableRow class="bg-muted/40 hover:bg-muted/40">
              <TableHead>{{ t('settings.usbDevices.colDevice') }}</TableHead>
              <TableHead>VID:PID</TableHead>
              <TableHead>{{ t('settings.usbDevices.colSpeed') }}</TableHead>
              <TableHead>{{ t('settings.usbDevices.colVideo') }}</TableHead>
              <TableHead class="text-right">{{
                t('settings.usbDevices.colAction')
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="dev in usbDevices" :key="`${dev.bus_num}-${dev.dev_num}`">
              <TableCell>
                <div
                  class="font-medium truncate max-w-[180px]"
                  :title="
                    dev.product ||
                    dev.manufacturer ||
                    `${dev.id_vendor}:${dev.id_product}`
                  "
                >
                  {{
                    dev.product ||
                    dev.manufacturer ||
                    `${dev.id_vendor}:${dev.id_product}`
                  }}
                </div>
              </TableCell>
              <TableCell class="font-mono text-xs"
                >{{ dev.id_vendor }}:{{ dev.id_product }}</TableCell
              >
              <TableCell class="text-xs">{{ usbSpeedLabel(dev.speed) }}</TableCell>
              <TableCell class="text-xs">
                <code v-if="dev.video_device">{{ dev.video_device }}</code>
                <span v-else class="text-muted-foreground">-</span>
              </TableCell>
              <TableCell class="text-right">
                <Button
                  v-if="dev.authorized != null"
                  variant="outline"
                  size="sm"
                  class="text-xs"
                  :disabled="usbResetLoading"
                  @click="usbResetTarget = dev"
                >
                  {{ t('settings.usbDevices.reset') }}
                </Button>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
    </template>
    <Skeleton v-else-if="usbDevicesLoading" class="h-24 w-full" />
    <Empty v-else class="py-6">
      <EmptyHeader>
        <EmptyMedia variant="icon"><Monitor /></EmptyMedia>
        <EmptyDescription>{{ t('settings.usbDevices.noDevices') }}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  </CardContent>
</Card>

<!-- USB Reset Confirmation Dialog -->
<AlertDialog v-if="usbResetSupported" :open="usbResetTarget != null">
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>{{
        t('settings.usbDevices.resetConfirmTitle')
      }}</AlertDialogTitle>
      <AlertDialogDescription>
        {{
          t('settings.usbDevices.resetConfirmDesc', {
            device:
              usbResetTarget?.product ||
              `${usbResetTarget?.id_vendor}:${usbResetTarget?.id_product}`,
          })
        }}
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel @click="usbResetTarget = null">{{
        t('common.cancel')
      }}</AlertDialogCancel>
      <AlertDialogAction
        :disabled="usbResetLoading"
        class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
        @click="confirmUsbReset()"
      >
        {{ t('settings.usbDevices.resetAction') }}
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
</template>
