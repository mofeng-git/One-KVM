<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ExternalLink, Terminal, X } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const { t } = useI18n()

function openTerminalInNewTab() {
  window.open('/api/terminal/', '_blank')
}

function hideFrameScrollbars(event: Event) {
  const frame = event.currentTarget as HTMLIFrameElement | null
  try {
    const doc = frame?.contentDocument
    if (!doc) return

    const styleId = 'one-kvm-terminal-scrollbar-style'
    let style = doc.getElementById(styleId) as HTMLStyleElement | null
    if (!style) {
      style = doc.createElement('style')
      style.id = styleId
      const parent = doc.head || doc.documentElement
      parent.appendChild(style)
    }

    style.textContent = `
      html,
      body,
      .xterm-viewport {
        -ms-overflow-style: none !important;
        scrollbar-width: none !important;
      }

      html::-webkit-scrollbar,
      body::-webkit-scrollbar,
      .xterm-viewport::-webkit-scrollbar {
        width: 0 !important;
        height: 0 !important;
        display: none !important;
      }
    `
  } catch {
    // The terminal is served from the same origin; keep the dialog usable if a browser blocks access.
  }
}
</script>

<template>
  <Dialog :open="props.open" @update:open="emit('update:open', $event)">
    <DialogContent
      :show-close-button="false"
      class="w-[98vw] sm:w-[95vw] max-w-5xl h-[90dvh] sm:h-[85dvh] max-h-[720px] p-0 flex flex-col gap-0 overflow-hidden"
    >
      <DialogHeader class="px-3 sm:px-4 py-1 border-b shrink-0">
        <DialogTitle class="flex h-8 items-center justify-between w-full">
          <div class="flex items-center gap-2 min-w-0">
            <Terminal class="size-4 shrink-0" />
            <span class="truncate text-sm font-semibold">{{ t('extensions.ttyd.title') }}</span>
          </div>
          <div class="flex items-center gap-1 shrink-0">
            <Button
              variant="ghost"
              size="icon-sm"
              @click="openTerminalInNewTab"
              :aria-label="t('extensions.ttyd.openInNewTab')"
              :title="t('extensions.ttyd.openInNewTab')"
            >
              <ExternalLink class="size-3.5" />
            </Button>
            <DialogClose as-child>
              <Button variant="ghost" size="icon-sm">
                <X class="size-3.5" />
                <span class="sr-only">Close</span>
              </Button>
            </DialogClose>
          </div>
        </DialogTitle>
      </DialogHeader>
      <div class="flex-1 min-h-0">
        <iframe
          v-if="props.open"
          src="/api/terminal/"
          class="block w-full h-full border-0"
          allow="clipboard-read; clipboard-write"
          scrolling="no"
          @load="hideFrameScrollbars"
        />
      </div>
    </DialogContent>
  </Dialog>
</template>
