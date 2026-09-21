<script setup lang="ts">
import type { ConsoleLayout } from '@/composables/useConsoleLayout'

defineProps<{ layout: ConsoleLayout }>()
</script>

<template>
  <svg
    viewBox="0 0 240 96"
    class="block h-24 w-full rounded-md border bg-background text-foreground"
    aria-hidden="true"
  >
    <defs>
      <linearGradient :id="`console-preview-${layout}`" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="currentColor" stop-opacity=".08" />
        <stop offset="1" stop-color="currentColor" stop-opacity=".02" />
      </linearGradient>
    </defs>

    <rect x=".5" y=".5" width="239" height="95" rx="7" fill="currentColor" opacity=".035" />
    <path d="M1 7.5A7 7 0 0 1 8 .5h224a7 7 0 0 1 7 7" fill="none" stroke="currentColor" opacity=".12" />
    <circle cx="10" cy="7" r="2" fill="#ef4444" opacity=".75" />
    <circle cx="17" cy="7" r="2" fill="#f59e0b" opacity=".75" />
    <circle cx="24" cy="7" r="2" fill="#22c55e" opacity=".75" />

    <rect
      :x="layout === 'sidebar' ? 32 : 8"
      y="14"
      :width="layout === 'sidebar' ? 200 : 224"
      height="74"
      rx="4"
      fill="#09090b"
    />
    <path
      :d="layout === 'sidebar' ? 'M46 82 93 36l31 28 25-21 38 39Z' : 'M24 82 80 36l36 29 28-22 48 39Z'"
      fill="none"
      stroke="#71717a"
      stroke-width="1"
      opacity=".3"
    />
    <rect
      :x="layout === 'sidebar' ? 40 : 16"
      y="22"
      :width="layout === 'sidebar' ? 184 : 208"
      height="58"
      rx="2"
      :fill="`url(#console-preview-${layout})`"
    />

    <g v-if="layout === 'current'">
      <rect x="8" y="14" width="224" height="16" rx="4" fill="var(--background)" />
      <path d="M8 26h224" stroke="currentColor" opacity=".12" />
      <circle cx="18" cy="22" r="3" fill="none" stroke="currentColor" opacity=".45" />
      <path d="M26 20h28m-28 4h18" stroke="currentColor" stroke-width="1.5" opacity=".34" />
      <g fill="none" stroke="currentColor" opacity=".5">
        <rect x="139" y="18" width="14" height="8" rx="2" />
        <path d="m160 19 4 3-4 3m8-6 4 3-4 3m10-6v6m7-6 5 6 5-6" />
        <rect x="202" y="18" width="20" height="8" rx="4" />
      </g>
    </g>

    <g v-else-if="layout === 'floating'">
      <rect x="50" y="19" width="140" height="18" rx="9" fill="var(--background)" stroke="currentColor" stroke-opacity=".2" />
      <circle cx="62" cy="28" r="4" fill="none" stroke="currentColor" opacity=".45" />
      <path d="M72 26h26m-26 4h18m18-6v8m10-8 6 8 6-8m10 1 6 6 6-6m11-1v8" fill="none" stroke="currentColor" opacity=".5" />
      <rect x="168" y="24" width="14" height="8" rx="4" fill="currentColor" opacity=".12" />
      <circle cx="177" cy="28" r="2" fill="#22c55e" />
      <path d="m114 84 6-4 6 4" fill="none" stroke="white" opacity=".45" />
    </g>

    <g v-else>
      <rect x="8" y="14" width="24" height="74" rx="4" fill="var(--background)" />
      <path d="M28 14v74" stroke="currentColor" opacity=".12" />
      <circle cx="20" cy="24" r="4" fill="none" stroke="currentColor" opacity=".5" />
      <g fill="none" stroke="currentColor" opacity=".52">
        <rect x="15" y="35" width="10" height="7" rx="2" />
        <path d="m16 52 4-4 4 4-4 4Zm0 11h8m-4-4v8" />
      </g>
      <circle cx="20" cy="81" r="2" fill="#22c55e" />
    </g>
  </svg>
</template>
