<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
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

// ---- 无信号检测 ----
interface ChipPreset {
  label: string
  register: string
  lockedValue: number
  verified: boolean
}
interface ChipModel {
  id: string
  label: string
  vid: string
  pid: string
  presets: ChipPreset[]
}

const chipModels: ChipModel[] = [
  { id: 'ms2130', label: 'MS2130', vid: '345f', pid: '2130', presets: [
    { label: '0x1F87 · 有信号 = 1（已验证）', register: '0x1F87', lockedValue: 1, verified: true },
  ] },
  { id: 'ms2130s', label: 'MS2130S', vid: '345f', pid: '2132', presets: [] },
  { id: 'ms2131', label: 'MS2131', vid: '345f', pid: '2131', presets: [] },
  { id: 'ms2131s', label: 'MS2131S', vid: '345f', pid: '2133', presets: [] },
  { id: 'ms2109', label: 'MS2109', vid: '534d', pid: '2109', presets: [] },
  { id: 'ms2109s', label: 'MS2109S', vid: '534d', pid: '', presets: [] },
  { id: 'other', label: '其他 / 自填', vid: '', pid: '', presets: [] },
]

const detectEnabled = ref(true)
const modelId = ref('ms2130')
const presetIndex = ref(0)
const customMode = ref(false)
const register = ref('0x1F87')
const lockedValue = ref(1)
const vid = ref('345f')
const pid = ref('')
const readResult = ref<{ ok: boolean; node?: string; raw?: number; error?: string } | null>(null)
const reading = ref(false)

const currentModel = computed<ChipModel>(() => chipModels.find(m => m.id === modelId.value) ?? chipModels[0]!)
const modelPresets = computed(() => currentModel.value.presets)

const loadingStyles = [
  { id: 'minimal', label: '精简（点阵 + 转圈）' },
  { id: 'branded', label: '品牌（深色 + Logo）' },
  { id: 'custom', label: '自定义背景' },
] as const

function applyModel() {
  const model = currentModel.value
  vid.value = model.vid
  pid.value = model.pid
  const first = model.presets[0]
  if (first) {
    customMode.value = false
    presetIndex.value = 0
    register.value = first.register
    lockedValue.value = first.lockedValue
  } else {
    customMode.value = true
  }
}

function applyPreset() {
  const preset = modelPresets.value[presetIndex.value]
  if (!preset) return
  register.value = preset.register
  lockedValue.value = preset.lockedValue
}

function syncFromSettings() {
  loadingStyle.value = (settings.value.loading_style as typeof loadingStyle.value) || 'minimal'
  background.value = settings.value.loading_background || null
  noSignalImage.value = settings.value.no_signal_image || null

  const signal = (settings.value as Record<string, any>).signal || {}
  const preset = (signal.preset as string) || 'auto'
  if (preset === 'off') {
    detectEnabled.value = false
    return
  }
  detectEnabled.value = true
  if (signal.vid || signal.pid) {
    vid.value = signal.vid || ''
    pid.value = signal.pid || ''
    const matched = chipModels.find(m => m.vid === vid.value && (m.pid === pid.value || !pid.value))
    modelId.value = matched?.id ?? 'other'
  }
  register.value = signal.register || register.value
  lockedValue.value = typeof signal.locked_value === 'number' ? signal.locked_value : lockedValue.value
  const presets = currentModel.value.presets
  const idx = presets.findIndex(
    p => p.register.toLowerCase() === register.value.toLowerCase() && p.lockedValue === lockedValue.value,
  )
  if (idx >= 0) {
    customMode.value = false
    presetIndex.value = idx
  } else {
    customMode.value = true
  }
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
        signal: detectEnabled.value
          ? {
              preset: 'custom',
              vid: vid.value || null,
              pid: pid.value || null,
              register: register.value || null,
              locked_value: lockedValue.value,
            }
          : { preset: 'off' },
      }),
    })
    if (!res.ok) throw new Error(await res.text())
    settings.value = await res.json()
    toast.success('已保存')
  } catch (error) {
    toast.error('保存失败', { description: String(error) })
  } finally {
    saving.value = false
  }
}

