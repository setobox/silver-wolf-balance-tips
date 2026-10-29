import type { ContentDocument } from '@silverwolf/core'
import type { ContentState } from './contentState.ts'
import assert from 'node:assert/strict'
import { it } from 'node:test'
import { emptyContent } from '@silverwolf/core'
import { createContentState } from './contentState.ts'
import { createPushPresentation } from './pushPresentation.ts'
import { parseThemeMessage } from './themeBridge.ts'

it('cancels the old provider and rejects late responses after switching', async () => {
  let current: ContentState = { content: null, loading: false, error: '' }
  const state = createContentState(value => current = value)
  let resolve: (content: ContentDocument) => void = () => {
  }
  let oldSignal: AbortSignal | undefined
  const old = state.refresh(signal => new Promise<ContentDocument>((done) => {
    oldSignal = signal
    resolve = done
  }), false)
  state.reset()
  await state.refresh(async () => ({ ...emptyContent('new'), text: 'new' }), false)
  resolve({ ...emptyContent('old'), text: 'old' })
  await old
  assert.equal(oldSignal?.aborted, true)
  assert.equal(current.content?.text, 'new')
  assert.equal(current.loading, false)
})
it('retains quotes after a failure but clears identity-sensitive quota', async () => {
  let current: ContentState = { content: null, loading: false, error: '' }
  const state = createContentState(value => current = value)
  await state.refresh(async () => emptyContent('retained'), false)
  await state.refresh(async () => {
    throw new Error('offline')
  }, false)
  assert.equal(current.content?.id, 'retained')
  assert.equal(current.error, 'offline')
  await state.refresh(async () => {
    throw new Error('logged out')
  }, true)
  assert.equal(current.content, null)
})
it('waits while hidden, preserves expanded state across a FIFO batch and restores it', () => {
  for (const initial of [false, true]) {
    const state = createPushPresentation()
    assert.equal(state.update(true, false, initial), initial)
    assert.equal(state.update(true, true, initial), true)
    assert.equal(state.update(true, false, true), true)
    assert.equal(state.update(true, true, true), true)
    assert.equal(state.update(false, true, true), initial)
  }
})
it('accepts only the current template session and known narrow messages', () => {
  const message = { channel: 'silverwolf-theme-v1', session: 'current', type: 'interaction', active: true }
  assert.deepEqual(parseThemeMessage(message, 'current'), { type: 'interaction', active: true })
  assert.equal(parseThemeMessage(message, 'old'), null)
  assert.equal(parseThemeMessage({ ...message, type: 'invoke', command: 'get_http_token' }, 'current'), null)
  assert.equal(parseThemeMessage({ ...message, active: 'true' }, 'current'), null)
})
