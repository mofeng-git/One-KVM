import type { HidConfig, HidConfigUpdate } from '../types/generated'
import type { BluetoothStatus } from '../types/bluetooth'
export type Driver = 'otg' | 'ch9329' | 'bluetooth' | 'none'
export interface HidSelection {
  backend: Driver; otg_udc: string; ch9329_port: string; ch9329_baudrate: number
  bluetooth: { adapter: string; name: string }
}
export function selectionFrom(hid?: HidConfig | null): HidSelection {
  return { backend: hid?.backend ?? 'ch9329', otg_udc: hid?.otg_udc ?? '',
    ch9329_port: hid?.ch9329_port ?? '', ch9329_baudrate: hid?.ch9329_baudrate ?? 9600,
    bluetooth: { adapter: hid?.bluetooth.adapter ?? '', name: hid?.bluetooth.name ?? 'One-KVM HID' } }
}
export function selectDevice(saved: string, candidates: string[]): string {
  return candidates.includes(saved) ? saved : candidates.length === 1 ? candidates[0]! : ''
}
export function validName(name: string): boolean {
  const length = new TextEncoder().encode(name).length
  return length > 0 && length <= 64 && !/[\u0000-\u001f\u007f-\u009f]/u.test(name)
}
export function deviceRequest(draft: HidSelection): HidConfigUpdate {
  const backend = draft.backend as HidConfigUpdate['backend']
  switch (draft.backend) {
    case 'otg': return { backend, otg_udc: draft.otg_udc }
    case 'ch9329': return { backend, ch9329_port: draft.ch9329_port, ch9329_baudrate: draft.ch9329_baudrate }
    case 'bluetooth': return { backend, bluetooth: { ...draft.bluetooth }, bluetooth_reset_pairing: true }
    case 'none': return { backend }
  }
}
export function matchesSelection(hid: HidConfig, draft: HidSelection): boolean {
  if (hid.backend !== draft.backend) return false
  switch (draft.backend) {
    case 'otg': return hid.otg_udc === draft.otg_udc
    case 'ch9329': return hid.ch9329_port === draft.ch9329_port && hid.ch9329_baudrate === draft.ch9329_baudrate
    case 'bluetooth': return hid.bluetooth.adapter === draft.bluetooth.adapter && hid.bluetooth.name === draft.bluetooth.name
    case 'none': return true
  }
}
export function bluetoothStage(status: BluetoothStatus | null): string {
  if (!status || status.error) return 'unknown'
  if (status.ready) return 'ready'
  if (status.connected) return 'preparing'
  if (status.devices.some(d => d.address === status.peer && d.paired)) return 'paired'
  if (status.pairing_seconds > 0) return 'pairing'
  return status.initialized ? 'waiting' : 'initializing'
}
export interface HidDeviceStatus {
  backend: string; online: boolean; error?: string | null; error_code?: string | null
}
export function hidDeviceError(status?: HidDeviceStatus | null): string | null {
  // An unplugged OTG cable is an expected connection state, not a hardware fault.
  if (status?.backend === 'otg' && status.error_code === 'udc_not_configured') return null
  return status?.error ?? null
}
export function hidDeviceStage(status: HidDeviceStatus | null | undefined, bluetooth: BluetoothStatus | null, readError = ''): string {
  if (!status || readError || hidDeviceError(status)) return 'unknown'
  if (status.backend === 'bluetooth') return bluetoothStage(bluetooth)
  if (status.backend === 'otg' && status.error_code === 'udc_not_configured') return 'waiting'
  return status.online ? 'ready' : 'waiting'
}
export const pendingHidKey = 'one-kvm.pending-hid.v1'
export interface PendingHid { selection: HidSelection; phase: 'selected' | 'applying' | 'applied' }
export function readPendingHid(): PendingHid | null {
  try {
    const value = JSON.parse(sessionStorage.getItem(pendingHidKey) ?? 'null')
    if (value && ['selected', 'applying', 'applied'].includes(value.phase)
      && ['otg', 'ch9329', 'bluetooth', 'none'].includes(value.selection?.backend)
      && typeof value.selection.otg_udc === 'string' && typeof value.selection.ch9329_port === 'string'
      && typeof value.selection.ch9329_baudrate === 'number'
      && typeof value.selection.bluetooth?.adapter === 'string' && typeof value.selection.bluetooth?.name === 'string') return value
  } catch { /* Invalid or unavailable session storage is not an applied configuration. */ }
  return null
}
export function writePendingHid(value: PendingHid) { sessionStorage.setItem(pendingHidKey, JSON.stringify(value)) }
