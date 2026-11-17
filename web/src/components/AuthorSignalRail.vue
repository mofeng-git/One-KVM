<script setup lang="ts">
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
</script>

<template>
  <section
    class="signal-rail overflow-hidden rounded-[28px] border border-white/10 bg-[#070b10] shadow-[0_24px_70px_-28px_rgba(16,185,129,0.45)]"
    :aria-label="t('authorPage.signal.status')"
  >
    <div
      class="flex items-center justify-between gap-4 border-b border-white/5 px-5 py-3 font-mono text-[10px] tracking-[0.24em] text-emerald-300/80 uppercase"
    >
      <span class="flex items-center gap-2">
        <span class="relative flex size-2">
          <span
            class="absolute inline-flex size-full animate-ping rounded-full bg-emerald-400 opacity-60 motion-reduce:animate-none"
          />
          <span class="relative inline-flex size-2 rounded-full bg-emerald-400" />
        </span>
        {{ t('authorPage.signal.status') }}
      </span>
      <span class="hidden text-white/35 sm:inline">{{ t('authorPage.signal.hint') }}</span>
    </div>

    <div class="relative px-3 pt-1 pb-4 sm:px-5 sm:pb-6">
      <svg viewBox="0 0 1000 170" class="w-full" aria-hidden="true">
        <defs>
          <linearGradient id="signal-gradient" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0%" stop-color="#34d399" />
            <stop offset="55%" stop-color="#22d3ee" />
            <stop offset="100%" stop-color="#818cf8" />
          </linearGradient>
          <filter id="signal-glow" x="-20%" y="-50%" width="140%" height="200%">
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        <g stroke="rgba(255,255,255,0.045)" stroke-width="1">
          <path v-for="x in 20" :key="`signal-v-${x}`" :d="`M ${x * 50} 0 V 170`" />
          <path v-for="y in [1, 2, 3]" :key="`signal-h-${y}`" :d="`M 0 ${y * 42.5} H 1000`" />
        </g>

        <path
          class="signal-trace"
          d="M 60 95 H 250 C 290 95 300 55 345 55 H 610 C 655 55 665 95 705 95 H 940"
        />
        <path
          class="signal-trace signal-trace-alt"
          d="M 60 95 H 200 C 250 95 260 130 310 130 H 560 C 610 130 620 95 670 95 H 940"
        />
        <path
          class="signal-trace signal-trace-faint"
          d="M 60 95 H 330 C 370 95 380 20 430 20 H 700 C 745 20 755 95 800 95 H 940"
        />

        <g class="signal-node">
          <rect x="42" y="77" width="36" height="36" rx="8" />
          <rect x="54" y="89" width="12" height="12" rx="2" class="signal-chip-core" />
        </g>
        <g class="signal-node">
          <circle cx="480" cy="95" r="17" />
          <circle cx="480" cy="95" r="5" class="signal-chip-core" />
        </g>
        <g class="signal-node">
          <path d="M 920 77 l 30 18 -30 18 -30 -18 Z" />
          <circle cx="920" cy="95" r="4.5" class="signal-chip-core" />
        </g>
      </svg>

      <div
        class="grid grid-cols-3 gap-2 px-2 pb-1 font-mono text-[10px] tracking-[0.24em] text-white/55 uppercase sm:px-6"
      >
        <span>{{ t('authorPage.signal.chip') }}</span>
        <span class="text-center">{{ t('authorPage.signal.device') }}</span>
        <span class="text-right">{{ t('authorPage.signal.cloud') }}</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.signal-trace {
  fill: none;
  stroke: url(#signal-gradient);
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-dasharray: 7 13;
  opacity: 0.9;
  filter: url(#signal-glow);
  animation: signal-flow 2.8s linear infinite;
}

.signal-trace-alt {
  opacity: 0.55;
  animation-duration: 4.2s;
}

.signal-trace-faint {
  stroke-width: 1;
  opacity: 0.3;
  animation-duration: 6.5s;
}

.signal-node rect,
.signal-node circle,
.signal-node path {
  fill: rgb(7 11 16 / 0.92);
  stroke: #34d399;
  stroke-width: 1.2;
}

.signal-chip-core {
  fill: #34d399;
  stroke: none;
  filter: url(#signal-glow);
}

@keyframes signal-flow {
  to {
    stroke-dashoffset: -100;
  }
}

@media (prefers-reduced-motion: reduce) {
  .signal-trace {
    animation: none;
  }
}
</style>
