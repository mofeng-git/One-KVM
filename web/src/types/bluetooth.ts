export interface BluetoothAdapter { name: string; address: string; powered: boolean }
export interface BluetoothStatus {
  initialized: boolean; connected: boolean; ready: boolean; adapter: string; adapter_address: string
  peer?: string | null; pairing_seconds: number; control_connected: boolean; interrupt_connected: boolean
  error?: string | null
  devices: Array<{ address: string; name: string; paired: boolean; connected: boolean }>
}
