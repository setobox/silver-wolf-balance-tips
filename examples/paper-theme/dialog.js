/* The host injects Silverwolf before this script; do not load a remote SDK. */
const text = document.querySelector('#text')
const picture = document.querySelector('#image')
const usage = document.querySelector('#usage')
window.Silverwolf.onContent((content, state) => {
  document.querySelector('#title').textContent = content?.title || '来自桌面的一句话'
  text.textContent = content?.text || ''
  picture.hidden = !content?.image
  if (content?.image)
    picture.src = content.image
  else
    picture.removeAttribute('src')
  document.querySelector('#credit').textContent = [content?.author, content?.source].filter(Boolean).join(' · ')
  document.querySelector('#status').textContent = state.loading ? '更新中…' : state.error || ''
  usage.replaceChildren()
  for (const meter of content?.usage?.meters ?? []) {
    const row = document.createElement('p')
    row.textContent = meter.kind === 'quota' ? `${meter.label} · 剩余 ${Math.round(meter.remainingPercent)}%` : `${meter.label} · ${meter.remaining} ${meter.currency}`
    usage.append(row)
  }
})
document.querySelector('#refresh').addEventListener('click', () => window.Silverwolf.refresh())
document.querySelector('#close').addEventListener('click', () => window.Silverwolf.close())
