import assert from 'node:assert/strict'
import { it } from 'node:test'
import { bounce, motionRegion, stageSize, staticRegion } from './widgetGeometry.ts'

it('keeps cloud, character and either toolbar side inside the native region throughout scaled motion', () => {
  for (const scale of [0.6, 0.8, 1, 1.6]) {
    const origin = { x: 19, y: 23 }
    for (const rect of [
      { x: 12, y: 18, width: 284, height: 254 },
      { x: 82, y: 186, width: 238, height: 244 },
      { x: 328, y: 282, width: 32, height: 140 },
      { x: 42, y: 282, width: 32, height: 140 },
    ]) {
      const region = motionRegion(rect, scale, origin)
      for (let i = 0; i < bounce.length - 1; i++) {
        for (const progress of [0, 0.25, 0.5, 0.75, 1]) {
          const sx = bounce[i]!.x + (bounce[i + 1]!.x - bounce[i]!.x) * progress
          const sy = bounce[i]!.y + (bounce[i + 1]!.y - bounce[i]!.y) * progress
          for (const x of [rect.x, rect.x + rect.width]) {
            for (const y of [rect.y, rect.y + rect.height]) {
              const screenX = origin.x + (stageSize.width / 2 + (x - stageSize.width / 2) * sx) * scale
              const screenY = origin.y + (stageSize.height + (y - stageSize.height) * sy) * scale
              assert.ok(screenX >= region.x && screenX <= region.x + region.width)
              assert.ok(screenY >= region.y && screenY <= region.y + region.height)
            }
          }
        }
      }
    }
  }
})

it('keeps the static settings panel and shadow inside the stage at all supported scales', () => {
  for (const scale of [0.6, 1, 1.6]) {
    const origin = { x: 19, y: 23 }
    const rect = { x: 24, y: 24, width: 304, height: 360 }
    const region = staticRegion(rect, scale, origin)
    assert.ok(region.x >= origin.x && region.y >= origin.y)
    assert.ok(region.x + region.width <= origin.x + stageSize.width * scale)
    assert.ok(region.y + region.height <= origin.y + stageSize.height * scale)
    assert.ok(region.width >= rect.width * scale)
    assert.ok(region.height >= rect.height * scale)
  }
})
