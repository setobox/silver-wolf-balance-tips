<script setup lang="ts">
import type { ContentDocument, RuntimeTheme } from '@silverwolf/core'
import { computed, onMounted, onUnmounted, shallowRef, useTemplateRef, watch } from 'vue'
import { parseThemeMessage } from '../../themeBridge'

const props = defineProps<{
  url: string
  theme: RuntimeTheme
  content: ContentDocument | null
  expanded: boolean
  loading: boolean
  error: string
}>()
const emit = defineEmits<{
  ready: [
        value: boolean,
  ]
  failed: [
  ]
  refresh: [
  ]
  close: [
  ]
  interaction: [
        active: boolean,
  ]
}>()
const frame = useTemplateRef<HTMLIFrameElement>('frame')
const session = crypto.randomUUID()
const src = computed(() => `${props.url}#session=${session}`)
const connected = shallowRef(false)
let timeout: ReturnType<typeof setTimeout> | undefined
let failed = false
function fail(): void {
  if (failed)
    return
  failed = true
  clearTimeout(timeout)
  emit('interaction', false)
  emit('failed')
}
function update(): void {
  if (!connected.value || failed)
    return
  emit('ready', false)
  clearTimeout(timeout)
  timeout = setTimeout(fail, 5000)
  frame.value?.contentWindow?.postMessage({ channel: 'silverwolf-theme-v1', session, type: 'content', content: props.content, state: { expanded: props.expanded, loading: props.loading, error: props.error }, theme: { id: props.theme.id, name: props.theme.name, layout: props.theme.layout } }, '*')
}
function receive(event: MessageEvent): void {
  if (event.source !== frame.value?.contentWindow || event.origin !== 'null' || failed)
    return
  const data = parseThemeMessage(event.data, session)
  if (!data)
    return
  if (data.type === 'ready') {
    connected.value = true
    update()
  }
  else if (data.type === 'rendered' && data.contentId === (props.content?.id ?? '')) {
    clearTimeout(timeout)
    emit('ready', true)
  }
  else if (data.type === 'interaction') {
    emit('interaction', data.active)
  }
  else if (data.type === 'error') {
    fail()
  }
  else if (data.type === 'refresh') {
    emit('refresh')
  }
  else if (data.type === 'close') {
    emit('close')
  }
}
watch(() => [props.content, props.expanded, props.loading, props.error], update)
onMounted(() => {
  window.addEventListener('message', receive)
  timeout = setTimeout(fail, 5000)
})
onUnmounted(() => {
  clearTimeout(timeout)
  window.removeEventListener('message', receive)
  emit('interaction', false)
})
</script>

<template>
  <iframe ref="frame" class="template-frame" :src="src" sandbox="allow-scripts" referrerpolicy="no-referrer" title="主题对话框" @error="fail" />
</template>

<style scoped>
.template-frame { width: 100%; height: 100%; border: 0; background: transparent; }
</style>
