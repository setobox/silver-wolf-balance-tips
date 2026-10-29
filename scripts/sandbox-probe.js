// Append to an imported test theme's script in an isolated profile.
// Results are displayed in the frame. No credentials or settings are retained.
void (async () => {
  const results = []
  try {
    void parent.document.body
    results.push('FAIL: parent accessible')
  }
  catch {
    results.push('parent blocked')
  }
  try {
    void localStorage.length
    results.push('FAIL: storage accessible')
  }
  catch {
    results.push('storage blocked')
  }
  try {
    await fetch('http://127.0.0.1:38429/v1/status', { signal: AbortSignal.timeout(2000) })
    results.push('FAIL: network accessible')
  }
  catch (error) {
    results.push(error.name === 'TimeoutError' ? 'FAIL: network timed out' : 'network blocked')
  }
  if (window.__TAURI_INTERNALS__?.invoke) {
    try {
      await Promise.race([
        window.__TAURI_INTERNALS__.invoke('get_preferences'),
        new Promise((_, reject) => setTimeout(() => reject(new Error('no IPC response')), 2000)),
      ])
      results.push('FAIL: native accessible')
    }
    catch (error) {
      results.push(error.message === 'no IPC response' ? 'native: no response' : 'native blocked')
    }
  }
  else {
    results.push('native unavailable')
  }
  const output = document.createElement('p')
  output.textContent = results.join(' / ')
  output.style.cssText = 'position:fixed;inset:0;background:#fff;color:#111;font:16px sans-serif;z-index:1000;overflow:auto;padding:14px'
  document.body.append(output)
})()
