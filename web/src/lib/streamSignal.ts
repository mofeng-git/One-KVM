import type { StreamDeviceLostEventData } from '@/types/websocket'

const AUDIO_STATE_REASONS = new Set(['audio_device_lost', 'audio_reconnecting'])

export function isAudioDeviceLostStateReason(reason: string | null | undefined): boolean {
  return typeof reason === 'string' && AUDIO_STATE_REASONS.has(reason)
}

export function isAudioStreamDeviceLostPayload(data: StreamDeviceLostEventData): boolean {
  return data.kind === 'audio'
}
