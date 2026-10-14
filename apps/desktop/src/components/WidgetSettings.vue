<script setup lang="ts">
import type { Preferences } from '../types'
import { shallowRef, watch } from 'vue'

const props = defineProps<{ preferences: Preferences, saving: boolean }>()
defineEmits<{ save: [patch: Partial<Preferences>], scale: [delta: number] }>()
const cliDraft = shallowRef(props.preferences.codexPath)
watch(() => props.preferences.codexPath, value => cliDraft.value = value)
</script>

<template>
  <section class="settings" aria-label="挂件设置">
    <label class="follow">
      <input type="checkbox" :checked="preferences.follow" :disabled="saving" @change="$emit('save', { follow: ($event.target as HTMLInputElement).checked })">
      跟随 Codex 窗口
    </label>
    <div class="scale-row">
      <span>大小</span>
      <div class="scale-controls">
        <button class="step" aria-label="缩小角色" :disabled="saving || preferences.scale <= 0.6" @click="$emit('scale', -0.1)">
          −
        </button>
        <output>{{ Math.round(preferences.scale * 100) }}%</output>
        <button class="step" aria-label="放大角色" :disabled="saving || preferences.scale >= 1.6" @click="$emit('scale', 0.1)">
          +
        </button>
      </div>
    </div>
    <label class="path-label" for="cli-path">Codex CLI 路径</label>
    <form class="path-row" @submit.prevent="$emit('save', { codexPath: cliDraft.trim() })">
      <input id="cli-path" v-model="cliDraft" class="path-input" placeholder="留空自动查找" spellcheck="false">
      <button class="save" :disabled="saving">
        保存
      </button>
    </form>
  </section>
</template>

<style scoped>
.settings { color: var(--muted); font-size: 11px; }
.follow { display: flex; align-items: center; gap: 6px; cursor: pointer; }
.follow input { margin: 0; accent-color: var(--accent); }
.scale-row { display: flex; align-items: center; justify-content: space-between; margin: 10px 0; }
.scale-controls { display: flex; align-items: center; gap: 7px; }
.scale-controls output { min-width: 30px; text-align: center; font-variant-numeric: tabular-nums; }
.step { width: 24px; height: 24px; border: 1px solid var(--line); border-radius: 50%; background: #f4f1f8; font-size: 16px; }
.step:hover, .save:hover { background: #e8e2f2; }
.path-label { display: block; margin-bottom: 5px; font-size: 10px; }
.path-row { display: flex; gap: 5px; }
.path-input { width: 0; min-width: 0; flex: 1; padding: 6px; border: 1px solid var(--line); border-radius: 5px; background: white; color: var(--ink); font-size: 10px; }
.save { padding: 4px 8px; border: 1px solid var(--line); border-radius: 5px; background: #f4f1f8; font-size: 10px; }
</style>
