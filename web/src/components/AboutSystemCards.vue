<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Copy, ExternalLink } from 'lucide-vue-next'
import { useSystemStore } from '@/stores/system'
import { useClipboard } from '@/composables/useClipboard'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import CardTitle from './SettingsCardTitle.vue'
import { Button } from '@/components/ui/button'

const props = defineProps<{ active: boolean }>()
const { t } = useI18n()
const system = useSystemStore()
const clipboard = useClipboard()
const failed = ref(false)
let timer: ReturnType<typeof setTimeout> | undefined
let generation = 0
async function refresh(own = generation) {
  try {
    await system.fetchSystemInfo()
    if (own === generation) failed.value = false
  } catch {
    if (own === generation) failed.value = true
  } finally {
    if (own === generation && props.active) timer = setTimeout(() => void refresh(own), 5000)
  }
}
watch(
  () => props.active,
  (active) => {
    clearTimeout(timer)
    const own = ++generation
    if (active) void refresh(own)
  },
  { immediate: true },
)
onUnmounted(() => {
  ++generation
  clearTimeout(timer)
})
const host = computed(() => system.hostInfo)
function bytes(value?: number) {
  if (value === undefined || !Number.isFinite(value)) return '—'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let index = 0
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024
    index++
  }
  return `${value.toLocaleString(undefined, { maximumFractionDigits: index > 2 ? 1 : 0 })} ${units[index]}`
}
function percent(used?: number, total?: number) {
  return used !== undefined && total && Number.isFinite(used)
    ? Math.min(100, Math.max(0, (used / total) * 100))
    : null
}
const metrics = computed(() => {
  const device = system.deviceInfo
  const shared = host.value?.storage.shared_partition === true
  const result = [
    {
      name: t('settingsUi.cpu'),
      value: device ? Math.min(100, Math.max(0, device.cpu_usage)) : null,
      detail: host.value ? t('settingsUi.cores', { count: host.value.cpu_count }) : '—',
    },
    {
      name: t('settingsUi.memory'),
      value: percent(device?.memory_used, device?.memory_total),
      detail: device ? `${bytes(device.memory_used)} / ${bytes(device.memory_total)}` : '—',
    },
    {
      name: t(shared ? 'settingsUi.sharedDisk' : 'settingsUi.systemDisk'),
      value: percent(host.value?.storage.system?.used, host.value?.storage.system?.total),
      detail: host.value?.storage.system
        ? `${bytes(host.value.storage.system.used)} / ${bytes(host.value.storage.system.total)}`
        : '—',
    },
  ]
  if (!shared && system.platform?.msd.available !== false) {
    const msd = host.value?.storage.msd
    result.push({
      name: t('settingsUi.msdDisk'),
      value: percent(msd?.used, msd?.total),
      detail: msd ? `${bytes(msd.used)} / ${bytes(msd.total)}` : '—',
    })
  }
  return result
})
const fields = computed(() => [
  [t('settingsUi.softwareVersion'), system.version ? `One-KVM v${system.version}` : '—'],
  [t('settingsUi.operatingSystem'), host.value?.operating_system || '—'],
  [t('settingsUi.processor'), system.deviceInfo?.cpu_model || '—'],
  [t('settingsUi.architecture'), host.value?.architecture || '—'],
  [t('settingsUi.kernel'), host.value?.kernel || '—'],
  [t('settingsUi.hostname'), system.deviceInfo?.hostname || '—'],
])
async function copyInfo() {
  const copied = await clipboard.copy(
    fields.value.map(([key, value]) => `${key}: ${value}`).join('\n'),
  )
  copied ? toast.success(t('settingsUi.copied')) : toast.error(t('settingsUi.copyFailed'))
}
</script>
<template>
  <Card class="system-load-card">
    <CardHeader
      ><CardTitle :help="t('settingsUi.loadHelp')">{{
        t('settingsUi.load')
      }}</CardTitle></CardHeader
    >
    <CardContent>
      <div class="load-metrics" :style="{ '--metric-count': metrics.length }">
        <div v-for="metric in metrics" :key="metric.name" class="load-metric">
          <div
            class="usage-ring"
            role="img"
            :aria-label="`${metric.name}: ${metric.value === null ? t('settingsUi.unavailable') : Math.round(metric.value) + '%'}`"
          >
            <svg viewBox="0 0 120 120" aria-hidden="true">
              <circle class="ring-track" cx="60" cy="60" r="49" />
              <circle
                v-if="metric.value !== null"
                class="ring-value"
                cx="60"
                cy="60"
                r="49"
                pathLength="100"
                :stroke-dasharray="`${metric.value} 100`"
              />
            </svg>
            <span class="usage-value"
              >{{ metric.value === null ? '—' : Math.round(metric.value)
              }}<small v-if="metric.value !== null">%</small></span
            >
          </div>
          <div class="metric-label">{{ metric.name }}</div>
          <div class="metric-detail">{{ metric.detail }}</div>
        </div>
      </div>
      <p
        v-if="failed"
        role="status"
        class="mt-4 flex items-center gap-2 text-sm text-muted-foreground"
      >
        {{ t('settingsUi.loadFailed') }}
      </p>
    </CardContent>
  </Card>
  <Card class="system-info-card">
    <CardHeader
      ><CardTitle :help="t('settingsUi.systemInfoHelp')">{{ t('settingsUi.systemInfo') }}</CardTitle
      ><Button variant="outline" @click="copyInfo"
        ><Copy class="size-4" />{{ t('settingsUi.copyInfo') }}</Button
      ></CardHeader
    >
    <CardContent>
      <dl class="system-info-grid">
        <div v-for="[key, value] in fields" :key="key">
          <dt>{{ key }}</dt>
          <dd>{{ value }}</dd>
        </div>
      </dl>
      <div class="project-links">
        <a href="https://github.com/mofeng-git/One-KVM" target="_blank" rel="noopener noreferrer"
          >{{ t('settingsUi.project') }}<ExternalLink /></a
        ><a href="https://docs.one-kvm.cn/" target="_blank" rel="noopener noreferrer"
          >{{ t('settingsUi.docs') }}<ExternalLink /></a
        ><a
          href="https://github.com/mofeng-git/One-KVM/issues"
          target="_blank"
          rel="noopener noreferrer"
          >{{ t('settingsUi.feedback') }}<ExternalLink
        /></a>
      </div>
    </CardContent>
  </Card>
</template>
