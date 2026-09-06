<script setup lang="ts">
// Static orthographic line art; coordinates are shared by all wiring diagrams.
defineProps<{ kind: 'kvm' | 'computer' }>()
</script>
<template>
  <g fill="none" stroke="currentColor" stroke-width="1.35" stroke-linejoin="round" stroke-linecap="round">
    <template v-if="kind === 'kvm'">
      <path d="M0 42 66 12 108 35 42 65Z" class="device-face" />
      <path d="M0 42v22l42 23V65m0 22 66-30V35" />
      <path d="m9 48 25 14v12L9 60Z" class="device-detail" />
      <path d="m51 65 17-8v9l-17 8Zm23-11 17-8v9l-17 8Z" />
      <path d="m20 39 33-15m-25 19 33-15m-25 19 33-15m-25 19 33-15" class="device-detail" />
      <path d="m7 68 4 2m24 13 4 2m14-1 4-2m39-17 4-2" class="device-detail" />
      <circle cx="98" cy="46" r="1.5" class="device-indicator" />
    </template>
    <template v-else-if="kind === 'computer'">
      <path d="M19 4 100 22v59L19 63Z" class="device-face" />
      <path d="m19 4 5-3 81 18v59l-5 3M100 22l5-3" />
      <path d="m25 13 69 15v43L25 56Z" class="device-detail" />
      <path d="M19 63 0 83l81 19 19-21" class="device-face" />
      <path d="M0 83v4l81 19 19-21v-4m-19 21v4" />
      <path d="m24 70 62 14-7 8-62-14Zm-1 5 59 13M36 73l-6 7m18-4-6 7m18-4-6 7m18-4-6 7" class="device-detail" />
      <path d="m34 85 20 5-4 4-20-5Z" class="device-detail" />
      <!-- USB socket on the laptop's left side plane. -->
      <path d="m8 75 7-7v3l-7 7Z" stroke-width="1.6" />
    </template>
  </g>
</template>
<style scoped>
.device-face { fill: var(--background); }
.device-detail { opacity: .4; }
.device-indicator { fill: currentColor; stroke: none; }
</style>
