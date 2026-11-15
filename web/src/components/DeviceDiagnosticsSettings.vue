<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { RefreshCw } from 'lucide-vue-next'
import { hidApi, streamApi, type VideoEncoderSelfCheckResponse } from '@/api'
import type { OtgSelfCheckLevel, OtgSelfCheckItem, OtgSelfCheckResponse as OtgSelfCheckResult } from '@/types/generated'
import { useSystemStore } from '@/stores/system'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Card, CardHeader, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Table, TableHeader, TableHead, TableRow, TableBody, TableCell } from '@/components/ui/table'
const props = defineProps<{ active: boolean; hidBackend: string }>()
const { t, te } = useI18n()
const systemStore = useSystemStore()
const otgSupported = computed(() => systemStore.platform?.otg.available === true)
type OtgCheckGroupStatus = 'ok' | 'warn' | 'error' | 'skipped'

interface OtgCheckGroupDef {
  id: string
  titleKey: string
  checkIds: string[]
}

interface OtgCheckGroup {
  id: string
  titleKey: string
  status: OtgCheckGroupStatus
  okCount: number
  warningCount: number
  errorCount: number
  items: OtgSelfCheckItem[]
}

const otgSelfCheckLoading = ref(false)
const otgSelfCheckResult = ref<OtgSelfCheckResult | null>(null)
const otgSelfCheckError = ref('')
const otgRunButtonPressed = ref(false)

const otgCheckGroupDefs: OtgCheckGroupDef[] = [
  {
    id: 'udc',
    titleKey: 'settings.otgSelfCheck.groups.udc',
    checkIds: ['udc_dir_exists', 'udc_has_entries', 'configured_udc_valid'],
  },
  {
    id: 'gadget_config',
    titleKey: 'settings.otgSelfCheck.groups.gadgetConfig',
    checkIds: ['configfs_mounted', 'usb_gadget_dir_exists', 'libcomposite_loaded'],
  },
  {
    id: 'one_kvm',
    titleKey: 'settings.otgSelfCheck.groups.oneKvm',
    checkIds: ['one_kvm_gadget_exists', 'one_kvm_bound_udc', 'other_gadgets', 'udc_conflict'],
  },
  {
    id: 'functions',
    titleKey: 'settings.otgSelfCheck.groups.functions',
    checkIds: [
      'hid_functions_present',
      'config_c1_exists',
      'function_links_ok',
      'hid_device_nodes',
    ],
  },
  {
    id: 'link',
    titleKey: 'settings.otgSelfCheck.groups.link',
    checkIds: ['udc_state', 'udc_speed'],
  },
]

const otgCheckGroups = computed<OtgCheckGroup[]>(() => {
  const items = otgSelfCheckResult.value?.checks || []
  return otgCheckGroupDefs.map((group) => {
    const groupItems = items.filter((item) => group.checkIds.includes(item.id))
    const errorCount = groupItems.filter((item) => item.level === 'error').length
    const warningCount = groupItems.filter((item) => item.level === 'warn').length
    const okCount = Math.max(0, groupItems.length - errorCount - warningCount)
    let status: OtgCheckGroupStatus = 'skipped'
    if (groupItems.length > 0) {
      if (errorCount > 0) status = 'error'
      else if (warningCount > 0) status = 'warn'
      else status = 'ok'
    }

    return {
      id: group.id,
      titleKey: group.titleKey,
      status,
      okCount,
      warningCount,
      errorCount,
      items: groupItems,
    }
  })
})

function otgCheckLevelClass(level: OtgSelfCheckLevel): string {
  if (level === 'error') return 'bg-destructive'
  if (level === 'warn') return 'bg-warning'
  return 'bg-info'
}

function otgCheckStatusText(level: OtgSelfCheckLevel): string {
  if (level === 'error') return t('common.error')
  if (level === 'warn') return t('common.warning')
  return t('common.info')
}

function otgGroupStatusClass(status: OtgCheckGroupStatus): string {
  if (status === 'error') return 'bg-destructive'
  if (status === 'warn') return 'bg-warning'
  if (status === 'ok') return 'bg-status-active'
  return 'bg-muted-foreground/40'
}

function otgGroupStatusText(status: OtgCheckGroupStatus): string {
  return t(`settings.otgSelfCheck.status.${status}`)
}

