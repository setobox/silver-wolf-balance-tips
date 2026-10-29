import type { QueueSnapshot } from '@silverwolf/core'
import type { Ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, shallowRef, watch } from 'vue'
import { desktop, native } from '../bridge'
import { createPushPresentation } from '../pushPresentation'

export function usePushQueue(expanded: Ref<boolean>, eligible: Ref<boolean>) {
  const queue = shallowRef<QueueSnapshot>({ revision: -1, messages: [] })
  const active = computed(() => queue.value.messages[0] ?? null)
  const presentation = createPushPresentation()
  let disposed = false
  let unlisten: (() => void) | undefined
  function apply(value: QueueSnapshot): void {
    if (!disposed && value.revision >= queue.value.revision)
      queue.value = value
  }
  watch([active, eligible], ([message, canShow]) => {
    expanded.value = presentation.update(!!message, canShow, expanded.value)
  }, { flush: 'sync' })
  onMounted(async () => {
    if (native) {
      unlisten = await listen<QueueSnapshot>('queue-changed', event => apply(event.payload))
      if (disposed) {
        unlisten()
        return
      }
    }
    apply(await desktop.queue())
  })
  onUnmounted(() => {
    disposed = true
    unlisten?.()
  })
  return { active, queue, finish: async (error: string | null = null) => {
    if (active.value) {
      await desktop.finish(active.value.id, error)
      apply(await desktop.queue())
    }
  } }
}
