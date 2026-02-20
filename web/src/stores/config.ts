import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  authConfigApi,
  atxConfigApi,
  audioConfigApi,
  hidConfigApi,
  msdConfigApi,
  rtspConfigApi,
  rustdeskConfigApi,
  streamConfigApi,
  videoConfigApi,
  webConfigApi,
} from '@/api'
import type {
  AtxConfig,
  AtxConfigUpdate,
  AudioConfig,
  AudioConfigUpdate,
  AuthConfig,
  AuthConfigUpdate,
  HidConfig,
  HidConfigUpdate,
  MsdConfig,
  MsdConfigUpdate,
  StreamConfigResponse,
  StreamConfigUpdate,
  VideoConfig,
  VideoConfigUpdate,
  WebConfig,
  WebConfigUpdate,
} from '@/types/generated'
import type {
  RtspConfigResponse as ApiRtspConfigResponse,
  RtspConfigUpdate as ApiRtspConfigUpdate,
  RtspStatusResponse as ApiRtspStatusResponse,
  RustDeskConfigResponse as ApiRustDeskConfigResponse,
  RustDeskConfigUpdate as ApiRustDeskConfigUpdate,
  RustDeskStatusResponse as ApiRustDeskStatusResponse,
  RustDeskPasswordResponse as ApiRustDeskPasswordResponse,
} from '@/api'

function normalizeErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'string') return error
  return 'Unknown error'
}

