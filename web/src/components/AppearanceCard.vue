<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { toast } from 'vue-sonner'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Button } from '@/components/ui/button'
import { useAppearance } from '@/composables/useAppearance'

const { settings, load } = useAppearance()

const loadingStyle = ref<'minimal' | 'branded' | 'custom'>('minimal')
const background = ref<string | null>(null)
const noSignalImage = ref<string | null>(null)
const saving = ref(false)
const busy = ref<'bg' | 'ns' | null>(null)

const signalPreset = ref<'auto' | 'off' | 'custom'>('auto')
const signalVid = ref('345f')
const signalPid = ref('')
const signalRegister = ref('0x1F87')
const signalLockedValue = ref(1)
const signalRead = ref<{ ok: boolean; node?: string; raw?: number; error?: string } | null>(null)
const reading = ref(false)

const styles: Array<{ id: 'minimal' | 'branded' | 'custom'; label: string }> = [
  { id: 'minimal', label: '精简（点阵 + 转圈）' },
  { id: 'branded', label: '品牌（深色 + Logo）' },
  { id: 'custom', label: '自定义背景' },
]

const presets: Array<{ id: 'auto' | 'off' | 'custom'; label: string }> = [
  { id: 'auto', label: '自动（MacroSilicon 0x1F87）' },
  { id: 'off', label: '关闭（从不显示）' },
  { id: 'custom', label: '自定义' },
]

function syncFromSettings() {
  loadingStyle.value = (settings.value.loading_style as typeof loadingStyle.value) || 'minimal'
  background.value = settings.value.loading_background || null
  noSignalImage.value = settings.value.no_signal_image || null
  const signal = (settings.value as Record<string, any>).signal || {}
  signalPreset.value = (signal.preset as typeof signalPreset.value) || 'auto'
  signalVid.value = signal.vid || '345f'
  signalPid.value = signal.pid || ''
  signalRegister.value = signal.register || '0x1F87'
  signalLockedValue.value = typeof signal.locked_value === 'number' ? signal.locked_value : 1
}

onMounted(async () => {
  await load(true)
  syncFromSettings()
})

async function save() {
  saving.value = true
  try {
    const res = await fetch('/api/appearance', {
      method: 'PUT',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        loading_style: loadingStyle.value,
        loading_background: background.value,
        no_signal_image: noSignalImage.value,
        signal: {
          preset: signalPreset.value,
          vid: signalVid.value || null,
          pid: signalPid.value || null,
          register: signalRegister.value || null,
          locked_value: signalLockedValue.value,
        },
      }),
    })
    if (!res.ok) throw new Error(await res.text())
    settings.value = await res.json()
    toast.success('外观已保存')
  } catch (error) {
    toast.error('保存失败', { description: String(error) })
  } finally {
    saving.value = false
  }
}

async function readSignal() {
  reading.value = true
  signalRead.value = null
  try {
    const res = await fetch('/api/signal/read', { credentials: 'include' })
    signalRead.value = await res.json()
  } catch (error) {
    signalRead.value = { ok: false, error: String(error) }
  } finally {
    reading.value = false
  }
}

function downscale(file: File, maxWidth: number, quality: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file)
    const image = new Image()
    image.onload = () => {
      const scale = Math.min(1, maxWidth / image.width)
      const canvas = document.createElement('canvas')
      canvas.width = Math.max(1, Math.round(image.width * scale))
      canvas.height = Math.max(1, Math.round(image.height * scale))
      const ctx = canvas.getContext('2d')
      if (!ctx) return reject(new Error('canvas unavailable'))
      ctx.drawImage(image, 0, 0, canvas.width, canvas.height)
      URL.revokeObjectURL(url)
      resolve(canvas.toDataURL('image/jpeg', quality))
    }
    image.onerror = () => {
      URL.revokeObjectURL(url)
      reject(new Error('图片读取失败'))
    }
    image.src = url
  })
}

async function pick(event: Event, target: 'bg' | 'ns') {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  busy.value = target
  try {
    const dataUrl = await downscale(file, target === 'ns' ? 1920 : 1600, 0.82)
    if (target === 'bg') background.value = dataUrl
    else noSignalImage.value = dataUrl
  } catch (error) {
    toast.error('处理图片失败', { description: String(error) })
  } finally {
    busy.value = null
  }
}
</script>