async function readSignal() {
  reading.value = true
  readResult.value = null
  try {
    const res = await fetch('/api/signal/read', { credentials: 'include' })
    readResult.value = await res.json()
  } catch (error) {
    readResult.value = { ok: false, error: String(error) }
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
    <CardContent class="space-y-6">
      <!-- 加载样式 -->
      <section class="space-y-2">
        <p class="text-sm font-medium">加载样式</p>
        <div class="grid gap-2 sm:grid-cols-3">
          <Button
            v-for="item in loadingStyles"
            :key="item.id"
            size="sm"
            class="justify-center"
            :variant="loadingStyle === item.id ? 'default' : 'outline'"
            @click="loadingStyle = item.id"
          >{{ item.label }}</Button>
        </div>
      </section>

      <!-- 两张图：只在「自定义背景」样式下出现 -->
      <section v-if="loadingStyle === 'custom'" class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <p class="text-sm font-medium">自定义背景</p>
          <p class="text-xs text-muted-foreground">「自定义背景」样式下作为刷新/连接时的底图</p>
          <div class="flex items-center gap-2">
            <label class="cursor-pointer rounded-md border px-3 py-1.5 text-sm" :class="busy === 'bg' ? 'opacity-60' : ''">
              <input type="file" accept="image/*" class="hidden" @change="pick($event, 'bg')" />
              {{ busy === 'bg' ? '处理中…' : '选择图片' }}
            </label>
            <Button v-if="background" size="sm" variant="ghost" @click="background = null">清除</Button>
            <img v-if="background" :src="background" alt="" class="h-10 w-16 rounded border object-cover" />
          </div>
        </div>
        <div class="space-y-2">
          <p class="text-sm font-medium">无信号画面</p>
          <p class="text-xs text-muted-foreground">没信号时替代彩条（Web / RTSP / VNC 一致）</p>
          <div class="flex items-center gap-2">
            <label class="cursor-pointer rounded-md border px-3 py-1.5 text-sm" :class="busy === 'ns' ? 'opacity-60' : ''">
              <input type="file" accept="image/*" class="hidden" @change="pick($event, 'ns')" />
              {{ busy === 'ns' ? '处理中…' : '选择图片' }}
            </label>
            <Button v-if="noSignalImage" size="sm" variant="ghost" @click="noSignalImage = null">清除</Button>
            <img v-if="noSignalImage" :src="noSignalImage" alt="" class="h-10 w-16 rounded border object-cover" />
          </div>
        </div>
      </section>

      <!-- 无信号检测 -->
      <section class="space-y-3 border-t pt-4">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <p class="text-sm font-medium">无信号检测</p>
          <div class="flex gap-2">
            <Button size="sm" :variant="detectEnabled ? 'default' : 'outline'" @click="detectEnabled = true">启用</Button>
            <Button size="sm" :variant="!detectEnabled ? 'default' : 'outline'" @click="detectEnabled = false">关闭</Button>
          </div>
        </div>

        <template v-if="detectEnabled">
          <div class="grid gap-3 sm:grid-cols-2">
            <label class="text-xs">
              采集卡型号
              <select
                v-model="modelId"
                class="mt-1 w-full rounded border bg-transparent px-2 py-1.5 text-sm"
                @change="applyModel()"
              >
                <option v-for="model in chipModels" :key="model.id" :value="model.id">
                  {{ model.label }}
                </option>
              </select>
            </label>
            <label class="text-xs">
              检测值
              <select
                v-if="modelPresets.length"
                v-model.number="presetIndex"
                class="mt-1 w-full rounded border bg-transparent px-2 py-1.5 text-sm"
                @change="applyPreset()"
              >
                <option v-for="(preset, index) in modelPresets" :key="preset.register + preset.lockedValue" :value="index">
                  {{ preset.label }}
                </option>
              </select>
              <select v-else class="mt-1 w-full rounded border bg-transparent px-2 py-1.5 text-sm" disabled>
                <option>暂无数据（可手动填写下面的值）</option>
              </select>
            </label>
          </div>

          <div class="grid gap-2 sm:grid-cols-4">
            <label class="text-xs">
              VID
              <input v-model="vid" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
            </label>
            <label class="text-xs">
              PID（空=任意）
              <input v-model="pid" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
            </label>
            <label class="text-xs">
              寄存器
              <input v-model="register" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
            </label>
            <label class="text-xs">
              有信号时的读数
              <input v-model.number="lockedValue" type="number" class="mt-1 w-full rounded border px-2 py-1 font-mono text-sm" />
            </label>
          </div>

          <div class="flex flex-wrap items-center gap-3">
            <Button size="sm" variant="outline" :disabled="reading" @click="readSignal()">
              {{ reading ? '读取中…' : '读取一次' }}
            </Button>
            <span v-if="readResult" class="text-xs">
              <template v-if="readResult.ok">
                {{ readResult.node }} · 0x{{ (readResult.raw ?? 0).toString(16).toUpperCase().padStart(2, '0') }}
                <span class="text-muted-foreground">（十进制 {{ readResult.raw }}）</span>
              </template>
              <template v-else>{{ readResult.error }}</template>
            </span>
          </div>
        </template>
        <p v-else class="text-xs text-muted-foreground">已关闭：任何情况下都不会显示彩条/无信号画面。</p>
      </section>

      <div class="flex items-center gap-3 border-t pt-4">
        <Button size="sm" :disabled="saving" @click="save()">
          {{ saving ? '保存中…' : '保存' }}
        </Button>
        <span class="text-xs text-muted-foreground">保存后刷新控制台页面生效</span>
      </div>
    </CardContent>
  </Card>
</template>