function otgGroupSummary(group: OtgCheckGroup): string {
  if (group.items.length === 0) {
    return t('settings.otgSelfCheck.notRun')
  }
  return t('settings.otgSelfCheck.groupCounts', {
    ok: group.okCount,
    warnings: group.warningCount,
    errors: group.errorCount,
  })
}

function otgCheckMessage(item: OtgSelfCheckItem): string {
  const key = `settings.otgSelfCheck.messages.${item.id}`
  const label = te(key) ? t(key) : item.message
  const result = otgSelfCheckResult.value
  if (!result) return label

  const value = (name: string) => t(`settings.otgSelfCheck.values.${name}`)

  switch (item.id) {
    case 'udc_has_entries':
      return `${label}：${result.available_udcs.length ? result.available_udcs.join(', ') : value('missing')}`
    case 'configured_udc_valid':
      if (!result.selected_udc) return `${label}：${value('notConfigured')}`
      return `${label}：${item.ok ? result.selected_udc : `${value('missing')}/${result.selected_udc}`}`
    case 'configfs_mounted':
      return `${label}：${item.ok ? value('mounted') : value('unmounted')}`
    case 'usb_gadget_dir_exists':
      return `${label}：${item.ok ? value('available') : value('unavailable')}`
    case 'libcomposite_loaded':
      return `${label}：${item.ok ? value('available') : value('unavailable')}`
    case 'one_kvm_gadget_exists':
      return `${label}：${item.ok ? value('exists') : value('missing')}`
    case 'other_gadgets':
      return `${label}：${result.other_gadgets.length ? result.other_gadgets.join(', ') : value('none')}`
    case 'one_kvm_bound_udc':
      return `${label}：${result.bound_udc || value('unbound')}`
    case 'udc_conflict':
      return `${label}：${item.ok ? value('noConflict') : value('conflict')}`
    case 'udc_state':
      return `${label}：${result.udc_state || value('unknown')}`
    case 'udc_speed':
      return `${label}：${result.udc_speed || value('unknown')}`
    default:
      return `${label}：${item.ok ? value('normal') : value('abnormal')}`
  }
}

function otgCheckHint(item: OtgSelfCheckItem): string {
  if (!item.hint) return ''
  const key = `settings.otgSelfCheck.hints.${item.id}`
  return te(key) ? t(key) : item.hint
}

async function runOtgSelfCheck() {
  if (!otgSupported.value) return
  otgSelfCheckLoading.value = true
  otgSelfCheckError.value = ''
  try {
    otgSelfCheckResult.value = await hidApi.otgSelfCheck()
  } catch {
    otgSelfCheckError.value = t('settings.otgSelfCheck.failed')
  } finally {
    otgSelfCheckLoading.value = false
  }
}

async function onRunOtgSelfCheckClick() {
  if (!otgSelfCheckLoading.value) {
    otgRunButtonPressed.value = true
    window.setTimeout(() => {
      otgRunButtonPressed.value = false
    }, 160)
  }
  await runOtgSelfCheck()
}

type VideoEncoderSelfCheckCell = VideoEncoderSelfCheckResponse['rows'][number]['cells'][number]
type VideoEncoderSelfCheckRow = VideoEncoderSelfCheckResponse['rows'][number]

const videoEncoderSelfCheckLoading = ref(false)
const videoEncoderSelfCheckResult = ref<VideoEncoderSelfCheckResponse | null>(null)
const videoEncoderSelfCheckError = ref('')
const videoEncoderRunButtonPressed = ref(false)

function videoEncoderCell(
  row: VideoEncoderSelfCheckRow,
  codecId: string,
): VideoEncoderSelfCheckCell | undefined {
  return row.cells.find((cell) => cell.codec_id === codecId)
}

const currentHardwareEncoderText = computed(() =>
  videoEncoderSelfCheckResult.value?.current_hardware_encoder === 'None'
    ? t('settings.encoderSelfCheck.none')
    : videoEncoderSelfCheckResult.value?.current_hardware_encoder ||
      t('settings.encoderSelfCheck.none'),
)

function videoEncoderCodecLabel(codecId: string, codecName: string): string {
  return codecId === 'h265' ? 'H.265' : codecName
}

function videoEncoderCellClass(ok: boolean | undefined): string {
  return ok ? 'text-success' : 'text-destructive'
}

function videoEncoderCellSymbol(ok: boolean | undefined): string {
  return ok ? '✓' : '✗'
}

