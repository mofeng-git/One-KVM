<script setup lang="ts">
import { RouterLink, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useSystemStore } from '@/stores/system'
import LanguageToggleButton from '@/components/LanguageToggleButton.vue'
import BrandMark from '@/components/BrandMark.vue'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  LogOut,
  Sun,
  Moon,
  Menu,
} from 'lucide-vue-next'

const { t } = useI18n()
const router = useRouter()
const authStore = useAuthStore()
const systemStore = useSystemStore()

function toggleTheme() {
  const isDark = document.documentElement.classList.contains('dark')
  document.documentElement.classList.toggle('dark', !isDark)
  localStorage.setItem('theme', isDark ? 'light' : 'dark')
}

async function handleLogout() {
  await authStore.logout()
  router.push('/login')
}
</script>

<template>
  <div class="h-screen h-dvh flex flex-col bg-background overflow-hidden">
    <!-- Header -->
    <header class="shrink-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div class="flex h-11 sm:h-14 items-center px-3 sm:px-4 max-w-full">
        <!-- Logo -->
        <RouterLink to="/" class="flex items-center gap-1.5 sm:gap-2 font-semibold">
          <BrandMark size="sm" />
          <span class="hidden sm:inline">One-KVM</span>
        </RouterLink>

        <!-- Right Side -->
        <div class="flex items-center gap-1 sm:gap-2 ml-auto">
          <!-- Version Badge -->
          <span v-if="systemStore.version" class="hidden sm:inline text-xs text-muted-foreground">
            v{{ systemStore.version }}
          </span>

          <!-- Theme Toggle -->
          <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('common.toggleTheme')" @click="toggleTheme">
            <Sun class="h-4 w-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
            <Moon class="absolute h-4 w-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
            <span class="sr-only">{{ t('common.toggleTheme') }}</span>
          </Button>

          <!-- Language Toggle -->
          <LanguageToggleButton />

          <!-- Mobile Menu -->
          <DropdownMenu>
            <DropdownMenuTrigger as-child class="md:hidden">
              <Button variant="ghost" size="icon" class="h-8 w-8" :aria-label="t('common.menu')">
                <Menu class="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem @click="handleLogout">
                <LogOut class="h-4 w-4 mr-2" />
                {{ t('nav.logout') }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          <!-- Logout Button (Desktop) -->
          <Button variant="ghost" size="icon" class="hidden md:flex h-8 w-8" :aria-label="t('nav.logout')" @click="handleLogout">
            <LogOut class="h-4 w-4" />
            <span class="sr-only">{{ t('nav.logout') }}</span>
          </Button>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main class="flex-1 overflow-hidden">
      <slot />
    </main>
  </div>
</template>
