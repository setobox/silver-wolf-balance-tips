<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, useTemplateRef } from 'vue'

const props = defineProps<{ restoreFocus: HTMLElement | null }>()
const emit = defineEmits<{ close: [] }>()
const panel = useTemplateRef<HTMLElement>('panel')
const closeButton = useTemplateRef<HTMLButtonElement>('close')
let returnFocus: HTMLElement | null = null

function trapFocus(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    emit('close')
    return
  }
  if (event.key !== 'Tab')
    return
  const controls = Array.from(panel.value?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), [tabindex="0"]') ?? [])
  if (!controls.length)
    return
  const first = controls[0]!
  const last = controls.at(-1)!
  if (!panel.value?.contains(document.activeElement)) {
    event.preventDefault()
    ;(event.shiftKey ? last : first).focus()
  }
  else if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  }
  else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}
onMounted(() => {
  returnFocus = props.restoreFocus
  closeButton.value?.focus({ preventScroll: true })
  document.addEventListener('keydown', trapFocus, true)
})
onUnmounted(() => {
  document.removeEventListener('keydown', trapFocus, true)
  void nextTick(() => {
    if (returnFocus?.isConnected)
      returnFocus.focus({ preventScroll: true })
  })
})
</script>

<template>
  <section ref="panel" class="settings-popover" role="dialog" aria-modal="true" aria-labelledby="settings-title">
    <header class="popover-header">
      <h2 id="settings-title">
        挂件设置
      </h2>
      <button ref="close" class="ui-icon-button" aria-label="关闭设置" title="关闭设置" @click="$emit('close')">
        ×
      </button>
    </header>
    <div class="popover-content">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.settings-popover { width: 304px; max-height: 360px; padding: 12px; display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: var(--radius-panel); background: var(--surface); color: var(--ink); box-shadow: 0 3px 12px #38304724; }
.popover-header { display: flex; align-items: center; justify-content: space-between; flex-shrink: 0; margin-bottom: 12px; }
.popover-header h2 { margin: 0; font-size: var(--font-title); font-weight: 650; }
.popover-content { min-height: 0; overflow: auto; scrollbar-width: thin; padding: 3px; margin: -3px; }
</style>
