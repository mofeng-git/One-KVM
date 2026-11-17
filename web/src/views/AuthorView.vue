<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, type Directive } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ArrowLeft,
  BadgeCheck,
  Cloud,
  Cpu,
  ExternalLink,
  GitBranch,
  Layers,
  MonitorSmartphone,
  Network,
  PackageCheck,
  QrCode,
  Scale,
  ShieldCheck,
  Sparkles,
} from 'lucide-vue-next'
import { useSystemStore } from '@/stores/system'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import AuthorSignalRail from '@/components/AuthorSignalRail.vue'

const { t } = useI18n()
const router = useRouter()
const systemStore = useSystemStore()

const ORIGINAL_AUTHOR_URL = 'https://github.com/mofeng-git/One-KVM'
const SOURCE_URL = 'https://github.com/zoneBen/One-KVM'
const LICENSE_URL = 'https://www.gnu.org/licenses/agpl-3.0.html'
const ISSUES_URL = `${ORIGINAL_AUTHOR_URL}/issues`

const versionLabel = computed(() => (systemStore.version ? `v${systemStore.version}` : 'One-KVM'))

const tagKeys = ['embedded', 'bus', 'desktop', 'acquisition', 'cloud', 'localization'] as const

const capabilityItems = [
  { key: 'embedded', icon: Cpu, tone: 'bg-success/10 text-success ring-success/20' },
  { key: 'bus', icon: Network, tone: 'bg-info/10 text-info ring-info/20' },
  { key: 'host', icon: MonitorSmartphone, tone: 'bg-warning/10 text-warning ring-warning/20' },
  { key: 'cloud', icon: Cloud, tone: 'bg-primary/10 text-primary ring-primary/20' },
] as const

const contactItemKeys = ['analysis', 'prototype', 'maintenance'] as const

const ecosystemColumns = [
  { key: 'os', icon: Layers, items: ['kylin', 'uos', 'openEuler', 'nfs'] },
  { key: 'cpu', icon: Cpu, items: ['loongarch', 'phtium', 'kunpeng', 'hygon'] },
  { key: 'delivery', icon: PackageCheck, items: ['cross', 'package', 'debug', 'private'] },
] as const

const licenseLinks = computed(() => [
  { label: t('authorPage.license.links.author'), href: ORIGINAL_AUTHOR_URL },
  { label: t('authorPage.license.links.source'), href: SOURCE_URL },
  { label: t('authorPage.license.links.license'), href: LICENSE_URL },
  { label: t('authorPage.license.links.issues'), href: ISSUES_URL },
])

const pageRef = ref<HTMLElement | null>(null)
const prefersReducedMotion =
  typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
let spotlightFrame = 0

function goBack() {
  router.push({ name: 'Settings', query: { tab: 'about' } })
}

function updateSpotlight(event: PointerEvent) {
  if (prefersReducedMotion || event.pointerType !== 'mouse' || !pageRef.value || spotlightFrame) {
    return
  }

  const { clientX, clientY } = event
  spotlightFrame = window.requestAnimationFrame(() => {
    spotlightFrame = 0
    const page = pageRef.value
    if (!page) return
    page.style.setProperty('--spot-x', `${clientX}px`)
    page.style.setProperty('--spot-y', `${clientY}px`)
    page.style.setProperty('--spot-opacity', '1')
  })
}

function hideSpotlight() {
  pageRef.value?.style.setProperty('--spot-opacity', '0')
}

function handleTilt(event: PointerEvent) {
  if (prefersReducedMotion || event.pointerType !== 'mouse') return
  const target = event.currentTarget as HTMLElement
  const rect = target.getBoundingClientRect()
  const x = (event.clientX - rect.left) / rect.width
  const y = (event.clientY - rect.top) / rect.height
  target.style.setProperty('--tilt-x', `${(0.5 - y) * 5}deg`)
  target.style.setProperty('--tilt-y', `${(x - 0.5) * 7}deg`)
  target.style.setProperty('--spot-x', `${x * 100}%`)
  target.style.setProperty('--spot-y', `${y * 100}%`)
}

