export type LayoutPreset = 'horizontal' | 'vertical' | 'grid' | 'auto'
export interface FrameSize { id: string; width: number; height: number }
export interface FrameRect extends FrameSize { x: number; y: number }
export interface CanvasLayout { width: number; height: number; frames: FrameRect[] }

/** Pure native-pixel geometry. Tile chrome never participates in these bounds. */
export function videoStageGeometry(input: FrameSize[], preset: LayoutPreset, aspect = 16 / 9): CanvasLayout {
  const frames = input.map(frame => ({ ...frame,
    width: Number.isFinite(frame.width) && frame.width > 0 ? frame.width : 1920,
    height: Number.isFinite(frame.height) && frame.height > 0 ? frame.height : 1080,
  }))
  if (!frames.length) return { width: 1, height: 1, frames: [] }
  if (preset === 'horizontal') {
    let x = 0
    return { width: frames.reduce((sum, f) => sum + f.width, 0), height: Math.max(...frames.map(f => f.height)),
      frames: frames.map(f => { const rect = { ...f, x, y: 0 }; x += f.width; return rect }) }
  }
  if (preset === 'vertical') {
    let y = 0
    return { width: Math.max(...frames.map(f => f.width)), height: frames.reduce((sum, f) => sum + f.height, 0),
      frames: frames.map(f => { const rect = { ...f, x: 0, y }; y += f.height; return rect }) }
  }
  if (preset === 'grid') {
    const columns = Math.max(1, Math.min(frames.length, Math.ceil(Math.sqrt(frames.length * aspect / (16 / 9)))))
    const rows = Math.ceil(frames.length / columns)
    const width = Math.max(...frames.map(f => f.width))
    const height = Math.max(...frames.map(f => f.height))
    return { width: width * columns, height: height * rows,
      frames: frames.map((f, i) => ({ ...f, x: (i % columns) * width, y: Math.floor(i / columns) * height, width, height })) }
  }
  // The accepted 2+1 arrangement is independent of viewport shape and source order.
  const byArea = [...frames].sort((a, b) => a.width * a.height - b.width * b.height)
  const [a, b, c] = byArea
  if (frames.length === 3 && a && b && c && a.width === b.width && a.height === b.height
      && c.width === a.width * 2 && c.height === a.height * 2) {
    return { width: a.width + c.width, height: c.height,
      frames: [{ ...a, x: 0, y: 0 }, { ...b, x: 0, y: a.height }, { ...c, x: a.width, y: 0 }] }
  }
  // Bounded column candidates: native sizes and a common scale for every source.
  let best: CanvasLayout | undefined
  let bestScore = -Infinity
  for (let count = 1; count <= Math.min(frames.length, 8); count++) {
    const columns = Array.from({ length: count }, () => ({ height: 0, width: 0, frames: [] as FrameRect[] }))
    for (const frame of [...byArea].reverse()) {
      const column = columns.reduce((shortest, col) => col.height < shortest.height ? col : shortest)
      column.frames.push({ ...frame, x: 0, y: column.height })
      column.height += frame.height
      column.width = Math.max(column.width, frame.width)
    }
    columns.sort((left, right) => left.width - right.width)
    let x = 0
    const rects = columns.flatMap(column => {
      const result = column.frames.map(frame => ({ ...frame, x: x + (column.width - frame.width) / 2 }))
      x += column.width
      return result
    })
    const height = Math.max(...columns.map(column => column.height))
    const scale = Math.min(aspect / x, 1 / height)
    const score = frames.reduce((sum, f) => sum + f.width * f.height, 0) * scale * scale
    if (score > bestScore) { bestScore = score; best = { width: x, height, frames: rects } }
  }
  return best!
}
