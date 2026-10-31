<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { toast } from 'vue-sonner'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

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
let timer: number | undefined

const stateText = computed(() => {
  const s = status.value
  if (!s) return '读取中…'
  if (!s.installed) return '未安装'
  switch (s.state) {
    case 'Running':
      return '已连接'
    case 'NeedsLogin':
      return '待登录'
    case 'Starting':
      return '启动中'
    case 'Stopped':
    case 'NoState':
      return '未连接'
    default:
      return s.state
  }
})

async function load() {
  try {
    const res = await fetch('/api/extensions/tailscale', { credentials: 'include' })
    if (!res.ok) return
    const data = await res.json()
    status.value = data.status
    config.value = data.config
    loginServer.value = data.config.login_server || ''
    hostname.value = data.config.hostname || ''
  }
  catch {
    // 忽略：设备可能还没升级到这个版本
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
    toast.error('操作失败', { description: String(error) })
  }
  finally {
    busy.value = false
  }
}

async function enable() {
  await patch({
    enabled: true,
    auth_key: authKey.value.trim() || undefined,
    login_server: loginServer.value.trim(),
    hostname: hostname.value.trim(),
  })
  authKey.value = ''
}

async function disable() {
  await patch({ enabled: false })
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
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="text-sm">
          <span class="text-muted-foreground">状态：</span>
          <span class="font-medium">{{ stateText }}</span>
          <span v-if="status?.ip" class="ml-3 font-mono text-muted-foreground">{{ status.ip }}</span>
        </div>
        <div class="flex gap-2">
          <Button size="sm" :disabled="busy || !status?.installed || config.enabled" @click="enable()">启用</Button>
          <Button size="sm" variant="outline" :disabled="busy || !config.enabled" @click="disable()">关闭</Button>
        </div>
      </div>

      <p v-if="status && !status.installed" class="text-xs text-destructive">
        设备上没有 /usr/bin/tailscale，请先在镜像里安装 tailscale。
      </p>

      <div v-if="config.enabled" class="grid gap-3 sm:grid-cols-2">
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
        <div class="flex items-end">
          <Button size="sm" variant="outline" :disabled="busy" @click="patch({ auth_key: authKey.trim() || undefined, login_server: loginServer.trim(), hostname: hostname.trim() })">
            保存认证信息
          </Button>
        </div>
      </div>

      <div v-if="status?.login_url" class="rounded-md border p-3 text-xs">
        <p class="mb-2">设备还没登录，点下面按钮在浏览器里确认：</p>
        <Button size="sm" @click="openLogin()">打开登录页面</Button>
      </div>
    </CardContent>
  </Card>
</template>
