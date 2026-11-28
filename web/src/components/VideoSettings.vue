<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Eye, EyeOff, Loader2, Save } from 'lucide-vue-next'
import { streamApi, type EncoderBackendInfo } from '@/api'
import { EncoderType } from '@/types/generated'
import { useConfigStore } from '@/stores/config'
import CaptureActivityControl from '@/components/CaptureActivityControl.vue'
import CardTitle from '@/components/SettingsCardTitle.vue'
import { Card, CardHeader, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from '@/components/ui/select'
const props = defineProps<{ active: boolean }>()
const { t } = useI18n()
const configStore = useConfigStore()
const showPasswords = ref(false)
const config = ref({
  encoder_backend: EncoderType.Auto,
  webrtc_udp_port_range: '',
  stun_server: '',
  turn_server: '',
  turn_username: '',
  turn_password: '',
})
const availableBackends = ref<EncoderBackendInfo[]>([])

const selectedBackendFormats = computed(() => {
  if (config.value.encoder_backend === 'auto') return []
  const backend = availableBackends.value.find((b) => b.id === config.value.encoder_backend)
  return backend?.supported_formats || []
})

const configLoaded = ref(false)
const videoSaving = ref<'encoder' | 'connection' | null>(null)
async function saveVideoCard(card: 'encoder' | 'connection') {
  if (videoSaving.value || !configLoaded.value) return
  videoSaving.value = card
  try {
    if (card === 'encoder') {
      await configStore.updateStream({ encoder: config.value.encoder_backend })
    } else {
      await configStore.updateStream({
        webrtc_udp_port_range: config.value.webrtc_udp_port_range.trim(),
        stun_server: config.value.stun_server.trim(),
        turn_server: config.value.turn_server.trim(),
        turn_username: config.value.turn_username.trim(),
        turn_password: config.value.turn_password.trim(),
      })
    }
    toast.success(t('settingsUi.saveDone'))
  } catch (error) {
    toast.error(t('settingsUi.saveFailed'), {
      description: error instanceof Error ? error.message : undefined,
    })
  } finally {
    videoSaving.value = null
  }
}

async function loadStreamConfig() {
  configLoaded.value = false
  try {
    const stream = await configStore.refreshStream()
    config.value = {
      encoder_backend: stream.encoder || EncoderType.Auto,
      webrtc_udp_port_range: stream.webrtc_udp_port_range ?? '',
      stun_server: stream.stun_server || '',
      turn_server: stream.turn_server || '',
      turn_username: stream.turn_username || '',
      turn_password: stream.turn_password || '',
    }
    configLoaded.value = true
  } catch { /* The request layer displays loading failures. */ }
}
async function loadBackends() {
  try {
    const result = await streamApi.getCodecs()
    availableBackends.value = result.backends || []
  } catch {}
}

watch(() => props.active, async (active) => {
  if (active) await Promise.all([loadStreamConfig(), loadBackends()])
}, { immediate: true })
</script>

<template>
<!-- Video Section -->
<div v-show="active" class="space-y-4">
  <CaptureActivityControl :active="active" />

  <!-- Encoder Settings -->
  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.encoderBackendDesc')">{{
        t('settings.encoderBackend')
      }}</CardTitle>
      <div class="settings-card-actions">
        <div class="settings-save-action">
          <Button :disabled="!configLoaded || videoSaving !== null" @click="saveVideoCard('encoder')"
            ><Loader2 v-if="videoSaving === 'encoder'" class="size-4 animate-spin" /><Save
              v-else
              class="size-4"
            />{{ t('settingsUi.saveEncoder') }}</Button
          >
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-field space-y-2">
        <Label for="encoder-backend">{{ t('settings.backend') }}</Label>
        <Select v-model="config.encoder_backend">
          <SelectTrigger id="encoder-backend" class="w-full"
            ><SelectValue
          /></SelectTrigger>
          <SelectContent>
            <SelectItem value="auto">{{ t('settings.autoRecommended') }}</SelectItem>
            <SelectItem
              v-for="backend in availableBackends"
              :key="backend.id"
              :value="backend.id"
              >{{ backend.name }}
              {{
                backend.is_hardware
                  ? `(${t('settings.hardware')})`
                  : `(${t('settings.software')})`
              }}</SelectItem
            >
          </SelectContent>
        </Select>
      </div>
      <div
        v-if="config.encoder_backend !== 'auto' && selectedBackendFormats.length > 0"
        class="settings-field space-y-2"
      >
        <Label>{{ t('settings.supportedFormats') }}</Label>
        <div class="flex flex-wrap gap-2">
          <Badge
            v-for="format in selectedBackendFormats"
            :key="format"
            variant="outline"
            >{{ format.toUpperCase() }}</Badge
          >
        </div>
      </div>
    </CardContent>
  </Card>

  <!-- WebRTC/STUN/TURN Settings -->
  <Card>
    <CardHeader class="settings-card-heading"
      ><CardTitle :help="t('settings.webrtcSettingsDesc')">{{
        t('settings.webrtcSettings')
      }}</CardTitle>
      <div class="settings-card-actions">
        <div class="settings-save-action">
          <Button :disabled="!configLoaded || videoSaving !== null" @click="saveVideoCard('connection')"
            ><Loader2
              v-if="videoSaving === 'connection'"
              class="size-4 animate-spin"
            /><Save v-else class="size-4" />{{ t('settingsUi.saveConnection') }}</Button
          >
        </div>
      </div></CardHeader
    >
    <CardContent class="space-y-4">
      <div class="settings-field space-y-2">
        <Label for="webrtc-udp-port-range">{{ t('settings.webrtcUdpPortRange') }}</Label>
        <div class="space-y-2">
          <Input
            id="webrtc-udp-port-range"
            v-model="config.webrtc_udp_port_range"
            placeholder="50000-50100"
            aria-describedby="webrtc-udp-port-range-help"
          />
          <p id="webrtc-udp-port-range-help" class="text-sm text-muted-foreground">
            {{ t('settings.webrtcUdpPortRangeHelp') }}
          </p>
        </div>
      </div>
      <div class="settings-field space-y-2">
        <Label for="stun-server">{{ t('settings.stunServer') }}</Label>
        <Input
          id="stun-server"
          v-model="config.stun_server"
          :placeholder="t('settings.stunServerPlaceholder')"
        />
      </div>
      <Separator />
      <div class="settings-field space-y-2">
        <Label for="turn-server">{{ t('settings.turnServer') }}</Label>
        <Input
          id="turn-server"
          v-model="config.turn_server"
          :placeholder="t('settings.turnServerPlaceholder')"
        />
      </div>
      <div class="settings-field-list grid gap-4 sm:grid-cols-2">
        <div class="settings-field space-y-2">
          <Label for="turn-username">{{ t('settings.turnUsername') }}</Label>
          <Input
            id="turn-username"
            v-model="config.turn_username"
            :disabled="!config.stun_server && !config.turn_server"
          />
        </div>
        <div class="settings-field space-y-2">
          <Label for="turn-password">{{ t('settings.turnPassword') }}</Label>
          <div class="relative">
            <Input
              id="turn-password"
              v-model="config.turn_password"
              :type="showPasswords ? 'text' : 'password'"
              autocomplete="off"
              :disabled="!config.stun_server && !config.turn_server"
            />
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground"
              :aria-label="
                showPasswords
                  ? t('common.hidePassword')
                  : t('common.showPassword')
              "
              @click="showPasswords = !showPasswords"
            >
              <Eye v-if="!showPasswords" class="size-4" />
              <EyeOff v-else class="size-4" />
            </Button>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</div>

</template>
