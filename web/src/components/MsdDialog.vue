<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { useSystemStore } from '@/stores/system'
import { msdApi, type MsdImage, type DriveFile } from '@/api'
import { useWebSocket } from '@/composables/useWebSocket'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Progress } from '@/components/ui/progress'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'
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
  Globe,
  X,
  AlertCircle,
} from 'lucide-vue-next'
import HelpTooltip from '@/components/HelpTooltip.vue'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const { t } = useI18n()
const systemStore = useSystemStore()
const { on, off } = useWebSocket()

// Tab state
const activeTab = ref('images')

// Image state
const images = ref<MsdImage[]>([])
const loadingImages = ref(false)
const uploadProgress = ref(0)
const uploading = ref(false)

// Mount options (using ToggleGroup)
const mountMode = ref<'cdrom' | 'flash'>('flash')
const accessMode = ref<'readonly' | 'readwrite'>('readonly')

// Computed properties for API compatibility
const cdromMode = computed(() => mountMode.value === 'cdrom')
const readOnly = computed(() => accessMode.value === 'readonly')

// Operation state flags
const connecting = ref(false)
const disconnecting = ref(false)
const deleting = ref(false)

// Drive state
const driveFiles = ref<DriveFile[]>([])
const currentPath = ref('/')
const loadingDrive = ref(false)
const driveInfo = ref<{ size: number; used: number; free: number; initialized: boolean } | null>(null)
const driveInitialized = ref(false)
const uploadingFile = ref(false)
const fileUploadProgress = ref(0)

// Inner dialog state
const showDeleteDialog = ref(false)
const deleteTarget = ref<{ type: 'image' | 'file'; id: string; name: string } | null>(null)
const showNewFolderDialog = ref(false)
const newFolderName = ref('')

// Drive init dialog state
const showDriveInitDialog = ref(false)
const showDeleteDriveDialog = ref(false)
const selectedDriveSize = ref(256) // Default 256MB
const customDriveSize = ref<number | undefined>(undefined)
const initializingDrive = ref(false)
const deletingDrive = ref(false)

// URL download state
const showUrlDialog = ref(false)
const downloadUrl = ref('')
const downloadFilename = ref('')
const downloading = ref(false)
const downloadProgress = ref<{
  download_id: string
  filename: string
  bytes_downloaded: number
  total_bytes: number | null
  progress_pct: number | null
  status: string
} | null>(null)

// Constants
const TWO_POINT_TWO_GB = 2.2 * 1024 * 1024 * 1024

// Computed
const msdConnected = computed(() => systemStore.msd?.connected ?? false)
const msdMode = computed(() => systemStore.msd?.mode ?? 'none')

// Get currently connected image name
const connectedImageName = computed(() => {
  if (!msdConnected.value) return null
  if (msdMode.value === 'drive') return t('msd.drive')
  const imageId = systemStore.msd?.imageId
  if (!imageId) return null
  const image = images.value.find(i => i.id === imageId)
  return image?.name ?? null
})

// Check if any operation is in progress
const operationInProgress = computed(() => {
  return connecting.value ||
         disconnecting.value ||
         deleting.value ||
         uploading.value ||
         uploadingFile.value ||
         initializingDrive.value ||
         deletingDrive.value
})

// Check if image is large (>2.2GB)
function isLargeFile(image: MsdImage): boolean {
  return image.size > TWO_POINT_TWO_GB
}

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

// Load data when dialog opens
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
  if (operationInProgress.value) {
    toast.warning(t('msd.operationInProgress'))
    return
  }

  connecting.value = true
  try {
    await msdApi.connect('image', image.id, cdromMode.value, readOnly.value)
    await systemStore.fetchMsdState()
    toast.success(t('msd.imageMounted', { name: image.name }))
  } catch (e) {
    console.error('Failed to connect image:', e)
  } finally {
    connecting.value = false
  }
}

async function connectDrive() {
  if (operationInProgress.value) {
    toast.warning(t('msd.operationInProgress'))
    return
  }

  connecting.value = true
  try {
    await msdApi.connect('drive')
    await systemStore.fetchMsdState()
    toast.success(t('common.connected'))
  } catch (e) {
    console.error('Failed to connect drive:', e)
  } finally {
    connecting.value = false
  }
}

