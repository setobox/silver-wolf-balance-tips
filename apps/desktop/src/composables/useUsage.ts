import type { UsageError, UsageSnapshot } from '@silverwolf/core'
import { onMounted, onUnmounted, shallowRef } from 'vue'
import { desktop } from '../bridge'

export function useUsage() {
  const snapshot = shallowRef<UsageSnapshot | null>(null)
  const error = shallowRef<UsageError | null>(null)
  const loading = shallowRef(false)
  const now = shallowRef(Date.now())
  let lastAttempt = 0
  let timer: ReturnType<typeof setInterval> | undefined
  let disposed = false
  let generation = 0
  let queued = false

  async function refresh(): Promise<void> {
    if (loading.value)
      return
    loading.value = true
    const requestGeneration = generation
    lastAttempt = Date.now()
    // No stale account data is retained across a failed identity check.
    snapshot.value = null
    error.value = null
    try {
      const result = await desktop.read()
      if (!disposed && requestGeneration === generation)
        snapshot.value = result
    }
    catch (cause) {
      if (!disposed && requestGeneration === generation) {
        error.value = cause && typeof cause === 'object' && 'message' in cause
          ? cause as UsageError
          : { code: 'query_failed', message: '无法读取额度，请稍后重试。' }
      }
    }
    finally {
      loading.value = false
      if (queued && !disposed) {
        queued = false
        void refresh()
      }
    }
  }

  function reconnect(): void {
    generation++
    snapshot.value = null
    error.value = null
    if (loading.value)
      queued = true
    else
      void refresh()
  }

  onMounted(() => {
    void refresh()
    timer = setInterval(async () => {
      now.value = Date.now()
      if (Date.now() - lastAttempt < 60_000)
        return
      try {
        if ((await desktop.status()).visible)
          await refresh()
      }
      catch { /* The native process may be exiting. */ }
    }, 1000)
  })
  onUnmounted(() => {
    disposed = true
    clearInterval(timer)
  })
  return { snapshot, error, loading, now, refresh, reconnect }
}
