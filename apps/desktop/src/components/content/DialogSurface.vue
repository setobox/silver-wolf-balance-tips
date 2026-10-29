<script setup lang="ts">
import type { ContentDocument, RuntimeTheme } from '@silverwolf/core'
import { computed, shallowRef, watch } from 'vue'
import ContentRenderer from './ContentRenderer.vue'
import TemplateDialog from './TemplateDialog.vue'

const props = defineProps<{
  theme: RuntimeTheme
  content: ContentDocument | null
  expanded: boolean
  loading: boolean
  error: string
  now: number
}>()
const emit = defineEmits<{
  ready: [
        value: boolean,
  ]
  mediaError: [
  ]
  refresh: [
  ]
  close: [
  ]
  interaction: [
        active: boolean,
  ]
}>()
const failed = shallowRef(false)
const imageReady = shallowRef(true)
const templateReady = shallowRef(false)
const custom = computed(() => !!props.theme.dialog && !failed.value)
watch(() => props.theme.dialog, () => {
  failed.value = false
  templateReady.value = false
})
watch(() => props.content?.image, (url, _, cleanup) => {
  imageReady.value = !url
  if (!url)
    return
  const image = new Image()
  let disposed = false
  const timeout = setTimeout(() => {
    if (!disposed) {
      disposed = true
      emit('mediaError')
    }
  }, 10000)
  image.src = url
  void image.decode().then(() => {
    if (!disposed) {
      clearTimeout(timeout)
      imageReady.value = true
    }
  }).catch(() => {
    if (!disposed) {
      clearTimeout(timeout)
      emit('mediaError')
    }
  })
  cleanup(() => {
    disposed = true
    clearTimeout(timeout)
    image.src = ''
  })
}, { immediate: true })
watch(() => imageReady.value && (!custom.value || templateReady.value), value => emit('ready', value), { immediate: true, flush: 'sync' })
</script>

<template>
  <TemplateDialog v-if="custom && theme.dialog" :key="theme.dialog" :url="theme.dialog" :theme="theme" :content="content" :expanded="expanded" :loading="loading" :error="error" @ready="templateReady = $event" @failed="failed = true" @refresh="emit('refresh')" @close="emit('close')" @interaction="emit('interaction', $event)" />
  <div v-else class="thought-shell">
    <svg class="thought-outline" viewBox="0 0 300 230" preserveAspectRatio="none" aria-hidden="true">
      <path d="M48 32C58 10 89 6 110 19 132 5 169 7 187 24 215 12 246 25 254 46 280 48 298 71 291 95 304 118 293 146 275 155 275 182 248 202 223 198 205 220 173 218 156 207 132 226 100 217 90 202 61 211 37 193 33 173 9 165 1 143 12 121 1 97 9 71 31 64 27 47 33 38 48 32Z" />
    </svg>
    <div class="thought-content">
      <h1 class="thought-title">
        {{ content?.title || (content?.usage ? '剩余额度' : '此刻一句') }}
      </h1>
      <div class="thought-scroll">
        <ContentRenderer :content="content" :loading="loading" :error="error" :now="now" />
      </div>
      <p class="caption" role="status">
        {{ failed ? '主题加载失败，已使用内置对话框' : loading ? '更新中…' : content?.usage ? `${new Date(content.observedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })} 更新` : content?.source || 'Silverwolf' }}
      </p>
    </div>
    <span class="thought-dot large" aria-hidden="true" /><span class="thought-dot small" aria-hidden="true" />
  </div>
</template>

<style scoped>
.thought-shell { position: relative; width: 100%; height: 100%; }
.thought-outline { position: absolute; width: 100%; height: calc(100% - 30px); overflow: visible; pointer-events: none; }
.thought-outline path { fill: white; stroke: #383047; stroke-width: 5.5; stroke-linejoin: round; }
.thought-content { position: absolute; top: 34px; right: 36px; bottom: 49px; left: 36px; display: flex; flex-direction: column; }
.thought-title { margin: 0 0 12px; color: #75618f; font-size: 16px; font-weight: 700; text-align: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex-shrink: 0; }
.thought-scroll { min-height: 0; flex: 1; overflow-y: auto; scrollbar-width: thin; scrollbar-color: #ccc2db transparent; padding: 0 2px; }
.caption { margin: 8px 0 0; color: var(--muted); font-size: 10px; line-height: 16px; text-align: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.thought-dot { position: absolute; display: block; border: 4px solid #383047; border-radius: 50%; background: white; pointer-events: none; }
.thought-dot.large { left: 40%; bottom: 17px; width: 23px; height: 17px; transform: rotate(-15deg); }
.thought-dot.small { left: 50%; bottom: 2px; width: 13px; height: 10px; border-width: 3px; }
</style>
