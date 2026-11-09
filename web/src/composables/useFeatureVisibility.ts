import { useLocalStorage } from '@vueuse/core'
import type { RemovableRef } from '@vueuse/core'

export type FeatureVisibilityKey = 'power' | 'webTerminal' | 'pasteText' | 'airferry'
export type FeatureVisibility = Record<FeatureVisibilityKey, boolean>

const DEFAULT_FEATURE_VISIBILITY: FeatureVisibility = {
  power: true,
  webTerminal: true,
  pasteText: true,
  airferry: true,
}

const featureVisibility = useLocalStorage<FeatureVisibility>(
  'featureVisibility',
  DEFAULT_FEATURE_VISIBILITY,
  { mergeDefaults: true },
)

export function useFeatureVisibility(): RemovableRef<FeatureVisibility> {
  return featureVisibility
}
