import { defineStore } from 'pinia'
import { createConfigResource, createRequestState } from '@/lib/configResource'
import {
  authConfigApi,
  atxConfigApi,
  audioConfigApi,
  hidConfigApi,
  msdConfigApi,
  otgConfigApi,
  rtspConfigApi,
  rustdeskConfigApi,
  streamConfigApi,
  vncConfigApi,
  videoConfigApi,
  webConfigApi,
} from '@/api'
import type {
  AtxConfigUpdate,
  AudioConfigUpdate,
  AuthConfigUpdate,
  HidConfigUpdate,
  MsdConfigUpdate,
  OtgConfigResponse,
  OtgConfigUpdate,
  StreamConfigUpdate,
  VideoConfigUpdate,
} from '@/types/generated'
import type {
  RtspConfigUpdate as ApiRtspConfigUpdate,
  RustDeskConfigUpdate as ApiRustDeskConfigUpdate,
  VncConfigUpdate as ApiVncConfigUpdate,
  WebConfigUpdate,
} from '@/api'

export const useConfigStore = defineStore('config', () => {
  const {
    data: auth, loading: authLoading, error: authError,
    refresh: refreshAuth, ensure: ensureAuth,
  } = createConfigResource(authConfigApi.get)
  const {
    data: video, loading: videoLoading, error: videoError,
    refresh: refreshVideo, ensure: ensureVideo,
  } = createConfigResource(videoConfigApi.get)
  const {
    data: audio, loading: audioLoading, error: audioError,
    refresh: refreshAudio, ensure: ensureAudio,
  } = createConfigResource(audioConfigApi.get)
  const {
    data: hid, loading: hidLoading, error: hidError,
    refresh: refreshHid, ensure: ensureHid,
  } = createConfigResource(hidConfigApi.get)
  const {
    data: msd, loading: msdLoading, error: msdError,
    refresh: refreshMsd, ensure: ensureMsd,
  } = createConfigResource(msdConfigApi.get)
  const {
    data: stream, loading: streamLoading, error: streamError,
    refresh: refreshStream, ensure: ensureStream,
  } = createConfigResource(streamConfigApi.get)
  const {
    data: web, loading: webLoading, error: webError,
    refresh: refreshWeb, ensure: ensureWeb,
  } = createConfigResource(webConfigApi.get)
  const {
    data: atx, loading: atxLoading, error: atxError,
    refresh: refreshAtx, ensure: ensureAtx,
  } = createConfigResource(atxConfigApi.get)

  const rtspRequests = createRequestState()
  const { loading: rtspLoading, error: rtspError } = rtspRequests
  const { data: rtspConfig, refresh: refreshRtspConfig, ensure: ensureRtspConfig } =
    createConfigResource(rtspConfigApi.get, rtspRequests)
  const { data: rtspStatus, refresh: refreshRtspStatus } = createConfigResource(
    rtspConfigApi.getStatus, rtspRequests, (response) => {
      rtspConfig.value = response.config
    },
  )

  const vncRequests = createRequestState()
  const { loading: vncLoading, error: vncError } = vncRequests
  const { data: vncConfig, refresh: refreshVncConfig, ensure: ensureVncConfig } =
    createConfigResource(vncConfigApi.get, vncRequests)
  const { data: vncStatus, refresh: refreshVncStatus } = createConfigResource(
    vncConfigApi.getStatus, vncRequests, (response) => {
      vncConfig.value = response.config
    },
  )

  const rustdeskRequests = createRequestState()
  const { loading: rustdeskLoading, error: rustdeskError } = rustdeskRequests
  const { data: rustdeskConfig, refresh: refreshRustdeskConfig, ensure: ensureRustdeskConfig } =
    createConfigResource(rustdeskConfigApi.get, rustdeskRequests)
  const { data: rustdeskStatus, refresh: refreshRustdeskStatus } = createConfigResource(
    rustdeskConfigApi.getStatus, rustdeskRequests, (response) => {
      rustdeskConfig.value = response.config
    },
  )
  const { data: rustdeskPassword, refresh: refreshRustdeskPassword } =
    createConfigResource(rustdeskConfigApi.getPassword, rustdeskRequests)

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

  async function updateHid(update: HidConfigUpdate, signal?: AbortSignal) {
    const response = await hidConfigApi.update(update, signal)
    hid.value = response
    return response
  }

  async function updateMsd(update: MsdConfigUpdate) {
    const response = await msdConfigApi.update(update)
    msd.value = response
    return response
  }

  async function updateOtg(update: OtgConfigUpdate): Promise<OtgConfigResponse> {
    const response = await otgConfigApi.update(update)
    hid.value = response.hid
    msd.value = response.msd
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

  async function updateVnc(update: ApiVncConfigUpdate) {
    const response = await vncConfigApi.update(update)
    vncConfig.value = response
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
    vncConfig,
    vncStatus,
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
    vncLoading,
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
    vncError,
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
    refreshVncConfig,
    refreshVncStatus,
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
    ensureVncConfig,
    ensureRustdeskConfig,
    updateAuth,
    updateVideo,
    updateAudio,
    updateHid,
    updateMsd,
    updateOtg,
    updateStream,
    updateWeb,
    updateAtx,
    updateRtsp,
    updateVnc,
    updateRustdesk,
    regenerateRustdeskId,
    regenerateRustdeskPassword,
  }
})
