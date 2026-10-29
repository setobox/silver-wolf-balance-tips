/* Theme bridge: only display data is exchanged with the parent. */
(() => {
  const session = new URLSearchParams(location.hash.slice(1)).get('session')
  const listeners = new Set()
  const send = (type, extra = {}) => parent.postMessage({ channel: 'silverwolf-theme-v1', session, type, ...extra }, '*')
  window.Silverwolf = Object.freeze({
    onContent(callback) {
      listeners.add(callback)
      return () => listeners.delete(callback)
    },
    refresh() {
      send('refresh')
    },
    close() {
      send('close')
    },
  })
  window.addEventListener('message', (event) => {
    const data = event.data
    if (event.source !== parent || !data || data.channel !== 'silverwolf-theme-v1' || data.session !== session || data.type !== 'content')
      return
    for (const callback of listeners) {
      try {
        callback(data.content, data.state, data.theme)
      }
      catch {
        send('error')
      }
    }
    send('rendered', { contentId: data.content?.id ?? '' })
  })
  let keyboard = false
  document.addEventListener('keydown', () => {
    keyboard = true
    send('interaction', { active: true })
  }, true)
  document.addEventListener('pointerdown', () => {
    keyboard = false
    send('interaction', { active: false })
  }, true)
  window.addEventListener('blur', () => {
    keyboard = false
    send('interaction', { active: false })
  })
  document.addEventListener('focusout', () => {
    if (keyboard)
      setTimeout(() => send('interaction', { active: document.activeElement !== document.body }), 0)
  })
  window.addEventListener('error', () => send('error'))
  window.addEventListener('unhandledrejection', () => send('error'))
  window.addEventListener('DOMContentLoaded', () => send('ready'), { once: true })
})()
