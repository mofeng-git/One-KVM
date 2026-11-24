import { defineStore } from 'pinia'
import { createConfigResource } from '@/lib/configResource'
import {
  authConfigApi,
  atxConfigApi,
  audioConfigApi,
  hidConfigApi,
  msdConfigApi,
  otgConfigApi,
  streamConfigApi,
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

  return {
    auth,
    video,
    audio,
    hid,
    msd,
    stream,
    web,
    atx,
    authLoading,
    videoLoading,
    audioLoading,
    hidLoading,
    msdLoading,
    streamLoading,
    webLoading,
    atxLoading,
    authError,
    videoError,
    audioError,
    hidError,
    msdError,
    streamError,
    webError,
    atxError,
    refreshAuth,
    refreshVideo,
    refreshAudio,
    refreshHid,
    refreshMsd,
    refreshStream,
    refreshWeb,
    refreshAtx,
    ensureAuth,
    ensureVideo,
    ensureAudio,
    ensureHid,
    ensureMsd,
    ensureStream,
    ensureWeb,
    ensureAtx,
    updateAuth,
    updateVideo,
    updateAudio,
    updateHid,
    updateMsd,
    updateOtg,
    updateStream,
    updateWeb,
    updateAtx,
  }
})