<template>
  <Card>
    <CardHeader class="settings-card-heading">
      <CardTitle>加载画面与无信号画面</CardTitle>
    </CardHeader>
    <CardContent class="space-y-5">
      <div class="space-y-2">
        <p class="text-sm font-medium">加载样式</p>
        <div class="grid gap-2 sm:grid-cols-3">
          <Button
            v-for="item in styles"
            :key="item.id"
            size="sm"
            class="justify-center"
            :variant="loadingStyle === item.id ? 'default' : 'outline'"
            @click="loadingStyle = item.id"
          >{{ item.label }}</Button>
        </div>
      </div>

      <div class="space-y-2">
        <p class="text-sm font-medium">自定义背景（选择“自定义背景”时生效）</p>
        <div class="flex flex-wrap items-center gap-3">
          <label
            class="inline-flex cursor-pointer items-center rounded-md border px-3 py-1.5 text-sm"
            :class="busy === 'bg' ? 'opacity-60' : ''"
          >
            <input type="file" accept="image/*" class="hidden" @change="pick($event, 'bg')" />
            {{ busy === 'bg' ? '处理中…' : '选择图片' }}
          </label>
          <Button v-if="background" size="sm" variant="ghost" @click="background = null">清除</Button>
          <img v-if="background" :src="background" alt="" class="h-12 w-20 rounded border object-cover" />
        </div>
      </div>

      <div class="space-y-2">
        <p class="text-sm font-medium">无信号画面（替代彩条）</p>
        <p class="text-xs text-muted-foreground">
          上传后，被控机没信号时所有客户端（Web/RTSP/VNC）看到的是这张图，未上传则用彩条。
        </p>
        <div class="flex flex-wrap items-center gap-3">
          <label
            class="inline-flex cursor-pointer items-center rounded-md border px-3 py-1.5 text-sm"
            :class="busy === 'ns' ? 'opacity-60' : ''"
          >
            <input type="file" accept="image/*" class="hidden" @change="pick($event, 'ns')" />
            {{ busy === 'ns' ? '处理中…' : '选择图片' }}
          </label>
          <Button v-if="noSignalImage" size="sm" variant="ghost" @click="noSignalImage = null">清除</Button>
          <img v-if="noSignalImage" :src="noSignalImage" alt="" class="h-12 w-20 rounded border object-cover" />
        </div>
      </div>

      <div class="space-y-2 border-t pt-4">
        <p class="text-sm font-medium">无信号检测（什么时候出彩条）</p>
        <p class="text-xs text-muted-foreground">
          采集卡没有 HDMI 信号时靠读取芯片寄存器判断。不同型号/固件的寄存器取值不一样，
          换卡后可以点“读取一次”看看实际读数。
        </p>
        <div class="grid gap-2 sm:grid-cols-3">
          <Button
            v-for="item in presets"
            :key="item.id"
            size="sm"
            class="justify-center"
            :variant="signalPreset === item.id ? 'default' : 'outline'"
            @click="signalPreset = item.id"
          >{{ item.label }}</Button>
        </div>
        <div v-if="signalPreset === 'custom'" class="grid gap-2 sm:grid-cols-4">
          <label class="text-xs">
            VID
            <input v-model="signalVid" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
          </label>
          <label class="text-xs">
            PID（空=任意）
            <input v-model="signalPid" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
          </label>
          <label class="text-xs">
            寄存器
            <input v-model="signalRegister" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
          </label>
          <label class="text-xs">
            有信号时的读数
            <input v-model.number="signalLockedValue" type="number" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
          </label>
        </div>
        <div class="flex flex-wrap items-center gap-3">
          <Button size="sm" variant="outline" :disabled="reading" @click="readSignal()">
            {{ reading ? '读取中…' : '读取一次' }}
          </Button>
          <span v-if="signalRead" class="text-xs">
            <template v-if="signalRead.ok">
              {{ signalRead.node }} · 0x{{ (signalRead.raw ?? 0).toString(16).toUpperCase().padStart(2, '0') }}
              <span class="text-muted-foreground">（十进制 {{ signalRead.raw }}）</span>
            </template>
            <template v-else>{{ signalRead.error }}</template>
          </span>
        </div>
      </div>

      <div class="flex items-center gap-3 border-t pt-4">
        <Button size="sm" :disabled="saving" @click="save()">
          {{ saving ? '保存中…' : '保存' }}
        </Button>
        <span class="text-xs text-muted-foreground">保存后刷新控制台页面生效</span>
      </div>
    </CardContent>
  </Card>
</template>
