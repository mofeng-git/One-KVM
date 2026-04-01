export function formatFpsValue(fps: number): string {
  if (!Number.isFinite(fps)) return '0'

  const rounded = Math.round(fps * 100) / 100
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(2).replace(/\.?0+$/, '')
}

export function formatFpsLabel(fps: number): string {
  return `${formatFpsValue(fps)} FPS`
}

export function toConfigFps(fps: number): number {
  if (!Number.isFinite(fps)) return 30
  return Math.round(fps)
}
