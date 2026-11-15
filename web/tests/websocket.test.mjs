import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import vm from 'node:vm'
import ts from 'typescript'

function harness(module = 'useWebSocket') {
  const sockets = []
  const timers = new Map()
  let timerId = 0
  class Socket {
    static CONNECTING = 0
    static OPEN = 1
    readyState = 0
    sent = []
    constructor() { sockets.push(this) }
    send(data) { this.sent.push(JSON.parse(data)) }
    close() { this.readyState = 3 }
    open() { this.readyState = 1; this.onopen() }
  }
  const source = readFileSync(new URL(`../src/composables/${module}.ts`, import.meta.url), 'utf8')
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } })
  const context = {
    exports: {}, console, WebSocket: Socket, ArrayBuffer, DataView,
    setTimeout: fn => { timers.set(++timerId, fn); return timerId },
    clearTimeout: id => timers.delete(id),
    require: name => name === 'vue' ? { ref: value => ({ value }) } : {
      buildWsUrl: path => path, WS_RECONNECT_DELAY: 1000, RESP_OK: 0,
    },
  }
  context.window = { ...context, addEventListener() {} }
  vm.runInNewContext(outputText, context)
  return { api: context.exports[module](), sockets, timers }
}

test('concurrent callers share a connecting socket and receive subscriptions', () => {
  const { api, sockets } = harness()
  api.connect(); api.connect()
  assert.equal(sockets.length, 1)
  api.on('status', () => {})
  sockets[0].open()
  assert.equal(sockets[0].sent[0].payload.topics[0], 'status')
})

test('intentional disconnect does not reconnect when close arrives asynchronously', () => {
  const { api, sockets, timers } = harness()
  api.connect(); sockets[0].open(); api.disconnect(); sockets[0].onclose()
  assert.equal(api.connected.value, false)
  assert.equal(api.networkError.value, false)
  assert.equal(timers.size, 0)
})

test('disconnect cancels a pending retry', () => {
  const { api, sockets, timers } = harness()
  api.connect(); sockets[0].onclose()
  assert.equal(timers.size, 1)
  api.disconnect()
  assert.equal(timers.size, 0)
})

test('stale close cannot disconnect a new socket', () => {
  const { api, sockets, timers } = harness()
  api.connect(); api.disconnect(); api.connect(); sockets[1].open()
  sockets[0].onclose()
  assert.equal(api.connected.value, true)
  assert.equal(timers.size, 0)
})

test('network closure retries and restores subscriptions', () => {
  const { api, sockets, timers } = harness()
  api.on('status', () => {}); api.connect(); sockets[0].open(); sockets[0].onclose()
  const retry = [...timers.values()][0]
  retry(); sockets[1].open()
  assert.equal(api.connected.value, true)
  assert.equal(sockets[1].sent[0].payload.topics[0], 'status')
})

for (const module of ['useWebSocket', 'useHidWebSocket']) {
  test(`${module}: disconnect and late close do not revive a cancelled connection`, async () => {
    const { api, sockets, timers } = harness(module)
    const pending = api.connect()
    api.disconnect()
    sockets[0].onclose()
    assert.equal(timers.size, 0)
    assert.equal(api.connected.value, false)
    if (module === 'useHidWebSocket') assert.equal(await pending, false)
  })
}

test('HID: close before handshake settles the caller and schedules recovery', async () => {
  const { api, sockets, timers } = harness('useHidWebSocket')
  const pending = api.connect()
  assert.equal(api.connect(), pending)
  sockets[0].onclose()
  assert.equal(await pending, false)
  assert.equal(timers.size, 1)
  api.disconnect()
  assert.equal(timers.size, 0)
})

test('HID: late close of a replaced socket leaves the new handshake intact', async () => {
  const { api, sockets, timers } = harness('useHidWebSocket')
  const old = api.connect(); api.disconnect()
  assert.equal(await old, false)
  const current = api.connect(); sockets[1].open(); sockets[0].onclose()
  sockets[1].onmessage({ data: Uint8Array.of(0).buffer })
  assert.equal(await current, true)
  assert.equal(api.connected.value, true)
  assert.equal(timers.size, 0)
})
