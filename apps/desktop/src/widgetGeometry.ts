export const stageSize = { width: 376, height: 430 }
export const squash = { x: 1.06, y: 0.84 }
export const bounce = [squash, { x: 0.97, y: 1.04 }, { x: 1.025, y: 0.97 }, { x: 0.995, y: 1.01 }, { x: 1, y: 1 }]

// Reserve all animation bounds before it starts; animated DOM bounds would
// otherwise clip the cloud on subsequent frames in the native Windows region.
export function motionRegion(rect: { x: number, y: number, width: number, height: number }, scale: number, origin: { x: number, y: number }) {
  const center = stageSize.width / 2
  const bottom = stageSize.height
  const left = Math.min(...bounce.map(s => center + (rect.x - center) * s.x)) - 2
  const top = Math.min(...bounce.map(s => bottom + (rect.y - bottom) * s.y)) - 2
  const right = Math.max(...bounce.map(s => center + (rect.x + rect.width - center) * s.x)) + 2
  const end = Math.max(...bounce.map(s => bottom + (rect.y + rect.height - bottom) * s.y)) + 2
  return { x: origin.x + left * scale, y: origin.y + top * scale, width: (right - left) * scale, height: (end - top) * scale }
}
