import type { UsageError, UsageSnapshot } from '@silverwolf/core'
import { shallowRef } from 'vue'

// Request state is independent of the polling lifecycle so refresh races can be tested.
export function createUsageState(read: () => Promise<UsageSnapshot>) {
  const snapshot = shallowRef<UsageSnapshot | null>(null)
  const error = shallowRef<UsageError | null>(null)
  const loading = shallowRef(false)
  let lastAttempt = 0
  let disposed = false
  let generation = 0
  let queued = false

  async function refresh(): Promise<void> {
    if (loading.value || disposed)
      return
    loading.value = true
    const requestGeneration = generation
    lastAttempt = Date.now()
    error.value = null
    // Keep the current snapshot visible until the replacement is ready.
    try {
      const result = await read()
      if (!disposed && requestGeneration === generation)
        snapshot.value = result
    }
    catch (cause) {
      if (!disposed && requestGeneration === generation) {
        // A failed identity check must not leave another account's data visible.
        snapshot.value = null
        error.value = cause && typeof cause === 'object' && 'message' in cause
          ? cause as UsageError
          : { code: 'query_failed', message: '无法读取额度，请稍后重试。' }
      }
    }
    finally {
      if (!disposed) {
        loading.value = false
        if (queued) {
          queued = false
          void refresh()
        }
      }
    }
  }

  function reconnect(): void {
    if (disposed)
      return
    generation++
    snapshot.value = null
    error.value = null
    if (loading.value)
      queued = true
    else
      void refresh()
  }

  return {
    snapshot,
    error,
    loading,
    refresh,
    reconnect,
    lastAttempt: () => lastAttempt,
    dispose() { disposed = true },
  }
}
