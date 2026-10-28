<script setup lang="ts">
import { SidebarMenuButton, useSidebar } from '@/components/ui/sidebar'
defineOptions({ inheritAttrs: false })
defineProps<{ isActive?: boolean }>()
const { isMobile, setOpenMobile } = useSidebar()
function closeMobile() {
  if (isMobile.value) setOpenMobile(false)
}
</script>
<template>
  <SidebarMenuButton
    v-bind="$attrs"
    :is-active="isActive"
    :aria-current="isActive ? 'page' : undefined"
    @click="closeMobile"
    ><slot
  /></SidebarMenuButton>
</template>
