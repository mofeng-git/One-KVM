<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Play, RefreshCw, Square } from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const { t } = useI18n()

interface TsStatus {
  installed: boolean
  running: boolean
  state: string
  ip: string | null
  hostname: string | null
  login_url: string | null
}

interface TsConfig {
  enabled: boolean
  login_server?: string
  hostname?: string
}

const status = ref<TsStatus | null>(null)
const config = ref<TsConfig>({ enabled: false })
const authKey = ref('')
const loginServer = ref('')
const hostname = ref('')
const busy = ref(false)
const loadError = ref('')
let timer: number | undefined

// 连接成功后显示「已连接到 <控制服务器>」
const controlServerText = computed(() => {
  const server = loginServer.value.trim() || config.value.login_server?.trim()
  return server || 'controlplane.tailscale.com'
})

const stateText = computed(() => {
  const s = status.value
  if (!s) return '读取中…'
  if (!s.installed) return '未安装'
  // 开关关闭时守护进程可能还在跑（只是下线），统一显示未连接
  if (!config.value.enabled) return '未连接'
  switch (s.state) {
    case 'Running':
      return '已连接'
    case 'NeedsLogin':
      return '待登录'
    case 'Starting':
      return '启动中'
    case 'Stopped':
    case 'NoState':
    case 'daemon_stopped':
      return '未连接'
    case 'error':
      return '读取状态失败'
    default:
      return s.state
  }
})

async function load() {
  try {
    const res = await fetch('/api/extensions/tailscale', { credentials: 'include' })
    if (res.status === 401) {
      loadError.value = '登录状态已过期，请重新登录'
      return
    }
    if (!res.ok) {
      loadError.value = `读取失败（HTTP ${res.status}）`
      return
    }
    const data = await res.json()
    loadError.value = ''
    status.value = data.status
    config.value = data.config
    loginServer.value = data.config.login_server || ''
    hostname.value = data.config.hostname || ''
  }
  catch (error) {
    loadError.value = `读取失败：${String(error)}`
  }
}

async function patch(body: Record<string, unknown>) {
  busy.value = true
  try {
    const res = await fetch('/api/extensions/tailscale/config', {
      method: 'PATCH',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    })
    if (!res.ok) throw new Error((await res.text()) || res.statusText)
    config.value = await res.json()
    await load()
  }
  catch (error) {
    toast.error(t('common.save') + '失败', { description: String(error) })
  }
  finally {
    busy.value = false
  }
}

async function start() {
  await patch({
    enabled: true,
    auth_key: authKey.value.trim() || undefined,
    login_server: loginServer.value.trim(),
    hostname: hostname.value.trim(),
  })
  authKey.value = ''
}

async function stop() {
  await patch({ enabled: false })
}

async function save() {
  await patch({
    auth_key: authKey.value.trim() || undefined,
    login_server: loginServer.value.trim(),
    hostname: hostname.value.trim(),
  })
  authKey.value = ''
  toast.success(t('common.save'))
}

function openLogin() {
  if (status.value?.login_url) window.open(status.value.login_url, '_blank')
}

onMounted(async () => {
  await load()
  timer = window.setInterval(() => {
    if (config.value.enabled) load()
  }, 5000)
})

onUnmounted(() => {
  if (timer) window.clearInterval(timer)
})
</script>

<template>
  <Card>
    <CardHeader class="settings-card-heading">
      <CardTitle>Tailscale</CardTitle>
      <div class="settings-card-actions">
        <div v-if="status?.installed !== false" class="settings-service-actions">
          <Button variant="outline" size="sm" :disabled="busy || config.enabled" @click="start()">
            <Play class="size-4 mr-1" />
            {{ t('extensions.start') }}
          </Button>
          <Button variant="outline" size="sm" :disabled="busy || !config.enabled" @click="stop()">
            <Square class="size-4 mr-1" />
            {{ t('extensions.stop') }}
          </Button>
        </div>
        <Button variant="ghost" size="sm" :disabled="busy" @click="load()">
          <RefreshCw class="size-4 mr-1" />
          刷新
        </Button>
        <div v-if="!status?.running" class="settings-save-action">
          <Button size="sm" :disabled="busy" @click="save()">
            {{ t('common.save') }}
          </Button>
        </div>
      </div>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="text-sm">
        <span class="text-muted-foreground">状态：</span>
        <template v-if="status?.running">
          <span class="font-medium">已连接到</span>
          <span class="ml-1 font-mono">{{ controlServerText }}</span>
          <template v-if="status.ip">
            <span class="ml-3 text-muted-foreground">ip地址：</span>
            <span class="font-mono">{{ status.ip }}</span>
          </template>
        </template>
        <span v-else class="font-medium">{{ stateText }}</span>
      </div>

      <p v-if="loadError" class="text-xs text-destructive">{{ loadError }}</p>

      <p v-if="status && !status.installed" class="text-xs text-destructive">
        设备上没有 /usr/bin/tailscale，请先在镜像里安装 tailscale。
      </p>

      <div v-if="!status?.running" class="grid gap-3 sm:grid-cols-2">
        <div class="space-y-1">
          <Label class="text-xs">Auth Key（留空则生成登录链接）</Label>
          <Input v-model="authKey" type="password" placeholder="tskey-auth-..." />
        </div>
        <div class="space-y-1">
          <Label class="text-xs">登录服务器（自建 Headscale 时填）</Label>
          <Input v-model="loginServer" placeholder="https://headscale.example.com" />
        </div>
        <div class="space-y-1">
          <Label class="text-xs">主机名（可选）</Label>
          <Input v-model="hostname" :placeholder="status?.hostname || 'one-kvm'" />
        </div>
      </div>

      <div v-if="status?.login_url" class="rounded-md border p-3 text-xs">
        <p class="mb-2">设备还没登录，点下面按钮在浏览器里确认：</p>
        <Button size="sm" @click="openLogin()">打开登录页面</Button>
      </div>
    </CardContent>
  </Card>
</template>
