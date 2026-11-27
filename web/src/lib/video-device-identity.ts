import type { VideoDevice } from '@/api'

export function matchesVideoDevice(device: VideoDevice, path: string): boolean {
  return device.path === path || device.aliases?.includes(path) === true
}

export function preferredVideoPath(device: VideoDevice): string {
  return device.aliases?.find(path => path.startsWith('/dev/one-kvm-'))
    ?? device.aliases?.find(path => path.startsWith('/dev/v4l/by-path/'))
    ?? device.aliases?.find(path => path.startsWith('/dev/v4l/by-id/'))
    ?? device.path
}
