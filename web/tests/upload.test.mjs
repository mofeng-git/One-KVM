import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import vm from 'node:vm'
import ts from 'typescript'

function api(status, body) {
  class Xhr {
    upload = {}
    status = status
    responseText = body
    open() {}
    send() { queueMicrotask(() => this.onload()) }
  }
  const source = readFileSync(new URL('../src/api/request.ts', import.meta.url), 'utf8')
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } })
  const context = { exports: {}, XMLHttpRequest: Xhr, console, require: name => name === 'vue-sonner'
    ? { toast: { error() {} } }
    : { default: { global: { t: key => key, te: () => false } } } }
  vm.runInNewContext(outputText, context)
  return context.exports
}

test('upload rejects a backend failure even with HTTP 200', async () => {
  const { uploadRequest } = api(200, '{"success":false,"message":"disk full","code":"NO_SPACE"}')
  await assert.rejects(uploadRequest('/upload', {}), error => error.status === 200 && error.code === 'NO_SPACE' && error.message === 'disk full')
})
test('upload rejects malformed JSON and HTTP errors', async () => {
  for (const [status, body] of [[200, 'not json'], [401, '{"message":"Session expired"}']]) {
    await assert.rejects(api(status, body).uploadRequest('/upload', {}), error => error.status === status)
  }
})
test('upload accepts successful JSON without imposing a success property', async () => {
  assert.equal((await api(201, '{"id":"image"}').uploadRequest('/upload', {})).id, 'image')
})
