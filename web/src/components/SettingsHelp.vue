<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import { onClickOutside, onKeyStroke } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { activeSettingsHelp } from '@/lib/settings-help'
import { HelpCircle } from 'lucide-vue-next'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'

defineProps<{ content: string }>()
const { t } = useI18n()
const trigger = ref<HTMLElement>()
const open = ref(false)
const pinned = ref(false)
const identity = Symbol('settings-help')
function close() {
  pinned.value = false
  open.value = false
  if (activeSettingsHelp.value === identity) activeSettingsHelp.value = null
}
function show(value: boolean) {
  if (value) {
    activeSettingsHelp.value = identity
    open.value = true
  } else if (!pinned.value) close()
}
function toggle() {
  if (pinned.value) close()
  else {
    pinned.value = true
    show(true)
  }
}
watch(activeSettingsHelp, (value) => {
  if (value !== identity) close()
})
onUnmounted(close)
onClickOutside(trigger, close)
onKeyStroke('Escape', close)
</script>

<template>
  <span ref="trigger" class="settings-help">
    <TooltipProvider :delay-duration="150">
      <Tooltip :open="open" @update:open="show">
        <TooltipTrigger as-child>
          <button
            type="button"
            :aria-label="t('settingsUi.help')"
            :aria-expanded="open"
            @click.stop="toggle"
          >
            <HelpCircle aria-hidden="true" />
          </button>
        </TooltipTrigger>
        <TooltipContent
          :show-arrow="false"
          side="bottom"
          align="start"
          class="settings-help-content"
        >
          {{ content }}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  </span>
</template>
