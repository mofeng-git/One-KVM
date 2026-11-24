import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import vm from 'node:vm'
import ts from 'typescript'

const source = readFileSync(new URL('../src/lib/pluginNavigation.ts', import.meta.url), 'utf8')
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } })
const context = { exports: {} }
vm.runInNewContext(outputText, context)
const { pluginText, pluginMenuId, pluginSettingsSection, pluginMenuLabel } = context.exports

test('first-party manifests use their intended destinations and localize display text', () => {
  const destinations = {
    vnc: 'third-party-access',
    rtsp: 'third-party-access',
    rustdesk: 'third-party-access',
    easytier: 'ext-remote-access',
    easyfrp: 'ext-remote-access',
    frpc: 'ext-remote-access',
    gostc: 'ext-remote-access',
    airferry: undefined,
  }
  for (const [id, expected] of Object.entries(destinations)) {
    const manifest = JSON.parse(readFileSync(new URL(`../../plugins/${id}/manifest.json`, import.meta.url)))
    assert.equal(typeof manifest.author, 'string')
    assert.ok(manifest.author.trim())
    assert.equal(pluginSettingsSection(pluginMenuId({ manifest })), expected)
    if (id === 'airferry') assert.equal(pluginText(manifest.name, 'zh-CN'), 'AirFerry')
    for (const label of [
      ...(id === 'airferry' ? [] : [manifest.name]), manifest.description,
      ...manifest.config_schema.map(field => field.label),
      ...manifest.status_fields.map(field => field.label),
      ...(manifest.actions ?? []).map(action => action.label),
    ]) {
      assert.ok(pluginText(label, 'en-US'))
      assert.match(pluginText(label, 'zh-CN'), /\p{Script=Han}/u)
    }
  }
})

test('custom menu IDs create stable tabs with translated labels', () => {
  const plugin = {
    manifest: { menu: { id: 'lab-tools', label: { 'en-US': 'Lab equipment', 'zh-CN': '实验台工具' } } },
  }
  assert.equal(pluginSettingsSection(pluginMenuId(plugin)), 'plugin-menu-lab-tools')
  assert.equal(pluginMenuLabel([plugin], 'lab-tools', 'zh-CN', key => key), '实验台工具')
  assert.equal(pluginMenuLabel([plugin], 'lab-tools', 'en-GB', key => key), 'Lab equipment')
})

test('custom menu IDs are namespaced separately from built-in settings sections', () => {
  const plugin = { manifest: { menu: 'plugin-management' } }
  assert.equal(pluginSettingsSection(pluginMenuId(plugin)), 'plugin-menu-plugin-management')
})
