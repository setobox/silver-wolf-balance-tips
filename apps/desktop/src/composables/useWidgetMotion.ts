import type { Ref } from 'vue'
import { onUnmounted } from 'vue'
import { bounce, squash } from '../widgetGeometry'

export function useWidgetMotion(target: Readonly<Ref<HTMLElement | null>>) {
  let animation: Animation | undefined
  let clearPress: (() => void) | undefined

  function animate(keyframes: Keyframe[], duration: number, fill: FillMode = 'none'): void {
    animation?.cancel()
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches)
      return
    animation = target.value?.animate(keyframes, { duration, fill, easing: 'ease-out' })
  }

  function rebound(): void {
    const offsets = [0, 0.36, 0.62, 0.82, 1]
    animate(bounce.map((s, i) => ({ transform: `scale(${s.x}, ${s.y})`, offset: offsets[i] })), 520)
  }

  function press(event: PointerEvent): void {
    if (event.button !== 0)
      return
    clearPress?.()
    const button = event.currentTarget as HTMLElement
    // The shared transform moves the button too. Keep release/click targeted
    // at the character even when it squashes away from the pointer.
    button.setPointerCapture(event.pointerId)
    animate([{ transform: 'scale(1)' }, { transform: `scale(${squash.x}, ${squash.y})` }], 110, 'forwards')
    function clear(): void {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', release)
      window.removeEventListener('pointercancel', cancel)
      window.removeEventListener('blur', cancel)
      clearPress = undefined
    }
    function cancel(): void {
      clear()
      animation?.cancel()
      if (button.hasPointerCapture(event.pointerId))
        button.releasePointerCapture(event.pointerId)
    }
    function release(next: PointerEvent): void {
      if (next.pointerId !== event.pointerId)
        return
      clear()
      rebound()
    }
    function move(next: PointerEvent): void {
      // Match native drag threshold; its move loop may consume pointerup.
      if (next.pointerId === event.pointerId && Math.hypot(next.screenX - event.screenX, next.screenY - event.screenY) >= 5)
        cancel()
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', release)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('blur', cancel)
    clearPress = cancel
  }

  onUnmounted(() => {
    clearPress?.()
    animation?.cancel()
  })
  return { press, rebound }
}
