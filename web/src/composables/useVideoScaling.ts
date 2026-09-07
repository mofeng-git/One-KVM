import { computed, nextTick, ref, watch } from 'vue'
import type { CSSProperties, Ref } from 'vue'
import { useElementSize } from '@vueuse/core'

export type VideoScaleMode = 'fit' | 'actual'
export type VideoRotation = 0 | 90 | 180 | 270

export interface VideoSize {
  width: number
  height: number
}

export function useVideoScaling(options: { rotation?: Readonly<Ref<VideoRotation>> } = {}) {
  const workspaceRef = ref<HTMLDivElement | null>(null)
  const scaleMode = ref<VideoScaleMode>('fit')
  const sourceSize = ref<VideoSize | null>(null)
  const rotation = options.rotation ?? ref<VideoRotation>(0)
  const { width: workspaceWidth, height: workspaceHeight } = useElementSize(workspaceRef)

  const sourceSizeAvailable = computed(() => sourceSize.value !== null)
  const effectiveScaleMode = computed<VideoScaleMode>(() => (
    scaleMode.value === 'actual' && sourceSizeAvailable.value ? 'actual' : 'fit'
  ))

  const hasQuarterTurn = computed(() => rotation.value === 90 || rotation.value === 270)
  const rotatedSourceSize = computed<VideoSize | null>(() => {
    const source = sourceSize.value
    if (!source) return null

    return hasQuarterTurn.value
      ? { width: source.height, height: source.width }
      : source
  })

  const fittedSize = computed<VideoSize | null>(() => {
    const source = rotatedSourceSize.value
    if (!source || workspaceWidth.value <= 0 || workspaceHeight.value <= 0) return null

    const scale = Math.min(
      workspaceWidth.value / source.width,
      workspaceHeight.value / source.height,
    )
    return {
      width: Math.max(1, Math.floor(source.width * scale)),
      height: Math.max(1, Math.floor(source.height * scale)),
    }
  })

  const stageClass = computed(() => effectiveScaleMode.value === 'actual'
    ? 'h-max w-max min-h-full min-w-full'
    : 'h-full w-full'
  )

  const containerStyle = computed<CSSProperties>(() => {
    const size = effectiveScaleMode.value === 'actual' ? rotatedSourceSize.value : fittedSize.value
    if (size) {
      return {
        width: `${size.width}px`,
        height: `${size.height}px`,
      }
    }

    return {
      width: '100%',
      height: '100%',
      minHeight: '120px',
    }
  })

  // A quarter turn swaps the displayed dimensions. Keep the video itself at
  // its unrotated dimensions, then rotate it inside the correctly sized frame.
  const contentStyle = computed<CSSProperties>(() => {
    const size = effectiveScaleMode.value === 'actual' ? rotatedSourceSize.value : fittedSize.value
    const quarterTurn = hasQuarterTurn.value

    return {
      width: size ? `${quarterTurn ? size.height : size.width}px` : '100%',
      height: size ? `${quarterTurn ? size.width : size.height}px` : '100%',
      transform: `rotate(${rotation.value}deg)`,
    }
  })

  function updateSourceSize(width: number, height: number) {
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return

    const nextSize = { width: Math.round(width), height: Math.round(height) }
    if (sourceSize.value?.width === nextSize.width && sourceSize.value.height === nextSize.height) return
    sourceSize.value = nextSize
  }

  function clearSourceSize() {
    sourceSize.value = null
  }

  function setScaleMode(mode: VideoScaleMode) {
    if (mode === 'actual' && !sourceSizeAvailable.value) return
    scaleMode.value = mode
  }

  watch(
    [effectiveScaleMode, () => sourceSize.value?.width, () => sourceSize.value?.height],
    async ([mode]) => {
      if (mode !== 'actual') return
      await nextTick()
      workspaceRef.value?.scrollTo({ left: 0, top: 0 })
    },
  )

  return {
    workspaceRef,
    scaleMode,
    sourceSize,
    sourceSizeAvailable,
    stageClass,
    containerStyle,
    contentStyle,
    updateSourceSize,
    clearSourceSize,
    setScaleMode,
  }
}
