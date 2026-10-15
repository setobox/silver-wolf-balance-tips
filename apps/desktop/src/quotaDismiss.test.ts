import type { DismissState } from './quotaDismiss.ts'
import assert from 'node:assert/strict'
import { it } from 'node:test'
import { createDismissTimer, validDismissSeconds } from './quotaDismiss.ts'

const open: DismissState = { expanded: true, seconds: 1, loading: false, settingsOpen: false, hovered: false, keyboardFocused: false }

function setup() {
  let now = 0
  let dismissed = 0
  const tasks: { at: number, callback: () => void, cancelled: boolean }[] = []
  const timer = createDismissTimer(() => dismissed++, {
    schedule(callback, delay) {
      const task = { at: now + delay, callback, cancelled: false }
      tasks.push(task)
      return () => {
        task.cancelled = true
      }
    },
  })
  return {
    timer,
    tasks,
    count: () => dismissed,
    tick(ms: number) {
      now += ms
      for (const task of tasks) {
        if (!task.cancelled && task.at <= now) {
          task.cancelled = true
          task.callback()
        }
      }
    },
  }
}

it('dismisses at one second, three seconds and a custom duration, never earlier', () => {
  for (const seconds of [1, 3, 17, 3600]) {
    const clock = setup()
    clock.timer.update({ ...open, seconds })
    clock.tick(seconds * 1000 - 1)
    assert.equal(clock.count(), 0)
    clock.tick(1)
    assert.equal(clock.count(), 1)
    clock.tick(5000)
    assert.equal(clock.count(), 1)
  }
})

it('pauses for every interaction and restarts a full countdown when all protections clear', () => {
  for (const protection of ['loading', 'settingsOpen', 'hovered', 'keyboardFocused'] as const) {
    const clock = setup()
    clock.timer.update(open)
    clock.tick(900)
    clock.timer.update({ ...open, [protection]: true })
    clock.tick(5000)
    assert.equal(clock.count(), 0)
    clock.timer.update({ ...open, [protection]: true, hovered: true })
    clock.timer.update({ ...open, hovered: true })
    clock.tick(5000)
    assert.equal(clock.count(), 0)
    clock.timer.update(open)
    clock.tick(999)
    assert.equal(clock.count(), 0)
    clock.tick(1)
    assert.equal(clock.count(), 1)
  }
})

it('cancels on close, persistent mode, duration changes and disposal, including stale callbacks', () => {
  const clock = setup()
  clock.timer.update(open)
  const stale = clock.tasks[0]!.callback
  clock.timer.update({ ...open, expanded: false })
  stale()
  clock.tick(2000)
  assert.equal(clock.count(), 0)
  clock.timer.update(open)
  clock.timer.update({ ...open, seconds: null })
  clock.tick(5000)
  assert.equal(clock.count(), 0)
  clock.timer.update(open)
  clock.tick(500)
  clock.timer.update({ ...open, seconds: 3 })
  clock.tick(2999)
  assert.equal(clock.count(), 0)
  clock.tick(1)
  assert.equal(clock.count(), 1)
  clock.timer.update(open)
  const pending = clock.tasks.at(-1)!.callback
  clock.timer.dispose()
  pending()
  clock.timer.update(open)
  clock.tick(5000)
  assert.equal(clock.count(), 1)
})

it('accepts only bounded integer durations', () => {
  for (const seconds of [0, -1, 1.5, 3601, Number.NaN, Number.POSITIVE_INFINITY])
    assert.equal(validDismissSeconds(seconds), false)
  for (const seconds of [1, 3, 3600])
    assert.equal(validDismissSeconds(seconds), true)
})
