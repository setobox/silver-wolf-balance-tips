// Run only against an isolated desktop test profile. Never prints its token.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import process from 'node:process'
import { setTimeout as delay } from 'node:timers/promises'

const [base, tokenFile, mode = 'drain'] = process.argv.slice(2)
assert.ok(base && tokenFile, 'Usage: node scripts/smoke-http.mjs http://127.0.0.1:PORT TOKEN_FILE [drain|paused]')
assert.equal(new URL(base).hostname, '127.0.0.1')
const authorization = `Bearer ${(await readFile(tokenFile, 'utf8')).trim()}`
const request = (path, options = {}) => fetch(`${base}${path}`, { ...options, headers: { authorization, ...options.headers } })
const status = async id => (await request(`/v1/messages/${id}`)).json()
const post = body => request('/v1/messages', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
async function send(body) {
  const result = await post(body)
  assert.equal(result.status, 202)
  return (await result.json()).id
}
assert.equal((await fetch(`${base}/v1/status`)).status, 401)
assert.equal((await request('/v1/status', { headers: { authorization: 'Bearer incorrect' } })).status, 401)
assert.equal((await request('/v1/status', { headers: { origin: 'https://example.org' } })).status, 401)
assert.equal((await post({ title: 'empty' })).status, 400)
assert.equal((await post({ text: 'invalid duration', durationSeconds: 0 })).status, 400)
assert.equal((await post({ image: { url: 'file:///C:/secret.png' } })).status, 400)
assert.equal((await post({ image: { assetId: '../secret.png' } })).status, 400)
assert.equal((await request('/v1/messages/does-not-exist')).status, 404)
const initial = await (await request('/v1/status')).json()
assert.equal(initial.queueLength, 0, 'Start with an empty isolated queue')

const fixture = createServer((_req, res) => res.end('this is not an image'))
await new Promise(resolve => fixture.listen(0, '127.0.0.1', resolve))
try {
  const failed = await send({ image: { url: `http://127.0.0.1:${fixture.address().port}/bad.png` }, durationSeconds: 1 })
  for (let attempt = 0; attempt < 50 && (await status(failed)).status !== 'failed'; attempt++)
    await delay(100)
  assert.equal((await status(failed)).status, 'failed')
  const upload = new FormData()
  upload.set('file', new Blob([await readFile(new URL('../apps/desktop/public/silverwolf.png', import.meta.url))], { type: 'image/png' }), 'character.png')
  const uploaded = await request('/v1/assets', { method: 'POST', body: upload })
  assert.equal(uploaded.status, 200)
  const { assetId } = await uploaded.json()
  const first = await send({ title: 'HTTP 验收 1', text: '第一条文字', durationSeconds: 1 })
  const second = await send({ title: 'HTTP 验收 2', text: '<script>这是一段普通文字</script>', image: { assetId }, durationSeconds: 1 })
  if (mode === 'paused') {
    await delay(2200)
    assert.equal((await status(first)).status, 'waiting', 'Open settings or hide the widget before this test')
    assert.equal((await status(second)).status, 'waiting')
    for (let i = 2; i < 100; i++)
      await send({ text: `queue ${i}`, durationSeconds: 1 })
    assert.equal((await post({ text: 'overflow' })).status, 429)
    assert.equal((await (await request('/v1/status')).json()).queueLength, 100)
    console.log('PASS: auth, validation, upload, image failure, paused queue, 100-message capacity. Clear queue in settings.')
  }
  else {
    const seen = new Set()
    for (let attempt = 0; attempt < 150; attempt++) {
      const a = await status(first)
      const b = await status(second)
      seen.add(`${a.status}/${b.status}`)
      assert.notEqual(a.status, 'failed')
      assert.notEqual(b.status, 'failed')
      if (b.status === 'showing' || b.status === 'completed')
        assert.equal(a.status, 'completed', 'FIFO order')
      if (b.status === 'completed')
        break
      await delay(100)
    }
    assert.equal((await status(second)).status, 'completed', `Queue did not drain: ${[...seen].join(', ')}`)
    assert.ok([...seen].some(s => s.startsWith('showing/')), 'First message was shown')
    assert.ok([...seen].some(s => s.endsWith('/showing')), 'Image was shown before completion')
    console.log('PASS: auth, validation, upload, image failure, FIFO, showing acknowledgement, automatic completion.')
  }
}
finally {
  fixture.close()
}
