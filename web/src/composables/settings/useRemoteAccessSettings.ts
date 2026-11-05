import { computed, onScopeDispose, ref, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import {
  rtspConfigApi, rustdeskConfigApi, vncConfigApi,
  type RustDeskConfigResponse, type RustDeskStatusResponse, type RustDeskPasswordResponse,
  type RtspStatusResponse, type RtspConfigUpdate, type VncConfigUpdate, type VncStatusResponse,
} from '@/api'
import { useConfigStore } from '@/stores/config'
import { useClipboard } from '@/composables/useClipboard'
import { formatHostForUrl } from '@/lib/url'

type RemoteService = 'rtsp' | 'vnc' | 'rustdesk'

interface RemoteAccessSettingsOptions {
  loading: Ref<boolean>
  showValidationError: (message: string) => boolean
}

export function useRemoteAccessSettings({ loading, showValidationError }: RemoteAccessSettingsOptions) {
  const { t } = useI18n()
  const configStore = useConfigStore()
  const { copy: clipboardCopy } = useClipboard()
  const serviceSaving = ref<RemoteService | null>(null)
  let copiedTimer: ReturnType<typeof setTimeout> | undefined
  onScopeDispose(() => clearTimeout(copiedTimer))

  const rustdeskConfig = ref<RustDeskConfigResponse | null>(null)
  const rustdeskStatus = ref<RustDeskStatusResponse | null>(null)
  const rustdeskPassword = ref<RustDeskPasswordResponse | null>(null)
  const rustdeskLoading = ref(false)
  const rustdeskCopied = ref<'id' | 'password' | null>(null)
  const rustdeskLocalConfig = ref({
    enabled: false,
    mode: 'id' as 'id' | 'direct_ip',
    codec: 'h264' as 'h264' | 'h265',
    direct_access_port: 21118,
    rendezvous_server: '',
    relay_server: '',
    relay_key: '',
  })

  const rustdeskValidationMessage = computed(() => {
    if (
      rustdeskLocalConfig.value.mode === 'id' &&
      !rustdeskLocalConfig.value.rendezvous_server?.trim()
    ) {
      return t('extensions.rustdesk.rendezvousServerRequired')
    }
    if (
      rustdeskLocalConfig.value.mode === 'direct_ip' &&
      (rustdeskLocalConfig.value.direct_access_port < 1 ||
        rustdeskLocalConfig.value.direct_access_port > 65535)
    ) {
      return t('extensions.rustdesk.directAccessPortInvalid')
    }
    return ''
  })

  const rtspStatus = ref<RtspStatusResponse | null>(null)
  const rtspLoading = ref(false)
  const rtspLocalConfig = ref<RtspConfigUpdate>({
    enabled: false,
    bind: '0.0.0.0',
    port: 8554,
    path: 'live',
    allow_one_client: true,
    codec: 'h264',
    username: '',
    password: '',
  })

  const vncStatus = ref<VncStatusResponse | null>(null)
  const vncLoading = ref(false)
  const vncLocalConfig = ref<VncConfigUpdate>({
    enabled: false,
    bind: '0.0.0.0',
    port: 5900,
    encoding: 'tight_jpeg',
    allow_one_client: true,
    password: '',
  })

  const rtspStreamUrl = computed(() => {
    const host = formatHostForUrl(window.location.hostname || '127.0.0.1')
    const path = normalizeRtspPath(rtspLocalConfig.value.path || 'live')
    const port = Number(rtspLocalConfig.value.port) || 8554
    return `rtsp://${host}:${port}/${path}`
  })

  const vncStreamUrl = computed(() => {
    const host = formatHostForUrl(window.location.hostname || '127.0.0.1')
    const port = Number(vncLocalConfig.value.port) || 5900
    return `${host}:${port}`
  })

  function applyRustdeskStatus(status: RustDeskStatusResponse) {
    const config = status.config
    rustdeskConfig.value = config
    rustdeskStatus.value = status
    rustdeskLocalConfig.value = {
      enabled: config.enabled,
      mode: config.mode || 'id',
      codec: config.codec || 'h264',
      direct_access_port: config.direct_access_port || 21118,
      rendezvous_server: config.rendezvous_server,
      relay_server: config.relay_server || '',
      relay_key: config.relay_key || '',
    }
  }

  async function loadRustdeskConfig() {
    rustdeskLoading.value = true
    try {
      const status = await configStore.refreshRustdeskStatus()
      applyRustdeskStatus(status)
    } catch {
    } finally {
      rustdeskLoading.value = false
    }
  }

  async function loadRustdeskPassword() {
    try {
      rustdeskPassword.value = await configStore.refreshRustdeskPassword()
    } catch {}
  }

  function normalizeRustdeskServer(value: string, defaultPort: number): string {
    const trimmed = value.trim()
    if (!trimmed) return ''
    if (trimmed.includes(':')) return trimmed
    return `${trimmed}:${defaultPort}`
  }

  /** Strip line breaks from pasted keys. */
  function normalizeRustdeskRelayKey(value: string): string {
    return value.replace(/\r?\n/g, '').trim()
  }

  function validateRustdeskConfig(): boolean {
    return !rustdeskValidationMessage.value || showValidationError(rustdeskValidationMessage.value)
  }

  function validateVncConfig(enabled = vncLocalConfig.value.enabled): boolean {
    const password = (vncLocalConfig.value.password || '').trim()
    if (enabled && !vncStatus.value?.config.has_password && !password) {
      return showValidationError(t('extensions.vnc.passwordRequired'))
    }
    if (password.length > 8) {
      return showValidationError(t('extensions.vnc.passwordMaxLength'))
    }
    return true
  }

  function normalizeRtspPath(path: string): string {
    return path.trim().replace(/^\/+|\/+$/g, '') || 'live'
  }

  function rustdeskUpdatePayload(enabled = rustdeskLocalConfig.value.enabled) {
    return {
      enabled,
      mode: rustdeskLocalConfig.value.mode,
      codec: rustdeskLocalConfig.value.codec,
      direct_access_port: rustdeskLocalConfig.value.direct_access_port,
      rendezvous_server: normalizeRustdeskServer(rustdeskLocalConfig.value.rendezvous_server, 21116),
      relay_server: normalizeRustdeskServer(rustdeskLocalConfig.value.relay_server, 21117),
      relay_key: normalizeRustdeskRelayKey(rustdeskLocalConfig.value.relay_key),
    }
  }

  async function saveService(
    service: RemoteService,
    persist: () => Promise<unknown>,
    reload: () => Promise<void>,
  ) {
    serviceSaving.value = service
    loading.value = true
    try {
      await persist()
      await reload()
      toast.success(t('settingsUi.saveDone'))
    } catch {
      // API requests report their own errors.
    } finally {
      loading.value = false
      serviceSaving.value = null
    }
  }

  async function saveRustdeskConfig() {
    if (
      (rustdeskLocalConfig.value.enabled || rustdeskLocalConfig.value.mode === 'direct_ip') &&
      !validateRustdeskConfig()
    )
      return

    await saveService(
      'rustdesk', () => configStore.updateRustdesk(rustdeskUpdatePayload()), loadRustdeskConfig,
    )
  }

  async function regenerateRustdeskId() {
    if (!confirm(t('extensions.rustdesk.confirmRegenerateId'))) return
    rustdeskLoading.value = true
    try {
      await configStore.regenerateRustdeskId()
      await loadRustdeskConfig()
      await loadRustdeskPassword()
    } catch {
    } finally {
      rustdeskLoading.value = false
    }
  }

  async function regenerateRustdeskPassword() {
    if (!confirm(t('extensions.rustdesk.confirmRegeneratePassword'))) return
    rustdeskLoading.value = true
    try {
      await configStore.regenerateRustdeskPassword()
      await loadRustdeskConfig()
      await loadRustdeskPassword()
    } catch {
    } finally {
      rustdeskLoading.value = false
    }
  }

  async function startRustdesk() {
    if (!validateRustdeskConfig()) return

    rustdeskLoading.value = true
    try {
      await configStore.updateRustdesk(rustdeskUpdatePayload())
      const status = await rustdeskConfigApi.start()
      applyRustdeskStatus(status)
    } catch {
    } finally {
      rustdeskLoading.value = false
    }
  }

  async function stopRustdesk() {
    rustdeskLoading.value = true
    try {
      const status = await rustdeskConfigApi.stop()
      applyRustdeskStatus(status)
    } catch {
    } finally {
      rustdeskLoading.value = false
    }
  }

  async function copyToClipboard(text: string, type: 'id' | 'password') {
    const success = await clipboardCopy(text)
    if (success) {
      rustdeskCopied.value = type
      clearTimeout(copiedTimer)
      copiedTimer = setTimeout(() => (rustdeskCopied.value = null), 2000)
    }
  }

  function applyRtspStatus(status: RtspStatusResponse) {
    rtspStatus.value = status
    rtspLocalConfig.value = {
      enabled: status.config.enabled,
      bind: status.config.bind,
      port: status.config.port,
      path: status.config.path,
      allow_one_client: status.config.allow_one_client,
      codec: status.config.codec,
      username: status.config.username || '',
      password: status.config.password || '',
    }
  }

  async function loadRtspConfig() {
    rtspLoading.value = true
    try {
      const status = await configStore.refreshRtspStatus()
      applyRtspStatus(status)
    } catch {
    } finally {
      rtspLoading.value = false
    }
  }

  function rtspUpdatePayload(enabled = !!rtspLocalConfig.value.enabled): RtspConfigUpdate {
    return {
      enabled,
      bind: rtspLocalConfig.value.bind?.trim() || '0.0.0.0',
      port: Number(rtspLocalConfig.value.port) || 8554,
      path: normalizeRtspPath(rtspLocalConfig.value.path || 'live'),
      allow_one_client: !!rtspLocalConfig.value.allow_one_client,
      codec: rtspLocalConfig.value.codec || 'h264',
      username: (rtspLocalConfig.value.username || '').trim(),
      password: (rtspLocalConfig.value.password || '').trim(),
    }
  }

  async function saveRtspConfig() {
    await saveService(
      'rtsp', () => configStore.updateRtsp(rtspUpdatePayload()), loadRtspConfig,
    )
  }

  async function startRtsp() {
    rtspLoading.value = true
    try {
      await configStore.updateRtsp(rtspUpdatePayload())
      const status = await rtspConfigApi.start()
      applyRtspStatus(status)
    } catch {
    } finally {
      rtspLoading.value = false
    }
  }

  async function stopRtsp() {
    rtspLoading.value = true
    try {
      const status = await rtspConfigApi.stop()
      applyRtspStatus(status)
    } catch {
    } finally {
      rtspLoading.value = false
    }
  }

  function applyVncStatus(status: VncStatusResponse) {
    vncStatus.value = status
    vncLocalConfig.value = {
      enabled: status.config.enabled,
      bind: status.config.bind,
      port: status.config.port,
      encoding: status.config.encoding,
      allow_one_client: status.config.allow_one_client,
      password: '',
    }
  }

  async function loadVncConfig() {
    vncLoading.value = true
    try {
      const status = await configStore.refreshVncStatus()
      applyVncStatus(status)
    } catch {
    } finally {
      vncLoading.value = false
    }
  }

  function vncUpdatePayload(enabled = !!vncLocalConfig.value.enabled): VncConfigUpdate {
    const update: VncConfigUpdate = {
      enabled,
      bind: vncLocalConfig.value.bind?.trim() || '0.0.0.0',
      port: Number(vncLocalConfig.value.port) || 5900,
      encoding: vncLocalConfig.value.encoding || 'tight_jpeg',
      allow_one_client: !!vncLocalConfig.value.allow_one_client,
    }
    const password = (vncLocalConfig.value.password || '').trim()
    if (password) update.password = password
    return update
  }

  async function saveVncConfig() {
    if (!validateVncConfig()) return

    await saveService(
      'vnc', () => configStore.updateVnc(vncUpdatePayload()), loadVncConfig,
    )
  }

  async function startVnc() {
    if (!validateVncConfig(true)) return

    vncLoading.value = true
    try {
      await configStore.updateVnc(vncUpdatePayload())
      const status = await vncConfigApi.start()
      applyVncStatus(status)
    } catch {
    } finally {
      vncLoading.value = false
    }
  }

  async function stopVnc() {
    vncLoading.value = true
    try {
      const status = await vncConfigApi.stop()
      applyVncStatus(status)
    } catch {
    } finally {
      vncLoading.value = false
    }
  }

  return {
    rustdeskConfig,
    rustdeskStatus,
    rustdeskPassword,
    rustdeskLoading,
    rustdeskCopied,
    rustdeskLocalConfig,
    rustdeskValidationMessage,
    rtspStatus,
    rtspLoading,
    rtspLocalConfig,
    vncStatus,
    vncLoading,
    vncLocalConfig,
    rtspStreamUrl,
    vncStreamUrl,
    serviceSaving,
    loadRustdeskConfig,
    loadRustdeskPassword,
    saveRustdeskConfig,
    regenerateRustdeskId,
    regenerateRustdeskPassword,
    startRustdesk,
    stopRustdesk,
    copyToClipboard,
    loadRtspConfig,
    saveRtspConfig,
    startRtsp,
    stopRtsp,
    loadVncConfig,
    saveVncConfig,
    startVnc,
    stopVnc,
  }
}