function resetTilt(event: PointerEvent) {
  const target = event.currentTarget as HTMLElement
  target.style.setProperty('--tilt-x', '0deg')
  target.style.setProperty('--tilt-y', '0deg')
}

const revealObservers = new WeakMap<HTMLElement, IntersectionObserver>()
const vReveal: Directive<HTMLElement> = {
  mounted(el) {
    if (prefersReducedMotion || typeof IntersectionObserver === 'undefined') {
      el.classList.add('is-visible')
      return
    }

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue
          el.classList.add('is-visible')
          observer.disconnect()
          revealObservers.delete(el)
        }
      },
      { threshold: 0.12, rootMargin: '0px 0px -8% 0px' },
    )
    revealObservers.set(el, observer)
    observer.observe(el)
  },
  unmounted(el) {
    revealObservers.get(el)?.disconnect()
    revealObservers.delete(el)
  },
}

onMounted(() => {
  if (!systemStore.version) {
    systemStore.fetchSystemInfo().catch(() => {})
  }
})

onBeforeUnmount(() => {
  if (spotlightFrame) window.cancelAnimationFrame(spotlightFrame)
  hideSpotlight()
})
</script>

<template>
  <div
    ref="pageRef"
    class="author-page"
    @pointermove="updateSpotlight"
    @pointerleave="hideSpotlight"
  >
    <div class="author-backdrop" aria-hidden="true" />
    <div class="author-spotlight" aria-hidden="true" />
    <div class="author-aurora author-aurora-a" aria-hidden="true" />
    <div class="author-aurora author-aurora-b" aria-hidden="true" />

    <header
      class="sticky top-0 z-30 border-b border-border/60 bg-background/80 backdrop-blur supports-[backdrop-filter]:bg-background/65"
    >
      <div class="mx-auto flex h-14 w-full max-w-7xl items-center justify-between gap-3 px-4 sm:px-6">
        <div class="flex min-w-0 items-center gap-2">
          <Button variant="outline" size="sm" class="h-8 rounded-full px-3 text-xs" @click="goBack">
            <ArrowLeft class="size-3.5" />
            {{ t('authorPage.back') }}
          </Button>
          <span class="hidden truncate text-sm text-muted-foreground sm:inline">
            {{ t('authorPage.breadcrumb') }}
          </span>
        </div>
        <Badge variant="outline" class="h-6 rounded-full px-2.5 font-mono text-[11px] text-muted-foreground">
          {{ versionLabel }}
        </Badge>
      </div>
    </header>

    <main class="relative z-10 mx-auto w-full max-w-7xl px-4 pt-10 pb-16 sm:px-6 lg:pt-16">
      <section class="author-hero grid items-start gap-10 lg:grid-cols-[minmax(0,1fr)_360px] lg:gap-16">
        <div class="min-w-0">
          <Badge
            variant="outline"
            class="h-7 rounded-full border-success/30 bg-success/5 px-3 text-xs font-medium text-success"
          >
            <Sparkles class="size-3.5" />
            {{ t('authorPage.badge') }}
          </Badge>

          <h1
            class="mt-6 text-[clamp(2.6rem,7vw,5.2rem)] font-semibold leading-[0.98] tracking-[-0.045em] text-foreground"
          >
            <span class="block">{{ t('authorPage.hero.line1') }}</span>
            <span class="hero-accent mt-1 block">{{ t('authorPage.hero.line2') }}</span>
          </h1>

          <p class="mt-6 max-w-2xl text-base leading-8 text-muted-foreground sm:text-lg">
            {{ t('authorPage.hero.description') }}
          </p>

          <div class="mt-6 flex flex-wrap gap-2">
            <span
              v-for="key in tagKeys"
              :key="key"
              class="rounded-full border border-border bg-card/70 px-3 py-1 text-xs text-muted-foreground transition-colors hover:border-success/30 hover:text-foreground"
            >
              {{ t(`authorPage.tags.${key}`) }}
            </span>
          </div>

          <div class="mt-8 flex flex-wrap items-center gap-3">
            <Button as-child size="lg" class="rounded-full px-5">
              <a href="#contact">
                <QrCode class="size-4" />
                {{ t('authorPage.hero.ctaPrimary') }}
              </a>
            </Button>
            <Button as-child variant="outline" size="lg" class="rounded-full px-5">
              <a :href="ORIGINAL_AUTHOR_URL" target="_blank" rel="noopener noreferrer">
                <GitBranch class="size-4" />
                {{ t('authorPage.hero.ctaSecondary') }}
                <ExternalLink class="size-3.5 opacity-60" />
              </a>
            </Button>
          </div>
        </div>

        <aside id="contact" class="author-qr scroll-mt-24 lg:sticky lg:top-24">
          <div
            class="qr-frame rounded-[28px] p-px shadow-[var(--card-shadow-raised)]"
          >
            <div class="rounded-[27px] bg-card p-5">
              <div class="rounded-2xl bg-white p-4 ring-1 ring-black/5">
                <img
                  src="/wechat_qrcode.png"
                  :alt="t('authorPage.contact.qrAlt')"
                  class="aspect-square w-full rounded-xl object-contain"
                  loading="lazy"
                />
              </div>

              <div class="mt-5 flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="text-base font-semibold">{{ t('authorPage.contact.title') }}</p>
                  <p class="mt-1 text-sm leading-6 text-muted-foreground">
                    {{ t('authorPage.contact.description') }}
                  </p>
                </div>
                <span class="grid size-10 shrink-0 place-items-center rounded-full bg-success/10 text-success">
                  <QrCode class="size-5" />
                </span>
              </div>

              <Separator class="my-4" />

              <ul class="space-y-2.5 text-sm text-muted-foreground">
                <li
                  v-for="key in contactItemKeys"
                  :key="key"
                  class="flex items-start gap-2 leading-6"
                >
                  <BadgeCheck class="mt-0.5 size-4 shrink-0 text-success" />
                  <span>{{ t(`authorPage.contact.items.${key}`) }}</span>
                </li>
              </ul>

              <p class="mt-4 rounded-xl border border-border/70 bg-muted/40 p-3 text-[11px] leading-5 text-muted-foreground">
                {{ t('authorPage.contact.note') }}
              </p>
            </div>
          </div>
        </aside>
      </section>

      <div v-reveal class="reveal mt-16">
        <AuthorSignalRail />
      </div>

      <section v-reveal class="reveal mt-24" aria-labelledby="capabilities-title">
        <div class="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <p class="font-mono text-[11px] tracking-[0.24em] text-success">
              {{ t('authorPage.capabilities.eyebrow') }}
            </p>
            <h2 id="capabilities-title" class="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
              {{ t('authorPage.capabilities.title') }}
            </h2>
          </div>
          <p class="text-sm text-muted-foreground">{{ t('authorPage.capabilities.subtitle') }}</p>
        </div>

        <div class="mt-8 grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <article
            v-for="(item, index) in capabilityItems"
            :key="item.key"
            class="tilt-card group relative overflow-hidden rounded-3xl border border-border bg-card p-6 hover:border-success/30 hover:shadow-[var(--card-shadow-raised)]"
            @pointermove="handleTilt"
            @pointerleave="resetTilt"
          >
            <div class="flex items-start justify-between gap-3">
              <span
                class="grid size-11 place-items-center rounded-2xl ring-1"
                :class="item.tone"
              >
                <component :is="item.icon" class="size-5" />
              </span>
              <span class="font-mono text-[10px] tracking-[0.2em] text-muted-foreground/50">
                {{ String(index + 1).padStart(2, '0') }}
              </span>
            </div>
            <h3 class="mt-5 text-lg font-semibold">
              {{ t(`authorPage.capabilities.items.${item.key}.title`) }}
            </h3>
            <p class="mt-2 text-sm leading-6 text-muted-foreground">
              {{ t(`authorPage.capabilities.items.${item.key}.description`) }}
            </p>
          </article>
        </div>
      </section>

      <section
        v-reveal
        class="reveal relative mt-10 overflow-hidden rounded-[32px] border border-success/20 bg-success/[0.06]"
        aria-labelledby="localization-title"
      >
        <span
          aria-hidden="true"
          class="pointer-events-none absolute -top-14 -right-6 select-none text-[11rem] font-black leading-none tracking-tighter text-foreground/[0.035] dark:text-foreground/[0.045]"
        >
          {{ t('authorPage.ecosystem.watermark') }}
        </span>

        <div
          class="relative grid gap-10 p-6 sm:p-10 lg:grid-cols-[minmax(0,1.05fr)_minmax(0,1fr)] lg:p-12"
        >
          <div>
            <Badge
              variant="outline"
              class="h-7 rounded-full border-success/30 bg-background/60 px-3 text-xs font-medium text-success"
            >
              <ShieldCheck class="size-3.5" />
              {{ t('authorPage.ecosystem.badge') }}
            </Badge>
            <h2
              id="localization-title"
              class="mt-5 whitespace-pre-line text-3xl font-semibold leading-tight tracking-tight sm:text-4xl"
            >
              {{ t('authorPage.ecosystem.title') }}
            </h2>
            <p class="mt-4 max-w-2xl text-sm leading-7 text-muted-foreground">
              {{ t('authorPage.ecosystem.description') }}
            </p>
            <p
              class="mt-6 rounded-2xl border border-border/70 bg-background/60 p-4 text-xs leading-6 text-muted-foreground"
            >
              {{ t('authorPage.ecosystem.note') }}
            </p>
          </div>

          <div class="grid gap-3 sm:grid-cols-3">
            <div
              v-for="column in ecosystemColumns"
              :key="column.key"
              class="min-w-0 rounded-2xl border border-border/70 bg-background/55 p-4 backdrop-blur-sm"
            >
              <div class="flex items-center gap-2 text-sm font-medium">
                <component :is="column.icon" class="size-4 shrink-0 text-success" />
                <span class="min-w-0">{{ t(`authorPage.ecosystem.${column.key}.title`) }}</span>
              </div>
              <div class="mt-3 flex flex-wrap gap-1.5">
                <span
                  v-for="item in column.items"
                  :key="item"
                  class="rounded-md border border-border/70 bg-card/70 px-2 py-1 text-[11px] leading-4 text-muted-foreground"
                >
                  {{ t(`authorPage.ecosystem.${column.key}.items.${item}`) }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section
        v-reveal
        class="reveal mt-10 rounded-[32px] border border-border bg-card/75 p-6 sm:p-8"
        aria-labelledby="license-title"
      >
        <div class="grid gap-8 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,0.6fr)] lg:gap-12">
          <div class="min-w-0">
            <p class="flex items-center gap-2 font-mono text-[11px] tracking-[0.24em] text-muted-foreground">
              <Scale class="size-3.5" />
              {{ t('authorPage.license.eyebrow') }}
            </p>
            <h2 id="license-title" class="mt-3 text-2xl font-semibold tracking-tight sm:text-3xl">
              {{ t('authorPage.license.title') }}
            </h2>
            <p class="mt-3 text-sm leading-7 text-muted-foreground">
              {{ t('authorPage.license.description') }}
            </p>
            <p
              class="mt-4 rounded-2xl border border-warning/30 bg-warning/5 p-3 text-xs leading-6 text-muted-foreground"
            >
              {{ t('authorPage.license.disclaimer') }}
            </p>
          </div>

          <div class="flex flex-col gap-2">
            <a
              v-for="link in licenseLinks"
              :key="link.href"
              :href="link.href"
              target="_blank"
              rel="noopener noreferrer"
              class="group flex items-center justify-between gap-3 rounded-xl border border-border/70 bg-background/60 px-3 py-2.5 text-sm text-muted-foreground transition-colors hover:border-success/30 hover:text-foreground"
            >
              <span class="min-w-0">{{ link.label }}</span>
              <ExternalLink
                class="size-3.5 shrink-0 opacity-60 transition-transform group-hover:-translate-y-0.5 group-hover:translate-x-0.5"
              />
            </a>
          </div>
        </div>
      </section>

      <footer
        class="mt-10 flex flex-col gap-2 border-t border-border/70 pt-6 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between"
      >
        <p>{{ t('authorPage.footer.copyright') }}</p>
        <p class="font-mono tracking-wide">{{ t('authorPage.footer.license') }}</p>
      </footer>
    </main>

    <a
      href="#contact"
      class="group fixed bottom-6 left-6 z-20 hidden items-center gap-2.5 rounded-full border border-white/10 bg-foreground px-4 py-2.5 text-xs font-medium text-background shadow-[0_12px_32px_rgba(0,0,0,0.25)] transition-transform duration-300 hover:-translate-y-0.5 lg:inline-flex motion-reduce:hover:translate-y-0"
    >
      <span class="relative flex size-1.5">
        <span class="absolute inline-flex size-full animate-ping rounded-full bg-success opacity-70 motion-reduce:animate-none" />
        <span class="relative inline-flex size-1.5 rounded-full bg-success" />
      </span>
      {{ t('authorPage.hero.ctaPrimary') }}
    </a>
  </div>
</template>

<style scoped>
@property --qr-spin {
  syntax: '<angle>';
  inherits: false;
  initial-value: 0deg;
}

.author-page {
  position: relative;
  min-height: 100dvh;
  overflow-x: clip;
  background: var(--background);
}

.author-backdrop {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  background-image:
    linear-gradient(to right, color-mix(in oklab, var(--border) 60%, transparent) 1px, transparent 1px),
    linear-gradient(to bottom, color-mix(in oklab, var(--border) 60%, transparent) 1px, transparent 1px),
    radial-gradient(60% 45% at 18% 0%, color-mix(in oklab, var(--success) 18%, transparent) 0%, transparent 70%),
    radial-gradient(45% 40% at 88% 8%, color-mix(in oklab, var(--primary) 14%, transparent) 0%, transparent 70%);
  background-size: 42px 42px, 42px 42px, auto, auto;
  mask-image: linear-gradient(to bottom, #000 0%, #000 58%, transparent 100%);
}

:global(.dark .author-backdrop) {
  opacity: 0.7;
}

.author-spotlight {
  position: fixed;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  opacity: var(--spot-opacity, 0);
  transition: opacity 450ms ease;
  background: radial-gradient(
    620px circle at var(--spot-x, 50vw) var(--spot-y, 0vh),
    color-mix(in oklab, var(--success) 12%, transparent),
    transparent 65%
  );
}

:global(.dark .author-spotlight) {
  background: radial-gradient(
    620px circle at var(--spot-x, 50vw) var(--spot-y, 0vh),
    color-mix(in oklab, var(--primary) 16%, transparent),
    transparent 65%
  );
}

.author-aurora {
  position: absolute;
  z-index: 0;
  pointer-events: none;
  border-radius: 9999px;
  filter: blur(72px);
  opacity: 0.42;
  will-change: transform;
  background: radial-gradient(circle, color-mix(in oklab, var(--success) 45%, transparent), transparent 70%);
  animation: aurora-drift 18s ease-in-out infinite alternate;
}

.author-aurora-a {
  top: -140px;
  left: -140px;
  width: 520px;
  height: 420px;
}

.author-aurora-b {
  top: 40px;
  right: -170px;
  width: 460px;
  height: 380px;
  background: radial-gradient(circle, color-mix(in oklab, var(--primary) 38%, transparent), transparent 70%);
  animation-delay: -6s;
  animation-duration: 22s;
}

:global(.dark .author-aurora) {
  opacity: 0.28;
}

.hero-accent {
  background-image: linear-gradient(
    100deg,
    var(--success) 0%,
    color-mix(in oklab, var(--success) 45%, var(--info)) 45%,
    color-mix(in oklab, var(--success) 60%, var(--primary)) 100%
  );
  background-size: 220% 100%;
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  color: transparent;
  animation: accent-pan 9s ease-in-out infinite alternate;
}

.qr-frame {
  position: relative;
  isolation: isolate;
  background: conic-gradient(
    from var(--qr-spin),
    color-mix(in oklab, var(--success) 85%, transparent),
    color-mix(in oklab, var(--info) 85%, transparent),
    color-mix(in oklab, var(--primary) 85%, transparent),
    color-mix(in oklab, var(--success) 85%, transparent)
  );
  animation: qr-spin 14s linear infinite;
}

.qr-frame::after {
  content: '';
  position: absolute;
  inset: -22px;
  z-index: -1;
  border-radius: 46px;
  background: conic-gradient(
    from var(--qr-spin),
    color-mix(in oklab, var(--success) 65%, transparent),
    color-mix(in oklab, var(--info) 55%, transparent),
    color-mix(in oklab, var(--primary) 55%, transparent),
    color-mix(in oklab, var(--success) 65%, transparent)
  );
  filter: blur(28px);
  opacity: 0.35;
}

.tilt-card {
  transform: perspective(900px) rotateX(var(--tilt-x, 0deg)) rotateY(var(--tilt-y, 0deg));
  transform-style: preserve-3d;
  transition:
    transform 240ms cubic-bezier(0.16, 1, 0.3, 1),
    border-color 300ms ease,
    box-shadow 300ms ease;
}

.tilt-card::before {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: inherit;
  pointer-events: none;
  opacity: 0;
  transition: opacity 220ms ease;
  background: radial-gradient(
    220px circle at var(--spot-x, 50%) var(--spot-y, 50%),
    color-mix(in oklab, var(--success) 14%, transparent),
    transparent 65%
  );
}

.tilt-card > * {
  position: relative;
  z-index: 1;
}

.tilt-card:hover::before {
  opacity: 1;
}

@media (hover: hover) {
  .tilt-card:hover {
    transform: perspective(900px) rotateX(var(--tilt-x, 0deg)) rotateY(var(--tilt-y, 0deg))
      translateY(-4px);
  }
}

.reveal {
  opacity: 0;
  transform: translateY(18px);
  transition:
    opacity 700ms cubic-bezier(0.16, 1, 0.3, 1),
    transform 700ms cubic-bezier(0.16, 1, 0.3, 1);
}

.reveal.is-visible {
  opacity: 1;
  transform: translateY(0);
}

.author-hero {
  animation: author-rise 620ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

.author-qr {
  animation: author-rise 700ms 90ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

@keyframes author-rise {
  from {
    opacity: 0;
    transform: translateY(16px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes aurora-drift {
  from {
    transform: translate3d(0, 0, 0) scale(1);
  }
  to {
    transform: translate3d(42px, 32px, 0) scale(1.12);
  }
}

@keyframes accent-pan {
  from {
    background-position: 0% 50%;
  }
  to {
    background-position: 100% 50%;
  }
}

@keyframes qr-spin {
  to {
    --qr-spin: 360deg;
  }
}

@media (prefers-reduced-motion: reduce) {
  .author-hero,
  .author-qr,
  .author-aurora,
  .hero-accent,
  .qr-frame {
    animation: none;
  }

  .tilt-card,
  .reveal {
    transform: none;
    transition: none;
  }

  .tilt-card::before {
    display: none;
  }

  .reveal {
    opacity: 1;
  }
}

@media (forced-colors: active) {
  .hero-accent {
    background: none;
    -webkit-text-fill-color: CanvasText;
    color: CanvasText;
  }
}
</style>
