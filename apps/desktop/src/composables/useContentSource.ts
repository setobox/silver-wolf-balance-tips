import type { ContentDocument } from '@silverwolf/core'
import type { Ref } from 'vue'
import type { Preferences } from '../types'
import { emptyContent } from '@silverwolf/core'
import { onUnmounted, shallowRef, watch } from 'vue'
import { assetUrl, desktop } from '../bridge'
import { createContentState } from '../contentState'

export function useContentSource(preferences: Ref<Preferences>, initialized: Ref<boolean>, visible: Ref<boolean>) {
  const content = shallowRef<ContentDocument | null>(null)
  const loading = shallowRef(false)
  const error = shallowRef('')
  const state = createContentState((value) => {
    content.value = value.content
    loading.value = value.loading
    error.value = value.error
  })
  let index = 0
  let lastAttempt = 0
  async function refresh(): Promise<void> {
    if (!initialized.value)
      return
    lastAttempt = Date.now()
    const config = structuredClone(preferences.value.content)
    await state.refresh(async (signal) => {
      if (config.provider === 'local') {
        if (!config.entries.length)
          throw new Error('请在内容设置中添加文字或图片。')
        const entry = config.entries[config.random ? Math.floor(Math.random() * config.entries.length) : index++ % config.entries.length]!
        return { ...emptyContent(crypto.randomUUID()), text: entry.text, image: entry.image ? assetUrl(entry.image) : null }
      }
      const id = crypto.randomUUID()
      const cancel = () => {
        void desktop.cancelSource(id).catch(() => {
        })
      }
      signal.addEventListener('abort', cancel, { once: true })
      try {
        return await desktop.source(config, id)
      }
      finally {
        signal.removeEventListener('abort', cancel)
      }
    }, config.provider === 'codex')
  }
  watch([() => initialized.value, () => JSON.stringify(preferences.value.content), () => preferences.value.codexPath], () => {
    index = 0
    state.reset()
    if (initialized.value)
      void refresh()
  }, { immediate: true })
  const timer = setInterval(() => {
    const seconds = preferences.value.content.provider === 'codex' ? 60 : preferences.value.content.intervalSeconds
    if (initialized.value && visible.value && seconds && !loading.value && Date.now() - lastAttempt >= seconds * 1000)
      void refresh()
  }, 1000)
  onUnmounted(() => {
    clearInterval(timer)
    state.dispose()
  })
  return { content, loading, error, refresh }
}
