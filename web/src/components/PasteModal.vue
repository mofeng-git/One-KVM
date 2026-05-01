<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import { CornerDownLeft, Square, AlertCircle } from 'lucide-vue-next'
import { charToKey, analyzeText } from '@/lib/charToHid'
import { hidApi } from '@/api'

const emit = defineEmits<{
  (e: 'close'): void
}>()

const { t } = useI18n()

const text = ref('')
const textareaRef = ref<HTMLTextAreaElement | null>(null)
const isPasting = ref(false)
const progress = ref(0)
const currentChar = ref(0)
const totalChars = ref(0)
const abortController = ref<AbortController | null>(null)

const typingDelay = ref(10)

const textAnalysis = computed(() => {
  if (!text.value) return null
  return analyzeText(text.value)
})

const hasUntypableChars = computed(() => {
  return textAnalysis.value && textAnalysis.value.untypable > 0
})

onMounted(() => {
  setTimeout(() => {
    textareaRef.value?.focus()
  }, 100)
})

onUnmounted(() => {
  cancelPaste()
})

/**
 * Sleep utility function
 */
function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

/**
 * Type a single character via HID
 * Sends keydown then keyup events with appropriate modifiers
 */
async function typeChar(char: string, signal: AbortSignal): Promise<boolean> {
  if (signal.aborted) return false

  const mapping = charToKey(char)
  if (!mapping) {
    return true
  }

  const { key, shift } = mapping
  const modifier = shift ? 0x02 : 0

  try {
    await hidApi.keyboard('down', key, modifier)

    await sleep(5)

    if (signal.aborted) {
      // Even if aborted, still send keyup to release the key
      await hidApi.keyboard('up', key, modifier)
      return false
    }

    await hidApi.keyboard('up', key, modifier)

    await sleep(2)

    return true
  } catch (error) {
    console.error('[Paste] Failed to type character:', char, error)
    try {
      await hidApi.keyboard('up', key, modifier)
    } catch {
    }
    return false
  }
}

/**
 * Main paste function - types all characters sequentially
 */
async function handlePaste() {
  const textToType = text.value
  if (!textToType.trim()) return

  isPasting.value = true
  progress.value = 0
  currentChar.value = 0
  totalChars.value = textToType.length

  // Create abort controller for cancellation
  abortController.value = new AbortController()
  const signal = abortController.value.signal

  try {
    const chars = [...textToType] // Convert to array for proper iteration
    const totalLength = chars.length
    let charIndex = 0
    for (const char of chars) {
      if (signal.aborted) {
        break
      }

      charIndex++
      currentChar.value = charIndex
      progress.value = Math.round((charIndex / totalLength) * 100)

      if (char === '\r' && charIndex < totalLength && chars[charIndex] === '\n') {
        continue
      }

      await typeChar(char, signal)

      if (typingDelay.value > 0 && charIndex < totalLength) {
        await sleep(typingDelay.value)
      }
    }

    if (!signal.aborted) {
      await sleep(200)
      text.value = ''
      emit('close')
    }
  } catch (error) {
    console.error('[Paste] Error during paste operation:', error)
  } finally {
    try {
      await hidApi.reset()
    } catch {
    }
    isPasting.value = false
    progress.value = 0
    currentChar.value = 0
    totalChars.value = 0
    abortController.value = null
  }
}

/**
 * Cancel ongoing paste operation
 */
function cancelPaste() {
  if (abortController.value) {
    abortController.value.abort()
    abortController.value = null
  }
}

function handleKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
    e.preventDefault()
    if (!isPasting.value) {
      handlePaste()
    }
  }
  if (e.key === 'Escape') {
    e.preventDefault()
    if (isPasting.value) {
      cancelPaste()
    } else {
      emit('close')
    }
  }
  e.stopPropagation()
}
</script>

<template>
  <div class="p-4 space-y-4">
    <div class="space-y-1">
      <h3 class="font-semibold text-sm">{{ t('paste.title') }}</h3>
      <p class="text-xs text-muted-foreground">{{ t('paste.description') }}</p>
    </div>

    <div class="space-y-2">
      <Label for="paste-text">{{ t('paste.label') }}</Label>
      <Textarea
        id="paste-text"
        ref="textareaRef"
        v-model="text"
        :placeholder="t('paste.placeholder')"
        class="min-h-[120px] resize-none font-mono text-sm"
        :disabled="isPasting"
        @keydown="handleKeydown"
        @keyup.stop
      />
    </div>

    <!-- Warning for untypable characters -->
    <div v-if="hasUntypableChars && !isPasting" class="flex items-start gap-2 p-2 rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400">
      <AlertCircle class="h-4 w-4 shrink-0 mt-0.5" />
      <div class="text-xs">
        <p class="font-medium">{{ t('paste.untypableWarning') }}</p>
        <p class="text-muted-foreground mt-0.5">
          {{ t('paste.untypableChars', { chars: textAnalysis?.untypableChars.slice(0, 5).map(c => c === '\n' ? '\\n' : c === '\r' ? '\\r' : c === '\t' ? '\\t' : c).join(', ') }) }}
          <span v-if="textAnalysis && textAnalysis.untypableChars.length > 5">...</span>
        </p>
      </div>
    </div>

    <!-- Progress indicator during paste -->
    <div v-if="isPasting" class="space-y-2">
      <div class="flex items-center justify-between text-xs text-muted-foreground">
        <span>{{ t('paste.typing') }}</span>
        <span>{{ currentChar }} / {{ totalChars }}</span>
      </div>
      <Progress :model-value="progress" class="h-2" />
    </div>

    <div class="flex items-center justify-between">
      <p v-if="!isPasting" class="text-xs text-muted-foreground">
        {{ t('paste.hint') }}
      </p>
      <p v-else class="text-xs text-muted-foreground">
        {{ t('paste.escToCancel') }}
      </p>
      <div class="flex gap-2">
        <Button v-if="!isPasting" variant="ghost" size="sm" @click="emit('close')">
          {{ t('common.cancel') }}
        </Button>
        <Button v-else variant="ghost" size="sm" @click="cancelPaste">
          <Square class="h-3 w-3 mr-1.5 fill-current" />
          {{ t('paste.stop') }}
        </Button>
        <Button
          size="sm"
          :disabled="!text.trim() || isPasting"
          @click="handlePaste"
        >
          <CornerDownLeft class="h-4 w-4 mr-1.5" />
          {{ t('paste.confirm') }}
        </Button>
      </div>
    </div>
  </div>
</template>
