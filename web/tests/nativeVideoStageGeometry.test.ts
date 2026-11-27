import { test } from 'node:test'
import assert from 'node:assert/strict'
import { videoStageGeometry, type FrameSize, type LayoutPreset } from '../src/lib/videoStageGeometry'

test('accepted three displays keep native pixels in a 5760×2160 canvas, independent of source order', () => {
  const geometry = videoStageGeometry([
    { id: '4k', width: 3840, height: 2160 },
    { id: 'a', width: 1920, height: 1080 },
    { id: 'b', width: 1920, height: 1080 },
  ], 'auto', 9 / 16)
  assert.equal(geometry.width, 5760)
  assert.equal(geometry.height, 2160)
  assert.deepEqual(geometry.frames.map(f => [f.id, f.x, f.y, f.width, f.height]), [
    ['a', 0, 0, 1920, 1080], ['b', 0, 1080, 1920, 1080], ['4k', 1920, 0, 3840, 2160],
  ])
})

for (const count of [1, 2, 3, 4, 5, 9, 16]) {
  for (const preset of ['horizontal', 'vertical', 'grid', 'auto'] as LayoutPreset[]) {
    test(`${preset} contains ${count} mixed landscape/portrait displays without overlap`, () => {
      const input: FrameSize[] = Array.from({ length: count }, (_, i) => ({ id: String(i),
        width: i % 2 ? 1080 : 2560, height: i % 2 ? 1920 : 1440 }))
      for (const aspect of [9 / 16, 16 / 9, 3]) {
        const geometry = videoStageGeometry(input, preset, aspect)
        assert.equal(geometry.frames.length, count)
        assert.equal(new Set(geometry.frames.map(f => f.id)).size, count)
        for (const [i, frame] of geometry.frames.entries()) {
          assert.ok(frame.x >= 0 && frame.y >= 0)
          assert.ok(frame.x + frame.width <= geometry.width && frame.y + frame.height <= geometry.height)
          if (preset !== 'grid') {
            const original = input.find(f => f.id === frame.id)!
            assert.equal(frame.width, original.width)
            assert.equal(frame.height, original.height)
          }
          for (const other of geometry.frames.slice(i + 1)) {
            assert.ok(frame.x + frame.width <= other.x || other.x + other.width <= frame.x
              || frame.y + frame.height <= other.y || other.y + other.height <= frame.y)
          }
        }
      }
    })
  }
}

test('missing/invalid startup frame sizes and empty collections remain finite', () => {
  assert.deepEqual(videoStageGeometry([], 'auto'), { width: 1, height: 1, frames: [] })
  const geometry = videoStageGeometry([{ id: 'new', width: NaN, height: 0 }], 'auto')
  assert.equal(geometry.width, 1920)
  assert.equal(geometry.height, 1080)
})
