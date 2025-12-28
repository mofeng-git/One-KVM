<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSystemStore } from '@/stores/system'
import { msdApi, type MsdImage, type DriveFile } from '@/api'
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from '@/components/ui/sheet'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Progress } from '@/components/ui/progress'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  HardDrive,
  Upload,
  Trash2,
  Link,
  Unlink,
  Disc,
  File,
  Folder,
  FolderPlus,
  Download,
  RefreshCw,
  ChevronRight,
  ArrowLeft,
} from 'lucide-vue-next'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const { t } = useI18n()
const systemStore = useSystemStore()

// Tab state
const activeTab = ref('images')

// Image state
const images = ref<MsdImage[]>([])
const loadingImages = ref(false)
const uploadProgress = ref(0)
const uploading = ref(false)
const cdromMode = ref(true)
const readOnly = ref(true)

// Drive state
const driveFiles = ref<DriveFile[]>([])
const currentPath = ref('/')
const loadingDrive = ref(false)
const driveInfo = ref<{ size: number; used: number; free: number; initialized: boolean } | null>(null)
const driveInitialized = ref(false)
const uploadingFile = ref(false)
const fileUploadProgress = ref(0)

// Dialog state
const showDeleteDialog = ref(false)
const deleteTarget = ref<{ type: 'image' | 'file'; id: string; name: string } | null>(null)
const showNewFolderDialog = ref(false)
const newFolderName = ref('')

// Computed
const msdConnected = computed(() => systemStore.msd?.connected ?? false)
const msdMode = computed(() => systemStore.msd?.mode ?? 'none')

const breadcrumbs = computed(() => {
  const parts = currentPath.value.split('/').filter(Boolean)
  const crumbs = [{ name: '/', path: '/' }]
  let path = ''
  for (const part of parts) {
    path += '/' + part
    crumbs.push({ name: part, path })
  }
  return crumbs
})

// Load data when sheet opens
watch(() => props.open, async (isOpen) => {
  if (isOpen) {
    await loadData()
  }
})

async function loadData() {
  await systemStore.fetchMsdState()
  await loadImages()
  await loadDriveInfo()
  if (driveInitialized.value) {
    await loadDriveFiles()
  }
}

// Image functions
async function loadImages() {
  loadingImages.value = true
  try {
    images.value = await msdApi.listImages()
  } catch (e) {
    console.error('Failed to load images:', e)
  } finally {
    loadingImages.value = false
  }
}

async function handleImageUpload(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return

  uploading.value = true
  uploadProgress.value = 0

  try {
    const image = await msdApi.uploadImage(file, (progress) => {
      uploadProgress.value = progress
    })
    images.value.push(image)
  } catch (e) {
    console.error('Failed to upload image:', e)
  } finally {
    uploading.value = false
    uploadProgress.value = 0
    input.value = ''
  }
}

async function connectImage(image: MsdImage) {
  try {
    await msdApi.connect('image', image.id, cdromMode.value, readOnly.value)
    await systemStore.fetchMsdState()
  } catch (e) {
    console.error('Failed to connect image:', e)
  }
}

async function disconnect() {
  try {
    await msdApi.disconnect()
    await systemStore.fetchMsdState()
  } catch (e) {
    console.error('Failed to disconnect:', e)
  }
}

function confirmDelete(type: 'image' | 'file', id: string, name: string) {
  deleteTarget.value = { type, id, name }
  showDeleteDialog.value = true
}

async function executeDelete() {
  if (!deleteTarget.value) return

  try {
    if (deleteTarget.value.type === 'image') {
      await msdApi.deleteImage(deleteTarget.value.id)
      images.value = images.value.filter(i => i.id !== deleteTarget.value!.id)
    } else {
      await msdApi.deleteDriveFile(deleteTarget.value.id)
      await loadDriveFiles()
    }
  } catch (e) {
    console.error('Failed to delete:', e)
  } finally {
    showDeleteDialog.value = false
    deleteTarget.value = null
  }
}

// Drive functions
async function loadDriveInfo() {
  try {
    driveInfo.value = await msdApi.driveInfo()
    driveInitialized.value = true
  } catch {
    driveInitialized.value = false
  }
}

