import { useLocalStorage } from '@vueuse/core'
import type { RemovableRef } from '@vueuse/core'

export type FeatureVisibilityKey = 'webTerminal' | 'computerUse' | 'pasteText'
export type FeatureVisibility = Record<FeatureVisibilityKey, boolean>

const DEFAULT_FEATURE_VISIBILITY: FeatureVisibility = {
  webTerminal: true,
  computerUse: true,
  pasteText: true,
}

const featureVisibility = useLocalStorage<FeatureVisibility>(
  'featureVisibility',
  DEFAULT_FEATURE_VISIBILITY,
  { mergeDefaults: true },
)

export function useFeatureVisibility(): RemovableRef<FeatureVisibility> {
  return featureVisibility
}
