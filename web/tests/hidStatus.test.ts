import assert from 'node:assert/strict'
import { getHidStatus } from '../src/lib/hidStatus'

const hid = { available: true, initialized: true, online: true, backend: 'ch9329' }
const transport = {
  useWebRtc: true, dataChannelReady: false, rtcConnecting: true, rtcConnected: false,
  wsConnected: true, wsNetworkError: false, wsHidUnavailable: false,
}

assert.equal(getHidStatus(hid, transport), 'connected', 'working WS remains usable while video connects')
assert.equal(getHidStatus(hid, { ...transport, rtcConnecting: false, rtcConnected: true }), 'connected')
assert.equal(getHidStatus(hid, { ...transport, wsConnected: false, dataChannelReady: true }), 'connected')
assert.equal(getHidStatus(hid, { ...transport, wsConnected: false }), 'connecting')
assert.notEqual(getHidStatus({ ...hid, online: false }, transport), 'connected')
assert.notEqual(getHidStatus({ ...hid, available: false }, transport), 'connected')
console.log('6 HID transport status checks passed')
