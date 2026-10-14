import type { QuotaMeter } from './types.ts'
import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { isStale, resetLabel, windowLabel } from './format.ts'

describe('quota presentation', () => {
  it('formats reset countdowns with days, hours and rounded-up minutes', () => {
    const now = 1_000_000
    const label = (minutes: number) => resetLabel(now + minutes * 60_000, now)
    assert.equal(label(3 * 1440 + 6 * 60 + 23), '3d6h23m')
    assert.equal(label(1440), '1d0h0m')
    assert.equal(label(60), '1h0m')
    assert.equal(label(59), '59m')
    assert.equal(label(0.01), '1m')
    assert.equal(label(0), '等待刷新')
    assert.equal(label(-1), '等待刷新')
  })

  it('keeps arbitrary account windows instead of assuming two fixed windows', () => {
    assert.equal(windowLabel(300), '5 小时')
    assert.equal(windowLabel(10080), '每周')
    assert.equal(windowLabel(43200), '30 天')
    assert.equal(windowLabel(43800), '730 小时')
    assert.equal(windowLabel(37), '37 分钟')
  })

  it('marks old or reset snapshots as stale without manufacturing new quota', () => {
    const meter: QuotaMeter = { kind: 'quota', id: 'codex:primary', label: 'Codex', remainingPercent: 8, durationMinutes: 300, resetsAt: 400_000 }
    assert.equal(isStale(100_000, meter, 150_000), false)
    assert.equal(isStale(100_000, meter, 300_001), true)
    assert.equal(isStale(399_000, meter, 400_000), true)
    assert.equal(meter.remainingPercent, 8)
    assert.equal(resetLabel(null), '重置时间未知')
    assert.equal(resetLabel(1000, 1001), '等待刷新')
  })
})
