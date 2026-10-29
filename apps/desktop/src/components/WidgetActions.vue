<script setup lang="ts">
import { onUnmounted, shallowRef } from 'vue'

defineProps<{ expanded: boolean, settingsOpen: boolean, loading: boolean }>()
const emit = defineEmits<{ toggle: [], refresh: [], settings: [] }>()
const spinning = shallowRef(false)
let spinTimer: ReturnType<typeof setTimeout> | undefined

function refresh(): void {
  clearTimeout(spinTimer)
  spinning.value = true
  spinTimer = setTimeout(() => {
    spinning.value = false
  }, 600)
  emit('refresh')
}

onUnmounted(() => clearTimeout(spinTimer))
</script>

<template>
  <div class="actions" role="group" aria-label="角色操作">
    <button class="ui-icon-button refresh" :disabled="loading" :aria-busy="loading" aria-label="刷新内容" title="刷新内容" @click="refresh">
      <span class="i-lucide-refresh-cw ui-button-icon" :class="{ 'is-spinning': loading || spinning }" aria-hidden="true" />
    </button>
    <button class="ui-icon-button" :aria-pressed="expanded && settingsOpen" aria-label="挂件设置" title="挂件设置" @click="$emit('settings')">
      <span class="i-lucide-settings-2 ui-button-icon" aria-hidden="true" />
    </button>
    <button v-if="expanded" class="ui-icon-button" aria-label="关闭当前内容" title="关闭当前内容" @click="$emit('toggle')">
      <span class="i-lucide-x ui-button-icon" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
.actions { display: flex; flex-direction: column; gap: 4px; }
.ui-icon-button { margin: 0 auto; transition: color .15s, background .15s; }

.is-spinning { animation: refresh-spin .6s linear infinite; }
@keyframes refresh-spin { to { transform: rotate(360deg); } }
</style>
