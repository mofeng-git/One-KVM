<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Loader2 } from 'lucide-vue-next'
import { authApi } from '@/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
const open = defineModel<boolean>('open', { default: false })
const { t } = useI18n()
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const changingPassword = ref(false)
async function handleChangePassword() {
  if (!newPassword.value || !confirmPassword.value) {
    toast.error(t('auth.passwordRequired'))
    return
  }

  if (newPassword.value !== confirmPassword.value) {
    toast.error(t('auth.passwordMismatch'))
    return
  }

  if (newPassword.value.length < 4) {
    toast.error(t('auth.passwordTooShort'))
    return
  }

  changingPassword.value = true
  try {
    await authApi.changePassword(currentPassword.value, newPassword.value)

    currentPassword.value = ''
    newPassword.value = ''
    confirmPassword.value = ''
    open.value = false
  } catch (e) {
    console.info('[ChangePassword] Failed:', e)
  } finally {
    changingPassword.value = false
  }
}

</script>

<template>
    <Dialog v-model:open="open">
      <DialogContent class="w-[95vw] max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('auth.changePassword') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3 sm:space-y-4 py-2 sm:py-4">
          <div class="space-y-2">
            <Label for="currentPassword">{{ t('auth.currentPassword') }}</Label>
            <Input
              id="currentPassword"
              v-model="currentPassword"
              type="password"
              autocomplete="current-password"
              :placeholder="t('auth.currentPasswordPlaceholder')"
            />
          </div>
          <div class="space-y-2">
            <Label for="newPassword">{{ t('auth.newPassword') }}</Label>
            <Input
              id="newPassword"
              v-model="newPassword"
              type="password"
              autocomplete="new-password"
              :placeholder="t('auth.newPasswordPlaceholder')"
            />
          </div>
          <div class="space-y-2">
            <Label for="confirmPassword">{{ t('auth.confirmPassword') }}</Label>
            <Input
              id="confirmPassword"
              v-model="confirmPassword"
              type="password"
              autocomplete="new-password"
              :placeholder="t('auth.confirmPasswordPlaceholder')"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="open = false">
            {{ t('common.cancel') }}
          </Button>
          <Button @click="handleChangePassword" :disabled="changingPassword">
            <Loader2 v-if="changingPassword" class="size-4 mr-2 animate-spin" />
            {{ t('common.confirm') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
</template>
