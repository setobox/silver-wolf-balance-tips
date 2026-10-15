<script setup lang="ts">
defineProps<{ expanded: boolean, settingsOpen: boolean, loading: boolean }>()
defineEmits<{ toggle: [], refresh: [], settings: [], back: [] }>()
</script>

<template>
  <div class="actions" role="group" aria-label="角色操作">
    <button class="ui-icon-button" :aria-expanded="expanded" aria-label="查看额度" title="查看额度" @click="$emit('back')">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 0 1-8 8H8l-5 3 1.5-5A8 8 0 1 1 20 11Z" /></svg>
    </button>
    <button class="ui-icon-button refresh" :disabled="loading" aria-label="刷新额度" title="刷新额度" @click="$emit('refresh')">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5M19 12a7 7 0 1 0-2 5M20 12l-3-5" /></svg>
    </button>
    <button class="ui-icon-button" :aria-pressed="expanded && settingsOpen" aria-label="挂件设置" title="挂件设置" @click="$emit('settings')">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 17h16M8 4v6m8 4v6" /></svg>
    </button>
    <button v-if="expanded" class="ui-icon-button" aria-label="收起额度" title="收起额度" @click="$emit('toggle')">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>
    </button>
  </div>
</template>

<style scoped>
.actions { display: flex; flex-direction: column; gap: 4px; }
.ui-icon-button { margin: 0 auto; transition: color .15s, background .15s; }

.ui-icon-button svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
</style>
