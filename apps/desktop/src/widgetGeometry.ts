import type { ThemeLayout } from '@silverwolf/core'
import { defaultLayout } from '@silverwolf/core'

export const stageSize = defaultLayout
export const squash = { x: 1.06, y: 0.84 }
export const bounce = [squash, { x: 0.97, y: 1.04 }, { x: 1.025, y: 0.97 }, { x: 0.995, y: 1.01 }, { x: 1, y: 1 }]

export function motionFrames(layout: ThemeLayout) {
  const center = layout.width / 2
  const rects = [layout.character, layout.bubble, ...[layout.actions.x, layout.actions.leftX].map(x => ({ x, y: layout.actions.y, width: 32, height: 100 }))]
  const reach = Math.max(...rects.flatMap(r => [Math.abs(r.x - center), Math.abs(r.x + r.width - center)]))
  const maxX = center / reach
  const maxY = layout.height / (layout.height - Math.min(...rects.map(r => r.y)))
  return bounce.map(frame => ({ x: Math.min(frame.x, maxX), y: Math.min(frame.y, maxY) }))
}

// The settings panel is outside the motion container; include its shadow and focus ring.
export function staticRegion(rect: { x: number, y: number, width: number, height: number }, scale: number, origin: { x: number, y: number }) {
  const padding = 12
  return {
    x: origin.x + (rect.x - padding) * scale,
    y: origin.y + (rect.y - padding) * scale,
    width: (rect.width + padding * 2) * scale,
    height: (rect.height + padding * 2) * scale,
  }
}

// Reserve all animation bounds before it starts; animated DOM bounds would
// otherwise clip the cloud on subsequent frames in the native Windows region.
export function motionRegion(rect: { x: number, y: number, width: number, height: number }, scale: number, origin: { x: number, y: number }, canvas = stageSize) {
  const center = canvas.width / 2
  const bottom = canvas.height
  const frames = motionFrames(canvas)
  const left = Math.min(...frames.map(s => center + (rect.x - center) * s.x)) - 2
  const top = Math.min(...frames.map(s => bottom + (rect.y - bottom) * s.y)) - 2
  const right = Math.max(...frames.map(s => center + (rect.x + rect.width - center) * s.x)) + 2
  const end = Math.max(...frames.map(s => bottom + (rect.y + rect.height - bottom) * s.y)) + 2
  return { x: origin.x + left * scale, y: origin.y + top * scale, width: (right - left) * scale, height: (end - top) * scale }
}
