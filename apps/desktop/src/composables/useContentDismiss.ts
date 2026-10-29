import type { Ref } from 'vue'
import { onMounted, onUnmounted, shallowRef, watch } from 'vue'
import { createDismissTimer } from '../quotaDismiss'

export function useContentDismiss(options: {
  expanded: Ref<boolean>
  seconds: () => number | null
  loading: Ref<boolean>
  settingsOpen: Ref<boolean>
  bubble: Ref<HTMLElement | null>
  actions: Ref<HTMLElement | null>
  paused: () => boolean
  dismiss: () => void
}) {
  const hovered = shallowRef(false)
  const keyboardFocused = shallowRef(false)
  let keyboard = false
  const timer = createDismissTimer(options.dismiss)

  function syncFocus(): void {
    const active = document.activeElement
    keyboardFocused.value = keyboard && !!active
      && !![options.bubble.value, options.actions.value].some(el => el?.contains(active))
  }
  function keydown(event: KeyboardEvent): void {
    if (event.metaKey || event.ctrlKey || event.altKey)
      return
    keyboard = true
    syncFocus()
  }
  function pointerdown(): void {
    keyboard = false
    syncFocus()
  }
  function blur(): void {
    hovered.value = false
    keyboardFocused.value = false
  }
  function syncHover(): void {
    hovered.value = options.expanded.value
      && !![options.bubble.value, options.actions.value].some(el => el?.matches(':hover'))
  }

  watch([options.expanded, options.settingsOpen], () => {
    syncHover()
    syncFocus()
  }, { flush: 'post' })
  const state = () => ({
    expanded: options.expanded.value,
    seconds: options.seconds(),
    loading: options.loading.value || options.paused(),
    settingsOpen: options.settingsOpen.value,
    hovered: hovered.value,
    keyboardFocused: keyboardFocused.value,
  })
  function restart(): void {
    timer.update(state())
  }
  watch(state, value => timer.update(value), { immediate: true, flush: 'sync' })

  onMounted(() => {
    document.addEventListener('keydown', keydown, true)
    document.addEventListener('pointerdown', pointerdown, true)
    document.addEventListener('focusin', syncFocus)
    document.addEventListener('focusout', syncFocus)
    window.addEventListener('blur', blur)
  })
  onUnmounted(() => {
    timer.dispose()
    document.removeEventListener('keydown', keydown, true)
    document.removeEventListener('pointerdown', pointerdown, true)
    document.removeEventListener('focusin', syncFocus)
    document.removeEventListener('focusout', syncFocus)
    window.removeEventListener('blur', blur)
  })
  return { syncHover, restart }
}