async function disconnect() {
  if (operationInProgress.value) {
    return
  }

  disconnecting.value = true
  try {
    await msdApi.disconnect()
    await systemStore.fetchMsdState()
    toast.success(t('msd.disconnected'))
  } catch (e) {
    console.error('Failed to disconnect:', e)
  } finally {
    disconnecting.value = false
  }
}

function confirmDelete(type: 'image' | 'file', id: string, name: string) {
  deleteTarget.value = { type, id, name }
  showDeleteDialog.value = true
}

async function executeDelete() {
  if (!deleteTarget.value || deleting.value) return

  deleting.value = true
  try {
    if (deleteTarget.value.type === 'image') {
      await msdApi.deleteImage(deleteTarget.value.id)
      images.value = images.value.filter(i => i.id !== deleteTarget.value!.id)
      toast.success(t('common.success'))
    } else {
      await msdApi.deleteDriveFile(deleteTarget.value.id)
      await loadDriveFiles()
      toast.success(t('common.success'))
    }
  } catch (e) {
    console.error('Failed to delete:', e)
  } finally {
    showDeleteDialog.value = false
    deleteTarget.value = null
    deleting.value = false
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

// Drive size options - computed for i18n support
const driveSizeOptions = computed(() => [
  { value: 64, label: '64 MB' },
  { value: 128, label: '128 MB' },
  { value: 256, label: `256 MB (${t('common.recommended')})`, recommended: true },
  { value: 512, label: '512 MB' },
  { value: 1024, label: '1 GB' },
  { value: 2048, label: '2 GB' },
  { value: 4096, label: '4 GB' },
  { value: 8192, label: '8 GB' },
])

// Computed final drive size
const finalDriveSize = computed(() => {
  return customDriveSize.value || selectedDriveSize.value
})

// Open drive init dialog
function initializeDrive() {
  showDriveInitDialog.value = true
}

// Create drive with selected size
async function createDrive() {
  initializingDrive.value = true
  try {
    await msdApi.initDrive(finalDriveSize.value)
    await loadDriveInfo()
    await loadDriveFiles()
    showDriveInitDialog.value = false
    toast.success(t('common.success'))
  } catch (e) {
    console.error('Failed to initialize drive:', e)
  } finally {
    initializingDrive.value = false
  }
}

// Delete virtual drive
async function deleteDrive() {
  deletingDrive.value = true
  try {
    await msdApi.deleteDrive()
    driveInitialized.value = false
    driveInfo.value = null
    driveFiles.value = []
    currentPath.value = '/'
    showDeleteDriveDialog.value = false
    toast.success(t('msd.driveDeleted'))
  } catch (e) {
    console.error('Failed to delete drive:', e)
  } finally {
    deletingDrive.value = false
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

// URL download functions
async function startUrlDownload() {
  if (!downloadUrl.value.trim()) return

  downloading.value = true
  try {
    const result = await msdApi.downloadFromUrl(
      downloadUrl.value.trim(),
      downloadFilename.value.trim() || undefined
    )
    downloadProgress.value = {
      download_id: result.download_id,
      filename: result.filename,
      bytes_downloaded: result.bytes_downloaded,
      total_bytes: result.total_bytes,
      progress_pct: result.progress_pct,
      status: result.status,
    }
  } catch (e) {
    console.error('Failed to start download:', e)
    downloading.value = false
  }
}

async function cancelUrlDownload() {
  if (!downloadProgress.value) return

  try {
    await msdApi.cancelDownload(downloadProgress.value.download_id)
  } catch (e) {
    console.error('Failed to cancel download:', e)
  } finally {
    resetDownloadState()
  }
}

function resetDownloadState() {
  downloading.value = false
  downloadProgress.value = null
  downloadUrl.value = ''
  downloadFilename.value = ''
}

function handleDownloadProgress(data: {
  download_id: string
  filename: string
  bytes_downloaded: number
  total_bytes: number | null
  progress_pct: number | null
  status: string
}) {
  if (downloadProgress.value?.download_id === data.download_id) {
    downloadProgress.value = data

    if (data.status === 'completed') {
      loadImages()
      setTimeout(() => {
        showUrlDialog.value = false
        resetDownloadState()
      }, 1000)
    } else if (data.status.startsWith('failed')) {
      downloading.value = false
    }
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
  on('msd.download_progress', handleDownloadProgress)
})

onUnmounted(() => {
  off('msd.download_progress', handleDownloadProgress)
})
</script>

<template>
  <TooltipProvider>
    <Dialog :open="open" @update:open="emit('update:open', $event)">
      <DialogContent class="sm:max-w-[600px] max-h-[90vh] overflow-hidden flex flex-col p-0">
      <DialogHeader class="px-6 pt-6">
        <DialogTitle class="flex items-center gap-2">
          <HardDrive class="h-5 w-5" />
          {{ t('msd.title') }}
        </DialogTitle>
        <DialogDescription class="flex items-center flex-wrap gap-x-2 gap-y-1 mt-1">
          <span :class="msdConnected ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'" class="flex items-center gap-1.5">
            <span class="relative flex h-2 w-2">
              <span v-if="msdConnected" class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
              <span :class="msdConnected ? 'bg-green-500' : 'bg-muted-foreground'" class="relative inline-flex rounded-full h-2 w-2"></span>
            </span>
            {{ msdConnected ? t('common.connected') : t('common.disconnected') }}
          </span>
          <template v-if="msdConnected && connectedImageName">
            <span class="text-muted-foreground">·</span>
            <Tooltip>
              <TooltipTrigger as-child>
                <span class="truncate max-w-[180px] cursor-help">{{ connectedImageName }}</span>
              </TooltipTrigger>
              <TooltipContent>
                <p>{{ connectedImageName }}</p>
              </TooltipContent>
            </Tooltip>
            <Badge variant="secondary" class="text-xs">{{ msdMode === 'drive' ? t('msd.drive') : t('msd.images') }}</Badge>
            <Button
              variant="outline"
              size="sm"
              class="h-6 px-2 text-xs text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
              :disabled="operationInProgress"
              @click="disconnect"
            >
              <Unlink v-if="!disconnecting" class="h-3 w-3 mr-1" />
              <span v-if="disconnecting">{{ t('common.disconnecting') }}...</span>
              <span v-else>{{ t('msd.disconnect') }}</span>
            </Button>
          </template>
        </DialogDescription>
      </DialogHeader>

      <Separator />

      <Tabs v-model="activeTab" class="flex-1 flex flex-col overflow-hidden px-6 pb-6 pt-4">
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

        <!-- Tab Description -->
        <p class="text-xs text-muted-foreground mt-2 mb-1">
          {{ activeTab === 'images' ? t('msd.imagesDesc') : t('msd.driveDesc') }}
        </p>

        <ScrollArea class="flex-1 mt-2">
          <!-- Images Tab -->
          <TabsContent value="images" class="m-0 space-y-3 pr-4">
            <!-- Compact Upload Toolbar -->
            <div class="flex items-center gap-2 min-w-0">
              <label class="flex-1">
                <input
                  type="file"
                  class="hidden"
                  accept=".iso,.img"
                  :disabled="uploading"
                  @change="handleImageUpload"
                />
                <Button variant="outline" size="sm" as="span" class="w-full cursor-pointer">
                  <Upload class="h-4 w-4 mr-1.5" />
                  {{ t('msd.uploadImage') }}
                </Button>
              </label>
              <Button
                variant="outline"
                size="sm"
                class="flex-1"
                @click="showUrlDialog = true"
              >
                <Globe class="h-4 w-4 mr-1.5" />
                {{ t('msd.downloadFromUrl') }}
              </Button>
            </div>
            <Progress v-if="uploading" :model-value="uploadProgress" class="h-1" />

            <!-- Options - Vertical compact layout -->
            <div class="flex flex-wrap items-center gap-x-4 gap-y-2 p-2 rounded-lg bg-muted/50 text-xs min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="text-muted-foreground whitespace-nowrap">{{ t('msd.storageMode') }}:</span>
                <HelpTooltip :content="mountMode === 'flash' ? t('help.flashMode') : t('help.cdromMode')" icon-size="sm" />
                <ToggleGroup v-model="mountMode" type="single" variant="outline" size="sm">
                  <ToggleGroupItem value="flash" class="h-6 px-2 text-xs data-[state=on]:bg-primary data-[state=on]:text-primary-foreground">
                    {{ t('msd.flash') }}
                  </ToggleGroupItem>
                  <ToggleGroupItem value="cdrom" class="h-6 px-2 text-xs data-[state=on]:bg-primary data-[state=on]:text-primary-foreground">
                    {{ t('msd.cdrom') }}
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
              <div class="flex items-center gap-1.5">
                <span class="text-muted-foreground whitespace-nowrap">{{ t('msd.accessMode') }}:</span>
                <HelpTooltip :content="accessMode === 'readonly' ? t('help.readOnlyMode') : t('help.readWriteMode')" icon-size="sm" />
                <ToggleGroup v-model="accessMode" type="single" variant="outline" size="sm">
                  <ToggleGroupItem value="readonly" class="h-6 px-2 text-xs data-[state=on]:bg-primary data-[state=on]:text-primary-foreground">
                    {{ t('msd.readOnly') }}
                  </ToggleGroupItem>
                  <ToggleGroupItem value="readwrite" class="h-6 px-2 text-xs data-[state=on]:bg-primary data-[state=on]:text-primary-foreground">
                    {{ t('msd.readWrite') }}
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
            </div>

            <!-- Image List -->
            <div class="space-y-2 min-w-0">
              <div class="flex items-center justify-between">
                <h4 class="text-sm font-medium">{{ t('msd.imageList') }}</h4>
                <Button variant="ghost" size="icon" class="h-7 w-7" @click="loadImages">
                  <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loadingImages }" />
                </Button>
              </div>

              <div v-if="images.length === 0" class="text-center py-6 text-muted-foreground text-sm">
                {{ t('msd.noImages') }}
              </div>

              <div v-else class="space-y-2">
                <div
                  v-for="image in images"
                  :key="image.id"
                  class="p-3 rounded-lg border transition-colors"
                  :class="[
                    msdConnected && systemStore.msd?.imageId === image.id
                      ? 'border-primary bg-primary/5'
                      : 'hover:bg-accent/50'
                  ]"
                >
                  <div class="flex items-start justify-between gap-2">
                    <div class="flex items-start gap-2 w-0 flex-1">
                      <Disc class="h-4 w-4 text-muted-foreground shrink-0 mt-0.5" />
                      <div class="w-0 flex-1">
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <p class="text-sm font-medium cursor-help overflow-hidden text-ellipsis whitespace-nowrap">{{ image.name }}</p>
                          </TooltipTrigger>
                          <TooltipContent>
                            <p class="max-w-sm break-all">{{ image.name }}</p>
                          </TooltipContent>
                        </Tooltip>
                        <div class="flex items-center gap-2 mt-0.5 flex-wrap">
                          <span class="text-xs text-muted-foreground">{{ formatBytes(image.size) }}</span>
                          <Tooltip v-if="isLargeFile(image)">
                            <TooltipTrigger as-child>
                              <Badge
                                variant="outline"
                                class="text-[10px] h-4 px-1.5 border-amber-500/50 text-amber-600 dark:text-amber-400 cursor-help"
                              >
                                <AlertCircle class="h-2.5 w-2.5 mr-0.5" />
                                {{ t('msd.largeFileWarning') }}
                              </Badge>
                            </TooltipTrigger>
                            <TooltipContent>
                              <p>{{ t('msd.largeFileTooltip') }}</p>
                            </TooltipContent>
                          </Tooltip>
                        </div>
                      </div>
                    </div>
                    <div class="flex items-center gap-1.5 shrink-0">
                      <template v-if="msdConnected && systemStore.msd?.imageId === image.id">
                        <Badge variant="default" class="text-xs h-7 px-2">
                          <span class="relative flex h-1.5 w-1.5 mr-1.5">
                            <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                            <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-white"></span>
                          </span>
                          {{ t('common.connected') }}
                        </Badge>
                      </template>
                      <template v-else>
                        <Button
                          variant="default"
                          size="sm"
                          class="h-7 text-xs"
                          :disabled="operationInProgress"
                          @click="connectImage(image)"
                        >
                          <Link v-if="!connecting" class="h-3.5 w-3.5 mr-1" />
                          <span v-if="connecting">{{ t('common.connecting') }}...</span>
                          <span v-else>{{ t('msd.connect') }}</span>
                        </Button>
                      </template>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-7 w-7 text-destructive hover:text-destructive"
                        :disabled="operationInProgress || (msdConnected && systemStore.msd?.imageId === image.id)"
                        @click="confirmDelete('image', image.id, image.name)"
                      >
                        <Trash2 class="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </div>
                </div>
              </div>

              <!-- System Storage Footer -->
              <div v-if="systemStore.diskSpace" class="pt-2 border-t mt-2">
                <p class="text-[11px] text-muted-foreground text-center">
                  {{ t('msd.systemAvailable') }}: {{ formatBytes(systemStore.diskSpace.available) }}
                </p>
              </div>
            </div>
          </TabsContent>

          <!-- Drive Tab -->
          <TabsContent value="drive" class="m-0 space-y-4 pr-4">
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
              <!-- Drive Info Card -->
              <div class="p-3 rounded-lg border space-y-3" :class="msdConnected && msdMode === 'drive' ? 'border-primary bg-primary/5' : 'bg-muted/50'">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-2">
                    <HardDrive class="h-4 w-4 text-muted-foreground" />
                    <span class="text-sm font-medium">{{ t('msd.drive') }}</span>
                    <Badge variant="outline" class="text-xs">
                      {{ Math.round((driveInfo?.size || 0) / 1024 / 1024) }} MB
                    </Badge>
                  </div>
                  <div class="flex items-center gap-1.5">
                    <template v-if="msdConnected && msdMode === 'drive'">
                      <Badge variant="default" class="text-xs h-7 px-2">
                        <span class="relative flex h-1.5 w-1.5 mr-1.5">
                          <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                          <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-white"></span>
                        </span>
                        {{ t('common.connected') }}
                      </Badge>
                    </template>
                    <template v-else>
                      <Button
                        variant="default"
                        size="sm"
                        class="h-7 text-xs"
                        :disabled="operationInProgress"
                        @click="connectDrive"
                      >
                        <Link v-if="!connecting" class="h-3.5 w-3.5 mr-1" />
                        <span v-if="connecting">{{ t('common.connecting') }}...</span>
                        <span v-else>{{ t('msd.connect') }}</span>
                      </Button>
                    </template>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-destructive hover:text-destructive"
                      :disabled="operationInProgress || msdConnected"
                      @click="showDeleteDriveDialog = true"
                    >
                      <Trash2 class="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
                <!-- Storage usage bar -->
                <div class="space-y-1.5">
                  <Progress
                    v-if="driveInfo"
                    :model-value="driveInfo.size > 0 ? (driveInfo.used / driveInfo.size) * 100 : 0"
                    class="h-2"
                  />
                  <div class="flex items-center justify-between text-xs text-muted-foreground">
                    <span>{{ formatBytes(driveInfo?.used || 0) }} {{ t('msd.usedSpace') }}</span>
                    <span>{{ formatBytes(driveInfo?.free || 0) }} {{ t('msd.freeSpace') }}</span>
                  </div>
                </div>
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
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <p class="text-sm font-medium truncate cursor-help">{{ file.name }}</p>
                          </TooltipTrigger>
                          <TooltipContent>
                            <p class="max-w-sm break-all">{{ file.name }}</p>
                          </TooltipContent>
                        </Tooltip>
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
    </DialogContent>
  </Dialog>
  </TooltipProvider>

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
        <Button variant="outline" @click="showDeleteDialog = false" :disabled="deleting">
          {{ t('common.cancel') }}
        </Button>
        <Button variant="destructive" @click="executeDelete" :disabled="deleting">
          <span v-if="deleting">{{ t('common.deleting') }}...</span>
          <span v-else>{{ t('common.delete') }}</span>
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <!-- Delete Drive Dialog -->
  <Dialog v-model:open="showDeleteDriveDialog">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{{ t('msd.deleteDrive') }}</DialogTitle>
        <DialogDescription>
          {{ t('msd.confirmDeleteDrive') }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" @click="showDeleteDriveDialog = false" :disabled="deletingDrive">
          {{ t('common.cancel') }}
        </Button>
        <Button variant="destructive" @click="deleteDrive" :disabled="deletingDrive">
          <span v-if="deletingDrive">{{ t('common.deleting') }}...</span>
          <span v-else>{{ t('common.delete') }}</span>
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <!-- Drive Init Dialog -->
  <Dialog v-model:open="showDriveInitDialog">
    <DialogContent class="max-w-md">
      <DialogHeader>
        <DialogTitle>{{ t('msd.initializeDrive') }}</DialogTitle>
        <DialogDescription>{{ t('msd.selectDriveSize') }}</DialogDescription>
      </DialogHeader>

      <div class="space-y-4">
        <!-- Preset size selection -->
        <div class="space-y-2">
          <Label>{{ t('msd.driveSize') }}</Label>
          <RadioGroup v-model="selectedDriveSize">
            <div v-for="size in driveSizeOptions" :key="size.value" class="flex items-center space-x-2">
              <RadioGroupItem :id="`size-${size.value}`" :value="size.value" />
              <Label :for="`size-${size.value}`" class="font-normal cursor-pointer flex-1">
                {{ size.label }}
              </Label>
            </div>
          </RadioGroup>
        </div>

        <!-- Custom size -->
        <div class="space-y-2">
          <Label for="custom-size">{{ t('msd.customSize') }}</Label>
          <div class="flex items-center gap-2">
            <Input
              id="custom-size"
              v-model.number="customDriveSize"
              type="number"
              :min="64"
              :max="32768"
              placeholder="256"
              class="flex-1"
            />
            <span class="text-sm text-muted-foreground">MB</span>
          </div>
          <p class="text-xs text-muted-foreground">
            {{ t('msd.driveSizeHint') }}
          </p>
        </div>

        <!-- Final size display -->
        <div class="p-3 rounded-lg bg-muted/50">
          <div class="flex items-center justify-between text-sm">
            <span class="text-muted-foreground">{{ t('msd.selectedSize') }}:</span>
            <span class="font-medium">{{ finalDriveSize }} MB</span>
          </div>
        </div>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="showDriveInitDialog = false" :disabled="initializingDrive">
          {{ t('common.cancel') }}
        </Button>
        <Button @click="createDrive" :disabled="initializingDrive">
          <span v-if="initializingDrive">{{ t('common.creating') }}...</span>
          <span v-else>{{ t('common.create') }}</span>
        </Button>
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

  <!-- URL Download Dialog -->
  <Dialog v-model:open="showUrlDialog">
    <DialogContent>
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Globe class="h-5 w-5" />
          {{ t('msd.downloadFromUrl') }}
        </DialogTitle>
        <DialogDescription>{{ t('msd.downloadFromUrlDesc') }}</DialogDescription>
      </DialogHeader>

      <div class="space-y-4">
        <div class="space-y-2">
          <Label for="download-url">{{ t('msd.url') }}</Label>
          <Input
            id="download-url"
            v-model="downloadUrl"
            placeholder="https://example.com/image.iso"
            :disabled="downloading"
          />
        </div>
        <div class="space-y-2">
          <Label for="download-filename">{{ t('msd.filename') }} ({{ t('common.optional') }})</Label>
          <Input
            id="download-filename"
            v-model="downloadFilename"
            :placeholder="t('msd.filenameAutoDetect')"
            :disabled="downloading"
          />
        </div>

        <!-- Download Progress -->
        <div v-if="downloadProgress" class="space-y-2 p-3 rounded-lg bg-muted/50">
          <div class="flex items-center justify-between text-sm">
            <span class="truncate">{{ downloadProgress.filename }}</span>
            <span class="text-muted-foreground shrink-0 ml-2">
              {{ downloadProgress.progress_pct?.toFixed(0) ?? 0 }}%
            </span>
          </div>
          <Progress :model-value="downloadProgress.progress_pct ?? 0" class="h-1.5" />
          <div class="flex items-center justify-between text-xs text-muted-foreground">
            <span>{{ formatBytes(downloadProgress.bytes_downloaded) }}</span>
            <span v-if="downloadProgress.total_bytes">
              / {{ formatBytes(downloadProgress.total_bytes) }}
            </span>
          </div>
          <div v-if="downloadProgress.status === 'completed'" class="text-xs text-green-600">
            {{ t('msd.downloadComplete') }}
          </div>
          <div v-else-if="downloadProgress.status.startsWith('failed')" class="text-xs text-destructive">
            {{ downloadProgress.status }}
          </div>
        </div>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="showUrlDialog = false; resetDownloadState()">
          {{ t('common.cancel') }}
        </Button>
        <Button
          v-if="!downloading"
          :disabled="!downloadUrl.trim()"
          @click="startUrlDownload"
        >
          <Download class="h-4 w-4 mr-1" />
          {{ t('msd.download') }}
        </Button>
        <Button v-else variant="destructive" @click="cancelUrlDownload">
          <X class="h-4 w-4 mr-1" />
          {{ t('common.cancel') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