function videoEncoderCellTime(cell: VideoEncoderSelfCheckCell | undefined): string {
  if (!cell || typeof cell.elapsed_ms !== 'number') return '-'
  return `${cell.elapsed_ms}ms`
}

async function runVideoEncoderSelfCheck() {
  videoEncoderSelfCheckLoading.value = true
  videoEncoderSelfCheckError.value = ''
  try {
    videoEncoderSelfCheckResult.value = await streamApi.encoderSelfCheck()
  } catch {
    videoEncoderSelfCheckError.value = t('settings.encoderSelfCheck.failed')
  } finally {
    videoEncoderSelfCheckLoading.value = false
  }
}

async function onRunVideoEncoderSelfCheckClick() {
  if (!videoEncoderSelfCheckLoading.value) {
    videoEncoderRunButtonPressed.value = true
    window.setTimeout(() => {
      videoEncoderRunButtonPressed.value = false
    }, 160)
  }
  await runVideoEncoderSelfCheck()
}

watch(() => props.hidBackend, () => {
  otgSelfCheckResult.value = null
  otgSelfCheckError.value = ''
})
</script>

<template>
<!-- Environment Section -->
<div v-show="active" class="settings-environment">
  <Card v-if="otgSupported" :class="{ 'settings-wide': otgSelfCheckResult }">
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.otgSelfCheck.desc')">{{
        t('settings.otgSelfCheck.title')
      }}</CardTitle>
      <div class="settings-card-actions">
        <Button
          variant="outline"
          size="sm"
          :disabled="otgSelfCheckLoading"
          :class="[
            'transition-all duration-150 active:scale-95 active:brightness-95',
            otgRunButtonPressed ? 'scale-95 brightness-95' : '',
          ]"
          @click="onRunOtgSelfCheckClick"
        >
          <RefreshCw
            class="size-4 mr-2"
            :class="{ 'animate-spin': otgSelfCheckLoading }"
          />
          {{ t('settings.otgSelfCheck.run') }}
        </Button>
      </div></CardHeader
    >
    <CardContent class="space-y-3">
      <p v-if="otgSelfCheckError" class="text-xs text-destructive">
        {{ otgSelfCheckError }}
      </p>

      <template v-if="otgSelfCheckResult">
        <div class="flex flex-wrap gap-2 text-xs">
          <Badge
            :variant="otgSelfCheckResult.overall_ok ? 'default' : 'destructive'"
            class="font-medium"
          >
            {{ t('settings.otgSelfCheck.overall') }}：{{
              otgSelfCheckResult.overall_ok
                ? t('settings.otgSelfCheck.ok')
                : t('settings.otgSelfCheck.hasIssues')
            }}
          </Badge>
          <Badge variant="outline" class="font-normal">
            {{
              t('settings.otgSelfCheck.counts', {
                errors: otgSelfCheckResult.error_count,
                warnings: otgSelfCheckResult.warning_count,
              })
            }}
          </Badge>
          <Badge variant="secondary" class="font-normal">
            {{ t('settings.otgSelfCheck.selectedUdc') }}：{{
              otgSelfCheckResult.selected_udc || '-'
            }}
          </Badge>
          <Badge variant="secondary" class="font-normal">
            {{ t('settings.otgSelfCheck.boundUdc') }}：{{
              otgSelfCheckResult.bound_udc || '-'
            }}
          </Badge>
        </div>

        <div class="rounded-md border divide-y">
          <details
            v-for="group in otgCheckGroups"
            :key="group.id"
            :open="group.status === 'error' || group.status === 'warn'"
            class="group"
          >
            <summary
              class="list-none cursor-pointer px-4 py-3 flex items-center justify-between gap-3 hover:bg-muted/40"
            >
              <div class="flex items-center gap-2 min-w-0">
                <span
                  class="inline-block size-2 rounded-full shrink-0"
                  :class="otgGroupStatusClass(group.status)"
                />
                <span class="text-sm font-medium truncate leading-6">{{
                  t(group.titleKey)
                }}</span>
              </div>
              <div class="flex items-center gap-2 shrink-0">
                <span class="text-xs text-muted-foreground">{{
                  otgGroupSummary(group)
                }}</span>
                <Badge variant="outline" class="text-[10px] h-5 px-1.5">{{
                  otgGroupStatusText(group.status)
                }}</Badge>
              </div>
            </summary>

            <div v-if="group.items.length > 0" class="border-t bg-muted/20">
              <div
                v-for="item in group.items"
                :key="item.id"
                class="px-4 py-3 border-b last:border-b-0"
              >
                <div class="flex items-start gap-2">
                  <span
                    class="inline-block size-2 rounded-full mt-1.5 shrink-0"
                    :class="otgCheckLevelClass(item.level)"
                  />
                  <div class="min-w-0 space-y-1">
                    <div class="flex items-center gap-2">
                      <p class="text-sm leading-5">{{ otgCheckMessage(item) }}</p>
                      <span class="text-[11px] text-muted-foreground shrink-0">{{
                        otgCheckStatusText(item.level)
                      }}</span>
                    </div>
                    <div
                      class="flex flex-wrap gap-x-2 gap-y-1 text-[11px] text-muted-foreground"
                    >
                      <span v-if="item.hint">{{ otgCheckHint(item) }}</span>
                      <code v-if="item.path" class="font-mono break-all">{{
                        item.path
                      }}</code>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <div
              v-else
              class="border-t bg-muted/20 px-4 py-3 text-xs text-muted-foreground"
            >
              {{ t('settings.otgSelfCheck.notRun') }}
            </div>
          </details>
        </div>
      </template>
      <p v-else-if="otgSelfCheckLoading" class="text-xs text-muted-foreground">
        {{ t('common.loading') }}
      </p>
    </CardContent>
  </Card>

  <Card :class="{ 'settings-wide': videoEncoderSelfCheckResult }">
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.encoderSelfCheck.desc')">{{
        t('settings.encoderSelfCheck.title')
      }}</CardTitle>
      <div class="settings-card-actions">
        <Button
          variant="outline"
          size="sm"
          :disabled="videoEncoderSelfCheckLoading"
          :class="[
            'transition-all duration-150 active:scale-95 active:brightness-95',
            videoEncoderRunButtonPressed ? 'scale-95 brightness-95' : '',
          ]"
          @click="onRunVideoEncoderSelfCheckClick"
        >
          <RefreshCw
            class="size-4 mr-2"
            :class="{ 'animate-spin': videoEncoderSelfCheckLoading }"
          />
          {{ t('settings.encoderSelfCheck.run') }}
        </Button>
      </div></CardHeader
    >
    <CardContent class="space-y-3">
      <p v-if="videoEncoderSelfCheckError" class="text-xs text-destructive">
        {{ videoEncoderSelfCheckError }}
      </p>

      <template v-if="videoEncoderSelfCheckResult">
        <div class="text-sm">
          {{ t('settings.encoderSelfCheck.currentHardwareEncoder') }}：{{
            currentHardwareEncoderText
          }}
        </div>

        <div class="rounded-md border bg-card">
          <Table class="table-fixed">
            <TableHeader>
              <TableRow>
                <TableHead class="w-[18%] px-2">{{
                  t('settings.encoderSelfCheck.resolution')
                }}</TableHead>
                <TableHead
                  v-for="codec in videoEncoderSelfCheckResult.codecs"
                  :key="codec.id"
                  class="w-[20.5%] px-2 text-center"
                >
                  {{ videoEncoderCodecLabel(codec.id, codec.name) }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow
                v-for="row in videoEncoderSelfCheckResult.rows"
                :key="row.resolution_id"
              >
                <TableCell class="px-2 py-3 align-middle">
                  <div class="font-medium">{{ row.resolution_label }}</div>
                </TableCell>
                <TableCell
                  v-for="codec in videoEncoderSelfCheckResult.codecs"
                  :key="`${row.resolution_id}-${codec.id}`"
                  class="px-2 py-3 align-middle"
                >
                  <div
                    class="flex flex-col items-center justify-center gap-1"
                    :class="videoEncoderCellClass(videoEncoderCell(row, codec.id)?.ok)"
                  >
                    <div class="text-lg leading-none font-semibold">
                      {{ videoEncoderCellSymbol(videoEncoderCell(row, codec.id)?.ok) }}
                    </div>
                    <div class="text-[11px] leading-4 text-foreground/70">
                      {{ videoEncoderCellTime(videoEncoderCell(row, codec.id)) }}
                    </div>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </template>
      <p v-else-if="videoEncoderSelfCheckLoading" class="text-xs text-muted-foreground">
        {{ t('common.loading') }}
      </p>
    </CardContent>
  </Card>
</div>

</template>