async function initializeDrive() {
  try {
    await msdApi.initDrive(256)
    await loadDriveInfo()
    await loadDriveFiles()
  } catch (e) {
    console.error('Failed to initialize drive:', e)
  }
}

async function loadDriveFiles() {
  loadingDrive.value = true
  try {
    driveFiles.value = await msdApi.listDriveFiles(currentPath.value)
  } catch (e) {
    console.error('Failed to load drive files:', e)
  } finally {
    loadingDrive.value = false
  }
}

function navigateTo(path: string) {
  currentPath.value = path
  loadDriveFiles()
}

function navigateUp() {
  const parts = currentPath.value.split('/').filter(Boolean)
  parts.pop()
  currentPath.value = '/' + parts.join('/')
  loadDriveFiles()
}

async function handleFileUpload(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return

  uploadingFile.value = true
  fileUploadProgress.value = 0

  try {
    await msdApi.uploadDriveFile(file, currentPath.value, (progress) => {
      fileUploadProgress.value = progress
    })
    await loadDriveFiles()
  } catch (e) {
    console.error('Failed to upload file:', e)
  } finally {
    uploadingFile.value = false
    fileUploadProgress.value = 0
    input.value = ''
  }
}

async function createFolder() {
  if (!newFolderName.value.trim()) return

  try {
    const path = currentPath.value === '/'
      ? '/' + newFolderName.value
      : currentPath.value + '/' + newFolderName.value
    await msdApi.createDirectory(path)
    await loadDriveFiles()
  } catch (e) {
    console.error('Failed to create folder:', e)
  } finally {
    showNewFolderDialog.value = false
    newFolderName.value = ''
  }
}

