import type { HidConfig, HidConfigUpdate } from '../types/generated'
import type { BluetoothStatus } from '../types/bluetooth'
export type Driver = 'otg' | 'ch9329' | 'ch9329_otg' | 'bluetooth' | 'none'
export function usesCh9329(backend?: string): boolean {
  return backend === 'ch9329' || backend === 'ch9329_otg'
}
export function supportsOtgFunctions(backend?: string): boolean {
  return backend === 'otg' || backend === 'ch9329_otg'
}
export function runtimeBackend(backend?: string): string | undefined {
  return backend === 'ch9329_otg' ? 'ch9329' : backend
}
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
  return saved || (candidates.length === 1 ? candidates[0]! : '')
}
export function validName(name: string): boolean {
  const length = new TextEncoder().encode(name).length
  return length > 0 && length <= 64 && !/[\u0000-\u001f\u007f-\u009f]/u.test(name)
}
export function deviceRequest(draft: HidSelection): HidConfigUpdate {
  const backend = draft.backend as HidConfigUpdate['backend']
  switch (draft.backend) {
    case 'otg': return { backend, otg_udc: draft.otg_udc }
    case 'ch9329_otg': return { backend, otg_udc: draft.otg_udc, ch9329_port: draft.ch9329_port, ch9329_baudrate: draft.ch9329_baudrate }
    case 'ch9329': return { backend, ch9329_port: draft.ch9329_port, ch9329_baudrate: draft.ch9329_baudrate }
    case 'bluetooth': return { backend, bluetooth: { ...draft.bluetooth }, bluetooth_reset_pairing: true }
    case 'none': return { backend }
  }
}
export function matchesSelection(hid: HidConfig, draft: HidSelection): boolean {
  if (hid.backend !== draft.backend) return false
  switch (draft.backend) {
    case 'otg': return hid.otg_udc === draft.otg_udc
    case 'ch9329_otg': return hid.otg_udc === draft.otg_udc && hid.ch9329_port === draft.ch9329_port && hid.ch9329_baudrate === draft.ch9329_baudrate
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
export interface OtgDeviceStatus {
  enabled: boolean
  health: 'healthy' | 'applying' | 'degraded'
  gadget_active: boolean
  udc: string | null
  udc_state: string | null
  error: string | null
}
export function otgDeviceStage(status?: OtgDeviceStatus | null): string {
  if (!status) return 'unknown'
  if (!status.enabled) return 'otgInactive'
  if (status.health === 'applying') return 'initializing'
  if (status.health === 'degraded' || status.error) return 'unknown'
  if (!status.gadget_active) return 'unknown'
  return status.udc_state === 'configured' ? 'otgConfigured' : 'otgWaiting'
}
export interface HidDeviceStatus {
  otg?: OtgDeviceStatus | null
  configured_backend?: string
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
