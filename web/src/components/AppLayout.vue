<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAuthStore } from '@/stores/auth'
import { useSystemStore } from '@/stores/system'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Monitor,
  Settings,
  LogOut,
  Sun,
  Moon,
  Languages,
  Menu,
} from 'lucide-vue-next'
import { setLanguage } from '@/i18n'

const { t, locale } = useI18n()
const route = useRoute()
const router = useRouter()
const authStore = useAuthStore()
const systemStore = useSystemStore()

const navItems = computed(() => [
  { path: '/', name: 'Console', icon: Monitor, label: t('nav.console') },
  { path: '/settings', name: 'Settings', icon: Settings, label: t('nav.settings') },
])

function toggleTheme() {
  const isDark = document.documentElement.classList.contains('dark')
  document.documentElement.classList.toggle('dark', !isDark)
  localStorage.setItem('theme', isDark ? 'light' : 'dark')
}

function toggleLanguage() {
  const newLang = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN'
  setLanguage(newLang)
}

async function handleLogout() {
  await authStore.logout()
  router.push('/login')
}
</script>

<template>
  <div class="min-h-screen bg-background">
    <!-- Header -->
    <header class="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div class="flex h-14 items-center px-4 max-w-full">
        <!-- Logo -->
        <RouterLink to="/" class="flex items-center gap-2 font-semibold">
          <Monitor class="h-5 w-5" />
          <span class="hidden sm:inline">One-KVM</span>
        </RouterLink>

        <!-- Navigation -->
        <nav class="hidden md:flex items-center gap-1 ml-6">
          <RouterLink
            v-for="item in navItems"
            :key="item.path"
            :to="item.path"
            class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-md transition-colors"
            :class="route.path === item.path
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'"
          >
            <component :is="item.icon" class="h-4 w-4" />
            {{ item.label }}
          </RouterLink>
        </nav>

        <!-- Right Side -->
        <div class="flex items-center gap-2 ml-auto">
          <!-- Version Badge -->
          <span v-if="systemStore.version" class="hidden sm:inline text-xs text-muted-foreground">
            v{{ systemStore.version }}
          </span>

          <!-- Theme Toggle -->
          <Button variant="ghost" size="icon" @click="toggleTheme">
            <Sun class="h-4 w-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
            <Moon class="absolute h-4 w-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
            <span class="sr-only">{{ t('common.toggleTheme') }}</span>
          </Button>

          <!-- Language Toggle -->
          <Button variant="ghost" size="icon" @click="toggleLanguage">
            <Languages class="h-4 w-4" />
            <span class="sr-only">{{ t('common.toggleLanguage') }}</span>
          </Button>

          <!-- Mobile Menu -->
          <DropdownMenu>
            <DropdownMenuTrigger as-child class="md:hidden">
              <Button variant="ghost" size="icon">
                <Menu class="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem v-for="item in navItems" :key="item.path" @click="router.push(item.path)">
                <component :is="item.icon" class="h-4 w-4 mr-2" />
                {{ item.label }}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem @click="handleLogout">
                <LogOut class="h-4 w-4 mr-2" />
                {{ t('nav.logout') }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          <!-- Logout Button (Desktop) -->
          <Button variant="ghost" size="icon" class="hidden md:flex" @click="handleLogout">
            <LogOut class="h-4 w-4" />
            <span class="sr-only">{{ t('nav.logout') }}</span>
          </Button>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main class="px-4 py-6 max-w-full">
      <slot />
    </main>
  </div>
</template>