async function connectDrive() {
  try {
    await msdApi.connect('drive')
    await systemStore.fetchMsdState()
  } catch (e) {
    console.error('Failed to connect drive:', e)
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB'
  return (bytes / 1024 / 1024 / 1024).toFixed(1) + ' GB'
}

onMounted(async () => {
  if (props.open) {
    await loadData()
  }
})
</script>

<template>
  <Sheet :open="open" @update:open="emit('update:open', $event)">
    <SheetContent side="right" class="w-full sm:max-w-lg overflow-hidden flex flex-col">
      <SheetHeader>
        <div class="flex items-center justify-between pr-8">
          <div>
            <SheetTitle class="flex items-center gap-2">
              <HardDrive class="h-5 w-5" />
              {{ t('msd.title') }}
            </SheetTitle>
            <SheetDescription class="flex items-center gap-2 mt-1">
              {{ msdConnected ? t('common.connected') : t('common.disconnected') }}
              <Badge v-if="msdConnected" variant="secondary" class="text-xs">{{ msdMode }}</Badge>
            </SheetDescription>
          </div>
          <Button v-if="msdConnected" variant="destructive" size="sm" @click="disconnect">
            <Unlink class="h-4 w-4 mr-1" />
            {{ t('msd.disconnect') }}
          </Button>
        </div>
      </SheetHeader>

      <Separator class="my-4" />

      <Tabs v-model="activeTab" class="flex-1 flex flex-col overflow-hidden">
        <TabsList class="w-full grid grid-cols-2">
          <TabsTrigger value="images">
            <Disc class="h-4 w-4 mr-1.5" />
            {{ t('msd.images') }}
          </TabsTrigger>
          <TabsTrigger value="drive">
            <HardDrive class="h-4 w-4 mr-1.5" />
            {{ t('msd.drive') }}
          </TabsTrigger>
        </TabsList>

        <ScrollArea class="flex-1 mt-4">
          <!-- Images Tab -->
          <TabsContent value="images" class="m-0 space-y-4">
            <!-- Upload Area -->
            <div class="space-y-3">
              <label class="block">
                <input
                  type="file"
                  accept=".iso,.img"
                  class="hidden"
                  :disabled="uploading"
                  @change="handleImageUpload"
                />
                <div class="flex items-center justify-center h-16 border-2 border-dashed rounded-lg cursor-pointer hover:border-primary transition-colors">
                  <div class="text-center">
                    <Upload class="h-5 w-5 mx-auto text-muted-foreground" />
                    <p class="text-xs text-muted-foreground mt-1">{{ t('msd.uploadImage') }} (ISO/IMG)</p>
                  </div>
                </div>
              </label>
              <Progress v-if="uploading" :model-value="uploadProgress" class="h-1" />
            </div>

            <!-- Options -->
            <div class="flex items-center gap-4 p-3 rounded-lg bg-muted/50">
              <div class="flex items-center gap-2">
                <Switch id="cdrom" v-model:checked="cdromMode" />
                <Label for="cdrom" class="text-xs">{{ t('msd.cdromMode') }}</Label>
              </div>
              <div class="flex items-center gap-2">
                <Switch id="readonly" v-model:checked="readOnly" />
                <Label for="readonly" class="text-xs">{{ t('msd.readOnly') }}</Label>
              </div>
            </div>

            <!-- Image List -->
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <h4 class="text-sm font-medium">{{ t('msd.imageList') }}</h4>
                <Button variant="ghost" size="icon" class="h-7 w-7" @click="loadImages">
                  <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loadingImages }" />
                </Button>
              </div>

              <div v-if="images.length === 0" class="text-center py-6 text-muted-foreground text-sm">
                {{ t('msd.noImages') }}
              </div>

              <div v-else class="space-y-1.5">
                <div
                  v-for="image in images"
                  :key="image.id"
                  class="flex items-center justify-between p-2.5 rounded-lg border hover:bg-accent/50 transition-colors"
                >
                  <div class="flex items-center gap-2.5 min-w-0 flex-1">
                    <Disc class="h-4 w-4 text-muted-foreground shrink-0" />
                    <div class="min-w-0">
                      <p class="text-sm font-medium truncate">{{ image.name }}</p>
                      <p class="text-xs text-muted-foreground">
                        {{ formatBytes(image.size) }}
                      </p>
                    </div>
                  </div>
                  <div class="flex items-center gap-1 shrink-0">
                    <Button
                      v-if="!msdConnected || systemStore.msd?.imageId !== image.id"
                      variant="outline"
                      size="sm"
                      class="h-7 text-xs"
                      @click="connectImage(image)"
                    >
                      <Link class="h-3.5 w-3.5 mr-1" />
                      {{ t('msd.connect') }}
                    </Button>
                    <Badge v-else variant="default" class="text-xs">{{ t('common.connected') }}</Badge>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-destructive"
                      @click="confirmDelete('image', image.id, image.name)"
                    >
                      <Trash2 class="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          </TabsContent>

          <!-- Drive Tab -->
          <TabsContent value="drive" class="m-0 space-y-4">
            <template v-if="!driveInitialized">
              <div class="text-center py-8 space-y-4">
                <HardDrive class="h-10 w-10 mx-auto text-muted-foreground" />
                <p class="text-sm text-muted-foreground">{{ t('msd.driveNotInitialized') }}</p>
                <Button size="sm" @click="initializeDrive">
                  {{ t('msd.initializeDrive') }}
                </Button>
              </div>
            </template>

            <template v-else>
              <!-- Drive Info -->
              <div class="p-3 rounded-lg bg-muted/50 space-y-2">
                <div class="flex items-center justify-between">
                  <div class="space-y-0.5">
                    <p class="text-xs text-muted-foreground">{{ t('msd.driveSize') }}: {{ (driveInfo?.size || 0) / 1024 / 1024 }}MB</p>
                    <p class="text-xs text-muted-foreground">
                      {{ formatBytes(driveInfo?.used || 0) }} / {{ formatBytes(driveInfo?.size || 0) }}
                    </p>
                  </div>
                  <Button
                    v-if="!msdConnected || msdMode !== 'drive'"
                    variant="outline"
                    size="sm"
                    class="h-7 text-xs"
                    @click="connectDrive"
                  >
                    <Link class="h-3.5 w-3.5 mr-1" />
                    {{ t('msd.connect') }}
                  </Button>
                  <Badge v-else class="text-xs">{{ t('common.connected') }}</Badge>
                </div>
                <Progress
                  v-if="driveInfo"
                  :model-value="driveInfo.size > 0 ? (driveInfo.used / driveInfo.size) * 100 : 0"
                  class="h-1"
                />
              </div>

              <!-- File Browser -->
              <div class="space-y-2">
                <!-- Toolbar -->
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-1 min-w-0 flex-1">
                    <Button
                      v-if="currentPath !== '/'"
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 shrink-0"
                      @click="navigateUp"
                    >
                      <ArrowLeft class="h-3.5 w-3.5" />
                    </Button>
                    <nav class="flex items-center text-xs min-w-0 overflow-hidden">
                      <template v-for="(crumb, index) in breadcrumbs" :key="crumb.path">
                        <ChevronRight v-if="index > 0" class="h-3 w-3 text-muted-foreground mx-0.5 shrink-0" />
                        <button
                          class="hover:text-primary transition-colors truncate"
                          :class="index === breadcrumbs.length - 1 ? 'font-medium' : 'text-muted-foreground'"
                          @click="navigateTo(crumb.path)"
                        >
                          {{ crumb.name }}
                        </button>
                      </template>
                    </nav>
                  </div>
                  <div class="flex items-center gap-1 shrink-0">
                    <label>
                      <input type="file" class="hidden" :disabled="uploadingFile" @change="handleFileUpload" />
                      <Button variant="ghost" size="icon" as="span" class="h-7 w-7 cursor-pointer">
                        <Upload class="h-3.5 w-3.5" />
                      </Button>
                    </label>
                    <Button variant="ghost" size="icon" class="h-7 w-7" @click="showNewFolderDialog = true">
                      <FolderPlus class="h-3.5 w-3.5" />
                    </Button>
                    <Button variant="ghost" size="icon" class="h-7 w-7" @click="loadDriveFiles">
                      <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loadingDrive }" />
                    </Button>
                  </div>
                </div>

                <Progress v-if="uploadingFile" :model-value="fileUploadProgress" class="h-1" />

                <!-- File List -->
                <div v-if="driveFiles.length === 0" class="text-center py-6 text-muted-foreground text-sm">
                  {{ t('msd.emptyFolder') }}
                </div>

                <div v-else class="space-y-1">
                  <div
                    v-for="file in driveFiles"
                    :key="file.path"
                    class="flex items-center justify-between p-2 rounded-lg hover:bg-accent/50 transition-colors"
                  >
                    <div
                      class="flex items-center gap-2 cursor-pointer flex-1 min-w-0"
                      @click="file.is_dir && navigateTo(file.path)"
                    >
                      <Folder v-if="file.is_dir" class="h-4 w-4 text-blue-500 shrink-0" />
                      <File v-else class="h-4 w-4 text-muted-foreground shrink-0" />
                      <div class="min-w-0">
                        <p class="text-sm font-medium truncate">{{ file.name }}</p>
                        <p v-if="!file.is_dir" class="text-xs text-muted-foreground">
                          {{ formatBytes(file.size) }}
                        </p>
                      </div>
                    </div>
                    <div class="flex items-center gap-0.5 shrink-0">
                      <Button
                        v-if="!file.is_dir"
                        variant="ghost"
                        size="icon"
                        class="h-7 w-7"
                        as="a"
                        :href="msdApi.downloadDriveFile(file.path)"
                        download
                      >
                        <Download class="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-7 w-7 text-destructive"
                        @click="confirmDelete('file', file.path, file.name)"
                      >
                        <Trash2 class="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </div>
                </div>
              </div>
            </template>
          </TabsContent>
        </ScrollArea>
      </Tabs>
    </SheetContent>
  </Sheet>

  <!-- Delete Dialog -->
  <Dialog v-model:open="showDeleteDialog">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{{ t('common.confirm') }}</DialogTitle>
        <DialogDescription>
          {{ t('msd.confirmDelete', { name: deleteTarget?.name }) }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" @click="showDeleteDialog = false">{{ t('common.cancel') }}</Button>
        <Button variant="destructive" @click="executeDelete">{{ t('common.delete') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <!-- New Folder Dialog -->
  <Dialog v-model:open="showNewFolderDialog">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{{ t('msd.createFolder') }}</DialogTitle>
      </DialogHeader>
      <Input v-model="newFolderName" :placeholder="t('msd.folderName')" @keyup.enter="createFolder" />
      <DialogFooter>
        <Button variant="outline" @click="showNewFolderDialog = false">{{ t('common.cancel') }}</Button>
        <Button @click="createFolder">{{ t('common.confirm') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
