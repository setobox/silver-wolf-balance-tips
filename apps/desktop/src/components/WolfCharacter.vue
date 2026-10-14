<script setup lang="ts">
defineProps<{ expanded: boolean }>()
defineEmits<{ toggle: [event: MouseEvent], pointerdown: [event: PointerEvent], wheel: [event: WheelEvent] }>()
</script>

<template>
  <button class="character" :aria-expanded="expanded" aria-label="银狼：点击查看额度，拖动移动，滚轮缩放" title="点击查看额度 · 拖动移动 · 滚轮缩放" @click="$emit('toggle', $event)" @pointerdown="$emit('pointerdown', $event)" @wheel.prevent="$emit('wheel', $event)">
    <img class="portrait" src="/silverwolf.png" alt="戴着蓝紫色护目镜的银狼" draggable="false">
  </button>
</template>

<style scoped>
.character { display: block; background: none; border: 0; width: 238px; height: 244px; padding: 12px 12px 0 8px; touch-action: none; -webkit-user-drag: none; cursor: grab; }
.character:active { cursor: grabbing; }
.portrait { display: block; width: 100%; height: 100%; object-fit: contain; object-position: bottom right; pointer-events: none; }
.character:focus-visible { outline-offset: -3px; border-radius: 16px; }
</style>
