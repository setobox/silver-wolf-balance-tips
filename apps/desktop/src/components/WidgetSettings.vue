<script setup lang="ts">
import type { Preferences } from '../types'
import { shallowRef, watch } from 'vue'
import { validDismissSeconds } from '../quotaDismiss'

const props = defineProps<{ preferences: Preferences, saving: boolean }>()
const emit = defineEmits<{ save: [patch: Partial<Preferences>], scale: [delta: number] }>()
const cliDraft = shallowRef(props.preferences.codexPath)
const custom = shallowRef(![1, 3, null].includes(props.preferences.quotaDismissSeconds))
const secondsDraft = shallowRef(String(props.preferences.quotaDismissSeconds ?? 5))
const secondsError = shallowRef('')
watch(() => props.preferences.codexPath, value => cliDraft.value = value)
watch(() => props.preferences.quotaDismissSeconds, (value) => {
  custom.value = ![1, 3, null].includes(value)
  secondsDraft.value = String(value ?? 5)
  secondsError.value = ''
})

function preset(seconds: number | null): void {
  custom.value = false
  secondsError.value = ''
  emit('save', { quotaDismissSeconds: seconds })
}
function saveSeconds(): void {
  const seconds = Number(secondsDraft.value)
  if (!secondsDraft.value.trim() || !validDismissSeconds(seconds)) {
    secondsError.value = '请输入 1–3600 的整数秒数。'
    return
  }
  secondsError.value = ''
  emit('save', { quotaDismissSeconds: seconds })
}
</script>

<template>
  <section class="settings" aria-label="挂件设置选项">
    <label class="ui-form-row follow">
      <span>跟随 Codex 窗口</span>
      <input type="checkbox" :checked="preferences.follow" :disabled="saving" @change="emit('save', { follow: ($event.target as HTMLInputElement).checked })">
    </label>
    <div class="ui-form-row">
      <span>角色大小</span>
      <div class="scale-controls">
        <button class="ui-button step" aria-label="缩小角色" :disabled="saving || preferences.scale <= 0.6" @click="emit('scale', -0.1)">
          −
        </button>
        <output class="ui-number">{{ Math.round(preferences.scale * 100) }}%</output>
        <button class="ui-button step" aria-label="放大角色" :disabled="saving || preferences.scale >= 1.6" @click="emit('scale', 0.1)">
          +
        </button>
      </div>
    </div>
    <fieldset class="duration" :disabled="saving">
      <legend class="ui-label">
        提示显示时长
      </legend>
      <div class="duration-options" role="group" aria-label="提示显示时长">
        <button v-for="option in [{ value: 1, label: '1 秒' }, { value: 3, label: '3 秒' }, { value: null, label: '常驻' }]" :key="option.label" class="ui-button" :aria-pressed="!custom && preferences.quotaDismissSeconds === option.value" @click="preset(option.value)">
          {{ option.label }}
        </button>
        <button class="ui-button" :aria-pressed="custom" @click="custom = true">
          自定义
        </button>
      </div>
      <form v-if="custom" class="custom-form" novalidate @submit.prevent="saveSeconds">
        <label class="ui-label" for="dismiss-seconds">自动收起</label>
        <input id="dismiss-seconds" :value="secondsDraft" class="ui-input ui-number seconds-input" type="number" min="1" max="3600" step="1" inputmode="numeric" :aria-invalid="!!secondsError" :aria-describedby="secondsError ? 'seconds-error' : undefined" @input="secondsDraft = ($event.target as HTMLInputElement).value">
        <span>秒后</span>
        <button class="ui-button" :disabled="saving">
          保存
        </button>
      </form>
      <p v-if="secondsError" id="seconds-error" class="ui-error" role="alert">
        {{ secondsError }}
      </p>
      <p class="ui-help">
        鼠标停留、键盘操作或打开设置时暂停计时。
      </p>
    </fieldset>
    <form class="path-form" @submit.prevent="emit('save', { codexPath: cliDraft.trim() })">
      <label class="ui-label" for="cli-path">Codex CLI 路径</label>
      <div class="path-row">
        <input id="cli-path" v-model="cliDraft" class="ui-input ui-number path-input" :title="cliDraft" :disabled="saving" placeholder="留空自动查找" spellcheck="false">
        <button class="ui-button" :disabled="saving">
          保存
        </button>
      </div>
    </form>
  </section>
</template>

<style scoped>
.settings { display: grid; gap: 12px; color: var(--ink); font-size: var(--font-body); }
.follow { cursor: pointer; }
.follow input { margin: 0; width: 14px; height: 14px; accent-color: var(--accent); }
.scale-controls { display: flex; align-items: center; gap: 8px; }
.scale-controls output { min-width: 36px; text-align: center; }
.step { width: 28px; padding: 0; font-size: 16px; }
.duration { min-width: 0; padding: 0; margin: 0; border: 0; }
.duration legend { padding: 0; margin-bottom: 8px; }
.duration-options { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; }
.custom-form { display: flex; align-items: center; gap: 4px; margin-top: 8px; }
.seconds-input { flex: 1; width: 0; }
.path-form { display: grid; gap: 8px; }
.path-row { display: flex; gap: 4px; }
.path-input { flex: 1; width: 0; text-overflow: ellipsis; }
</style>
