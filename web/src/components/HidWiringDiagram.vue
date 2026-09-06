<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { Driver } from '@/lib/hidGuide'
import HidWireframeDevice from './HidWireframeDevice.vue'

defineProps<{ backend: Driver }>()
const { t } = useI18n()
</script>
<template>
  <figure v-if="backend !== 'none'" class="wiring-diagram rounded-lg border border-border/60 bg-muted/20 p-3 space-y-2">
    <svg viewBox="0 0 460 184" class="block w-full text-foreground" role="img" :aria-label="t(`hidGuide.wiring_${backend}`)">
      <!-- A faint ground plane anchors the perspective without shadows or filters. -->
      <g fill="none" stroke="currentColor" stroke-width=".65" opacity=".07" aria-hidden="true">
        <path d="m7 87 101-46 75 38-101 46Zm270-5 98-45 79 39-98 46M31 99l101-46m-77 58 101-46M302 94l98-45m-71 58 98-45" />
      </g>
      <HidWireframeDevice kind="kvm" transform="translate(29 10)" />
      <HidWireframeDevice kind="computer" transform="translate(322 0)" />
      <g class="device-label" fill="currentColor" text-anchor="middle">
        <text x="83" y="120">{{ backend === 'bluetooth' ? 'One-KVM HID' : backend === 'otg' ? t('hidGuide.otgPort') : 'One-KVM USB' }}</text>
        <text x="375" y="120">{{ backend === 'bluetooth' ? t('hidGuide.host') : t('hidGuide.hostUsb') }}</text>
      </g>

      <g v-if="backend === 'otg'" class="connection" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round">
        <!-- Rectangular USB plugs terminate on the two side-mounted sockets. -->
        <g class="usb-plug">
          <path d="m112 61 9-4 10 5-9 4Z" />
          <path d="m112 61v7l10 5v-7Z" />
          <path d="m122 66 9-4v7l-9 4Z" />
          <path d="m112 64-5 2" class="plug-blade" />
        </g>
        <g class="usb-plug">
          <path d="m317 80 7-7 13-5-7 7Z" />
          <path d="m317 80 13-5v3l-13 5Z" />
          <path d="m324 73 13-5v3l-13 5Z" />
          <path d="m330 75 7-7v3l-7 7Z" class="plug-blade" />
        </g>
        <path d="M131 68c27 11 29 29 55 40l27 12c34 15 66 9 82-8l22-31" />
        <path d="m223 117 7 3-7 3" />
        <path d="M230 129v12" class="leader" />
      </g>
      <text v-if="backend === 'otg'" x="230" y="162" class="connection-label" text-anchor="middle" fill="currentColor">{{ t('hidGuide.dataCable') }}</text>

      <template v-else-if="backend === 'bluetooth'">
        <g class="connection" fill="none" stroke="currentColor" stroke-linecap="round">
          <path d="M137 48c9 8 9 22 0 30m9-39c15 13 15 35 0 48m164-37c-9 8-9 22 0 30m-9-39c-15 13-15 35 0 48" opacity=".45" />
          <path d="M166 64h30m64 0h28" stroke-dasharray="2 6" />
          <path d="m216 48 23 32-13 10V39l13 10-23 31" stroke-width="1.8" />
        </g>
        <text x="230" y="162" class="connection-label" text-anchor="middle" fill="currentColor">{{ t('hidGuide.wireless') }}</text>
      </template>

      <template v-else-if="backend === 'ch9329'">
        <!-- CH340 and CH9329 are enclosed in the cable and have no visible module. -->
        <g class="connection" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round">
          <g class="usb-plug">
            <path d="m112 61 9-4 10 5-9 4Z" />
            <path d="m112 61v7l10 5v-7Z" />
            <path d="m122 66 9-4v7l-9 4Z" />
            <path d="m112 64-5 2" class="plug-blade" />
          </g>
          <g class="usb-plug">
            <path d="m317 80 7-7 13-5-7 7Z" />
            <path d="m317 80 13-5v3l-13 5Z" />
            <path d="m324 73 13-5v3l-13 5Z" />
            <path d="m330 75 7-7v3l-7 7Z" class="plug-blade" />
          </g>
          <path d="M131 68c25 10 32 39 67 55 33 16 72 19 100-4l19-38" />
          <path d="m224 132 7 2-5 5" />
          <path d="M230 143v7" class="leader" />
        </g>
        <text x="230" y="169" class="connection-label" text-anchor="middle" fill="currentColor">{{ t('hidGuide.integratedCable') }}</text>
      </template>
    </svg>
    <figcaption class="text-xs leading-relaxed text-muted-foreground">{{ t(`hidGuide.wiring_${backend}`) }}</figcaption>
  </figure>
  <p v-else class="text-sm text-muted-foreground">{{ t('hidGuide.disabledHelp') }}</p>
</template>
<style scoped>
.device-label { font-size: 13px; font-weight: 500; }
.connection { color: var(--wiring-accent); stroke-width: 1.8; }
.connection-label { color: var(--wiring-accent); font-size: 12px; }
.usb-plug { fill: var(--background); stroke-width: 1.25; }
.plug-blade { fill: none; opacity: .75; }
.leader { opacity: .4; stroke-width: 1; }
.wiring-diagram { --wiring-accent: #0369a1; }
:global(.dark .wiring-diagram) { --wiring-accent: #7dd3fc; }
</style>
