import type { UsageSnapshot } from '@silverwolf/core'
import assert from 'node:assert/strict'
import { it } from 'node:test'
import { createUsageState } from './usageState.ts'

function snapshot(percent: number): UsageSnapshot {
  return { provider: 'codex', plan: null, observedAt: percent * 1000, meters: [{ kind: 'quota', id: 'primary', label: 'Codex', durationMinutes: 300, resetsAt: null, remainingPercent: percent }] }
}
function fixture() {
  const requests: { resolve: (value: UsageSnapshot) => void, reject: (error: Error) => void }[] = []
  const state = createUsageState(() => new Promise((resolve, reject) => requests.push({ resolve, reject })))
  return { state, requests }
}

it('starts empty and keeps the complete old snapshot visible until refresh succeeds', async () => {
  const { state, requests } = fixture()
  const first = state.refresh()
  assert.equal(state.snapshot.value, null)
  assert.equal(state.loading.value, true)
  const original = snapshot(70)
  requests[0]!.resolve(original)
  await first
  const next = state.refresh()
  assert.equal(state.loading.value, true)
  assert.equal(state.snapshot.value, original)
  await state.refresh()
  assert.equal(requests.length, 2)
  const replacement = snapshot(60)
  requests[1]!.resolve(replacement)
  await next
  assert.equal(state.snapshot.value, replacement)
  assert.equal(state.loading.value, false)
})

it('clears stale data on failed refresh and recovers on retry including empty results', async () => {
  const { state, requests } = fixture()
  const first = state.refresh()
  requests[0]!.resolve(snapshot(70))
  await first
  const next = state.refresh()
  requests[1]!.reject(new Error('登录失效'))
  await next
  assert.equal(state.snapshot.value, null)
  assert.equal(state.error.value?.message, '登录失效')
  assert.equal(state.loading.value, false)
  const retry = state.refresh()
  assert.equal(state.error.value, null)
  requests[2]!.resolve({ ...snapshot(60), meters: [] })
  await retry
  assert.deepEqual(state.snapshot.value?.meters, [])
})

it('connection changes clear previous data and reject an in-flight result from the old connection', async () => {
  const { state, requests } = fixture()
  const first = state.refresh()
  requests[0]!.resolve(snapshot(70))
  await first
  const oldRequest = state.refresh()
  state.reconnect()
  state.reconnect()
  assert.equal(state.snapshot.value, null)
  requests[1]!.resolve(snapshot(50))
  await oldRequest
  assert.equal(state.snapshot.value, null)
  assert.equal(state.loading.value, true)
  assert.equal(requests.length, 3)
  requests[2]!.resolve(snapshot(90))
  await Promise.resolve()
  assert.equal(state.snapshot.value?.observedAt, 90_000)
  assert.equal(state.loading.value, false)
})

it('does not apply late responses or start new requests after disposal', async () => {
  const { state, requests } = fixture()
  const pending = state.refresh()
  state.reconnect()
  state.dispose()
  requests[0]!.resolve(snapshot(70))
  await pending
  await state.refresh()
  state.reconnect()
  assert.equal(state.snapshot.value, null)
  assert.equal(requests.length, 1)
})
