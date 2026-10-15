import { onMounted, onUnmounted, shallowRef } from 'vue'
import { desktop } from '../bridge'
import { createUsageState } from '../usageState'

export function useUsage() {
  const state = createUsageState(desktop.read)
  const now = shallowRef(Date.now())
  let timer: ReturnType<typeof setInterval> | undefined

  onMounted(() => {
    void state.refresh()
    timer = setInterval(async () => {
      now.value = Date.now()
      if (Date.now() - state.lastAttempt() < 60_000)
        return
      try {
        if ((await desktop.status()).visible)
          await state.refresh()
      }
      catch { /* The native process may be exiting. */ }
    }, 1000)
  })
  onUnmounted(() => {
    state.dispose()
    clearInterval(timer)
  })
  return { snapshot: state.snapshot, error: state.error, loading: state.loading, now, refresh: state.refresh, reconnect: state.reconnect }
}
