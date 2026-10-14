<script setup lang="ts">
import { onUnmounted, useTemplateRef } from 'vue'

defineProps<{ expanded: boolean }>()
const emit = defineEmits<{ toggle: [], pointerdown: [event: PointerEvent], wheel: [event: WheelEvent] }>()
const portrait = useTemplateRef<HTMLImageElement>('portrait')
let animation: Animation | undefined
let clearPress: (() => void) | undefined

function animate(keyframes: Keyframe[], duration: number, fill: FillMode = 'none'): void {
  animation?.cancel()
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches)
    return
  animation = portrait.value?.animate(keyframes, { duration, fill, easing: 'ease-out' })
}

function rebound(): void {
  animate([
    { transform: 'scale(1.06, .84)', offset: 0 },
    { transform: 'scale(.97, 1.04)', offset: 0.36 },
    { transform: 'scale(1.025, .97)', offset: 0.62 },
    { transform: 'scale(.995, 1.01)', offset: 0.82 },
    { transform: 'scale(1)', offset: 1 },
  ], 520)
}

function pointerDown(event: PointerEvent): void {
  if (event.button !== 0)
    return
  clearPress?.()
  animate([{ transform: 'scale(1)' }, { transform: 'scale(1.06, .84)' }], 110, 'forwards')
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
  }
  function release(next: PointerEvent): void {
    if (next.pointerId !== event.pointerId)
      return
    clear()
    rebound()
  }
  function move(next: PointerEvent): void {
    // Match the window's drag threshold; native dragging may consume pointerup.
    if (next.pointerId === event.pointerId && Math.hypot(next.screenX - event.screenX, next.screenY - event.screenY) >= 5)
      cancel()
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', release)
  window.addEventListener('pointercancel', cancel)
  window.addEventListener('blur', cancel)
  clearPress = cancel
  emit('pointerdown', event)
}

function click(event: MouseEvent): void {
  if (event.detail === 0)
    rebound()
  emit('toggle')
}

onUnmounted(() => {
  clearPress?.()
  animation?.cancel()
})
</script>

<template>
  <button class="character" :aria-expanded="expanded" aria-label="银狼：点击查看额度，拖动移动，滚轮缩放" title="点击查看额度 · 拖动移动 · 滚轮缩放" @click="click" @pointerdown="pointerDown" @wheel.prevent="$emit('wheel', $event)">
    <img ref="portrait" class="portrait" src="/silverwolf.png" alt="戴着蓝紫色护目镜的银狼" draggable="false">
  </button>
</template>

<style scoped>
.character { display: block; background: none; border: 0; width: 238px; height: 244px; padding: 12px 8px 0; touch-action: none; -webkit-user-drag: none; cursor: grab; }
.character:active { cursor: grabbing; }
.portrait { display: block; width: 100%; height: 100%; object-fit: contain; object-position: bottom right; transform-origin: center bottom; pointer-events: none; }
.character:focus-visible { outline-offset: -3px; border-radius: 16px; }
</style>
