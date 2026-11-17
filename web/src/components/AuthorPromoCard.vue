<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { ArrowUpRight, Sparkles } from 'lucide-vue-next'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'

const { t } = useI18n()
</script>

<template>
  <Card class="relative overflow-hidden border-success/25 bg-gradient-to-br from-success/[0.09] via-card to-card">
    <div class="pointer-events-none absolute -top-24 -right-16 size-56 rounded-full bg-success/10 blur-3xl" />
    <CardContent class="relative flex flex-col gap-4 p-5 sm:flex-row sm:items-center sm:justify-between">
      <div class="flex min-w-0 items-start gap-3">
        <span class="grid size-10 shrink-0 place-items-center rounded-2xl bg-success/10 text-success ring-1 ring-success/20">
          <Sparkles class="size-5" />
        </span>
        <div class="min-w-0">
          <p class="font-mono text-[10px] tracking-[0.22em] text-success">
            {{ t('authorPage.promo.eyebrow') }}
          </p>
          <p class="mt-1 text-base font-semibold">{{ t('authorPage.promo.title') }}</p>
          <p class="mt-1 max-w-2xl text-sm leading-6 text-muted-foreground">
            {{ t('authorPage.promo.description') }}
          </p>
        </div>
      </div>
      <Button as-child class="shrink-0 self-start rounded-full sm:self-center">
        <RouterLink :to="{ name: 'Author' }">
          {{ t('authorPage.promo.action') }}
          <ArrowUpRight class="size-4" />
        </RouterLink>
      </Button>
    </CardContent>
  </Card>
</template>