export const useConfigStore = defineStore('config', () => {
  const auth = ref<AuthConfig | null>(null)
  const video = ref<VideoConfig | null>(null)
  const audio = ref<AudioConfig | null>(null)
  const hid = ref<HidConfig | null>(null)
  const msd = ref<MsdConfig | null>(null)
  const stream = ref<StreamConfigResponse | null>(null)
  const web = ref<WebConfig | null>(null)
  const atx = ref<AtxConfig | null>(null)
  const rtspConfig = ref<ApiRtspConfigResponse | null>(null)
  const rtspStatus = ref<ApiRtspStatusResponse | null>(null)
  const rustdeskConfig = ref<ApiRustDeskConfigResponse | null>(null)
  const rustdeskStatus = ref<ApiRustDeskStatusResponse | null>(null)
  const rustdeskPassword = ref<ApiRustDeskPasswordResponse | null>(null)

  const authLoading = ref(false)
  const videoLoading = ref(false)
  const audioLoading = ref(false)
  const hidLoading = ref(false)
  const msdLoading = ref(false)
  const streamLoading = ref(false)
  const webLoading = ref(false)
  const atxLoading = ref(false)
  const rtspLoading = ref(false)
  const rustdeskLoading = ref(false)

  const authError = ref<string | null>(null)
  const videoError = ref<string | null>(null)
  const audioError = ref<string | null>(null)
  const hidError = ref<string | null>(null)
  const msdError = ref<string | null>(null)
  const streamError = ref<string | null>(null)
  const webError = ref<string | null>(null)
  const atxError = ref<string | null>(null)
  const rtspError = ref<string | null>(null)
  const rustdeskError = ref<string | null>(null)

  let authPromise: Promise<AuthConfig> | null = null
  let videoPromise: Promise<VideoConfig> | null = null
  let audioPromise: Promise<AudioConfig> | null = null
  let hidPromise: Promise<HidConfig> | null = null
  let msdPromise: Promise<MsdConfig> | null = null
  let streamPromise: Promise<StreamConfigResponse> | null = null
  let webPromise: Promise<WebConfig> | null = null
  let atxPromise: Promise<AtxConfig> | null = null
  let rtspPromise: Promise<ApiRtspConfigResponse> | null = null
  let rtspStatusPromise: Promise<ApiRtspStatusResponse> | null = null
  let rustdeskPromise: Promise<ApiRustDeskConfigResponse> | null = null
  let rustdeskStatusPromise: Promise<ApiRustDeskStatusResponse> | null = null
  let rustdeskPasswordPromise: Promise<ApiRustDeskPasswordResponse> | null = null

  async function refreshAuth() {
    if (authLoading.value && authPromise) return authPromise
    authLoading.value = true
    authError.value = null
    const request = authConfigApi.get()
      .then((response) => {
        auth.value = response
        return response
      })
      .catch((error) => {
        authError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        authLoading.value = false
        authPromise = null
      })

    authPromise = request
    return request
  }

  async function refreshVideo() {
    if (videoLoading.value && videoPromise) return videoPromise
    videoLoading.value = true
    videoError.value = null
    const request = videoConfigApi.get()
      .then((response) => {
        video.value = response
        return response
      })
      .catch((error) => {
        videoError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        videoLoading.value = false
        videoPromise = null
      })

    videoPromise = request
    return request
  }

  async function refreshAudio() {
    if (audioLoading.value && audioPromise) return audioPromise
    audioLoading.value = true
    audioError.value = null
    const request = audioConfigApi.get()
      .then((response) => {
        audio.value = response
        return response
      })
      .catch((error) => {
        audioError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        audioLoading.value = false
        audioPromise = null
      })

    audioPromise = request
    return request
  }

  async function refreshHid() {
    if (hidLoading.value && hidPromise) return hidPromise
    hidLoading.value = true
    hidError.value = null
    const request = hidConfigApi.get()
      .then((response) => {
        hid.value = response
        return response
      })
      .catch((error) => {
        hidError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        hidLoading.value = false
        hidPromise = null
      })

    hidPromise = request
    return request
  }

  async function refreshMsd() {
    if (msdLoading.value && msdPromise) return msdPromise
    msdLoading.value = true
    msdError.value = null
    const request = msdConfigApi.get()
      .then((response) => {
        msd.value = response
        return response
      })
      .catch((error) => {
        msdError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        msdLoading.value = false
        msdPromise = null
      })

    msdPromise = request
    return request
  }

  async function refreshStream() {
    if (streamLoading.value && streamPromise) return streamPromise
    streamLoading.value = true
    streamError.value = null
    const request = streamConfigApi.get()
      .then((response) => {
        stream.value = response
        return response
      })
      .catch((error) => {
        streamError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        streamLoading.value = false
        streamPromise = null
      })

    streamPromise = request
    return request
  }

  async function refreshWeb() {
    if (webLoading.value && webPromise) return webPromise
    webLoading.value = true
    webError.value = null
    const request = webConfigApi.get()
      .then((response) => {
        web.value = response
        return response
      })
      .catch((error) => {
        webError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        webLoading.value = false
        webPromise = null
      })

    webPromise = request
    return request
  }

  async function refreshAtx() {
    if (atxLoading.value && atxPromise) return atxPromise
    atxLoading.value = true
    atxError.value = null
    const request = atxConfigApi.get()
      .then((response) => {
        atx.value = response
        return response
      })
      .catch((error) => {
        atxError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        atxLoading.value = false
        atxPromise = null
      })

    atxPromise = request
    return request
  }

  async function refreshRtspConfig() {
    if (rtspLoading.value && rtspPromise) return rtspPromise
    rtspLoading.value = true
    rtspError.value = null
    const request = rtspConfigApi.get()
      .then((response) => {
        rtspConfig.value = response
        return response
      })
      .catch((error) => {
        rtspError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        rtspLoading.value = false
        rtspPromise = null
      })

    rtspPromise = request
    return request
  }

  async function refreshRtspStatus() {
    if (rtspLoading.value && rtspStatusPromise) return rtspStatusPromise
    rtspLoading.value = true
    rtspError.value = null
    const request = rtspConfigApi.getStatus()
      .then((response) => {
        rtspStatus.value = response
        rtspConfig.value = response.config
        return response
      })
      .catch((error) => {
        rtspError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        rtspLoading.value = false
        rtspStatusPromise = null
      })

    rtspStatusPromise = request
    return request
  }

  async function refreshRustdeskConfig() {
    if (rustdeskLoading.value && rustdeskPromise) return rustdeskPromise
    rustdeskLoading.value = true
    rustdeskError.value = null
    const request = rustdeskConfigApi.get()
      .then((response) => {
        rustdeskConfig.value = response
        return response
      })
      .catch((error) => {
        rustdeskError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        rustdeskLoading.value = false
        rustdeskPromise = null
      })

    rustdeskPromise = request
    return request
  }

  async function refreshRustdeskStatus() {
    if (rustdeskLoading.value && rustdeskStatusPromise) return rustdeskStatusPromise
    rustdeskLoading.value = true
    rustdeskError.value = null
    const request = rustdeskConfigApi.getStatus()
      .then((response) => {
        rustdeskStatus.value = response
        rustdeskConfig.value = response.config
        return response
      })
      .catch((error) => {
        rustdeskError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        rustdeskLoading.value = false
        rustdeskStatusPromise = null
      })

    rustdeskStatusPromise = request
    return request
  }

  async function refreshRustdeskPassword() {
    if (rustdeskLoading.value && rustdeskPasswordPromise) return rustdeskPasswordPromise
    rustdeskLoading.value = true
    rustdeskError.value = null
    const request = rustdeskConfigApi.getPassword()
      .then((response) => {
        rustdeskPassword.value = response
        return response
      })
      .catch((error) => {
        rustdeskError.value = normalizeErrorMessage(error)
        throw error
      })
      .finally(() => {
        rustdeskLoading.value = false
        rustdeskPasswordPromise = null
      })

    rustdeskPasswordPromise = request
    return request
  }

  function ensureAuth() {
    if (auth.value) return Promise.resolve(auth.value)
    return refreshAuth()
  }

  function ensureVideo() {
    if (video.value) return Promise.resolve(video.value)
    return refreshVideo()
  }

  function ensureAudio() {
    if (audio.value) return Promise.resolve(audio.value)
    return refreshAudio()
  }

  function ensureHid() {
    if (hid.value) return Promise.resolve(hid.value)
    return refreshHid()
  }

  function ensureMsd() {
    if (msd.value) return Promise.resolve(msd.value)
    return refreshMsd()
  }

  function ensureStream() {
    if (stream.value) return Promise.resolve(stream.value)
    return refreshStream()
  }

  function ensureWeb() {
    if (web.value) return Promise.resolve(web.value)
    return refreshWeb()
  }

  function ensureAtx() {
    if (atx.value) return Promise.resolve(atx.value)
    return refreshAtx()
  }

  function ensureRtspConfig() {
    if (rtspConfig.value) return Promise.resolve(rtspConfig.value)
    return refreshRtspConfig()
  }

  function ensureRustdeskConfig() {
    if (rustdeskConfig.value) return Promise.resolve(rustdeskConfig.value)
    return refreshRustdeskConfig()
  }

  async function updateAuth(update: AuthConfigUpdate) {
    const response = await authConfigApi.update(update)
    auth.value = response
    return response
  }

  async function updateVideo(update: VideoConfigUpdate) {
    const response = await videoConfigApi.update(update)
    video.value = response
    return response
  }

  async function updateAudio(update: AudioConfigUpdate) {
    const response = await audioConfigApi.update(update)
    audio.value = response
    return response
  }

  async function updateHid(update: HidConfigUpdate) {
    const response = await hidConfigApi.update(update)
    hid.value = response
    return response
  }

  async function updateMsd(update: MsdConfigUpdate) {
    const response = await msdConfigApi.update(update)
    msd.value = response
    return response
  }

  async function updateStream(update: StreamConfigUpdate) {
    const response = await streamConfigApi.update(update)
    stream.value = response
    return response
  }

  async function updateWeb(update: WebConfigUpdate) {
    const response = await webConfigApi.update(update)
    web.value = response
    return response
  }

  async function updateAtx(update: AtxConfigUpdate) {
    const response = await atxConfigApi.update(update)
    atx.value = response
    return response
  }

  async function updateRtsp(update: ApiRtspConfigUpdate) {
    const response = await rtspConfigApi.update(update)
    rtspConfig.value = response
    return response
  }

  async function updateRustdesk(update: ApiRustDeskConfigUpdate) {
    const response = await rustdeskConfigApi.update(update)
    rustdeskConfig.value = response
    return response
  }

  async function regenerateRustdeskId() {
    const response = await rustdeskConfigApi.regenerateId()
    rustdeskConfig.value = response
    return response
  }

  async function regenerateRustdeskPassword() {
    const response = await rustdeskConfigApi.regeneratePassword()
    rustdeskConfig.value = response
    return response
  }

  return {
    auth,
    video,
    audio,
    hid,
    msd,
    stream,
    web,
    atx,
    rtspConfig,
    rtspStatus,
    rustdeskConfig,
    rustdeskStatus,
    rustdeskPassword,
    authLoading,
    videoLoading,
    audioLoading,
    hidLoading,
    msdLoading,
    streamLoading,
    webLoading,
    atxLoading,
    rtspLoading,
    rustdeskLoading,
    authError,
    videoError,
    audioError,
    hidError,
    msdError,
    streamError,
    webError,
    atxError,
    rtspError,
    rustdeskError,
    refreshAuth,
    refreshVideo,
    refreshAudio,
    refreshHid,
    refreshMsd,
    refreshStream,
    refreshWeb,
    refreshAtx,
    refreshRtspConfig,
    refreshRtspStatus,
    refreshRustdeskConfig,
    refreshRustdeskStatus,
    refreshRustdeskPassword,
    ensureAuth,
    ensureVideo,
    ensureAudio,
    ensureHid,
    ensureMsd,
    ensureStream,
    ensureWeb,
    ensureAtx,
    ensureRtspConfig,
    ensureRustdeskConfig,
    updateAuth,
    updateVideo,
    updateAudio,
    updateHid,
    updateMsd,
    updateStream,
    updateWeb,
    updateAtx,
    updateRtsp,
    updateRustdesk,
    regenerateRustdeskId,
    regenerateRustdeskPassword,
  }
})
