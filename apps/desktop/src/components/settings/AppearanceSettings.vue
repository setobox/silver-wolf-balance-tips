<script setup lang="ts">
import type { AppearancePreferences, RuntimeTheme } from '@silverwolf/core'
import { onUnmounted, shallowRef } from 'vue'
import { desktop } from '../../bridge'

const props = defineProps<{
  theme: RuntimeTheme
}>()
const emit = defineEmits<{
  error: [
        message: string,
  ]
  busy: [
        value: boolean,
  ]
}>()
const model = defineModel<AppearancePreferences>({ required: true })
const busy = shallowRef(false)
let audio: HTMLAudioElement | undefined
async function importFile(event: Event, kind: 'image' | 'sound' | 'theme' | 'directory'): Promise<void> {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  input.value = ''
  if (!files.length)
    return
  busy.value = true
  emit('busy', true)
  emit('error', '')
  try {
    const limit = kind === 'image' || kind === 'sound' ? 8 : 32
    if (files.length > 128 || files.reduce((sum, f) => sum + f.size, 0) > limit * 1024 * 1024)
      throw new Error(`导入文件过多或超过 ${limit} MB。`)
    if (kind === 'directory') {
      const entries = await Promise.all(files.map(async file => ({ path: file.webkitRelativePath, bytes: Array.from(new Uint8Array(await file.arrayBuffer())) })))
      model.value = { ...model.value, themeId: await desktop.importTheme(entries, null), character: null, sound: null }
    }
    else {
      const file = files[0]!
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()))
      if (kind === 'theme') {
        model.value = { ...model.value, themeId: await desktop.importTheme(null, bytes), character: null, sound: null }
      }
      else {
        const id = await desktop.importAsset(file.name, bytes, kind)
        model.value = { ...model.value, [kind === 'image' ? 'character' : 'sound']: id }
      }
    }
  }
  catch (error) {
    emit('error', String(error))
  }
  finally {
    busy.value = false
    emit('busy', false)
  }
}
async function play(): Promise<void> {
  if (!props.theme.sound)
    return
  audio?.pause()
  audio = new Audio(props.theme.sound)
  audio.volume = model.value.volume
  try {
    await audio.play()
  }
  catch {
    emit('error', '此音效无法播放，请更换文件。')
  }
}
onUnmounted(() => audio?.pause())
</script>

<template>
  <div class="settings-section">
    <h2 class="section-title">
      让角色成为你的样子
    </h2>
    <p class="ui-help">
      素材会复制到应用中；右侧预览保存前即可查看。
    </p>
    <fieldset class="setting-group" :disabled="busy">
      <legend>角色与主题</legend>
      <p>当前主题：{{ theme.name }}</p>
      <div class="button-row">
        <label class="file-button ui-button">更换人物<input type="file" accept="image/png,image/jpeg,image/webp,image/gif" @change="importFile($event, 'image')"></label>
        <label class="file-button ui-button">导入主题 ZIP<input type="file" accept=".zip" @change="importFile($event, 'theme')"></label>
        <label class="file-button ui-button">导入主题目录<input type="file" webkitdirectory @change="importFile($event, 'directory')"></label>
      </div>
      <button class="ui-button" @click="model = { themeId: 'builtin', character: null, sound: null, volume: 0.5, muted: false }">
        恢复银狼默认外观
      </button>
      <p class="ui-help">
        图片支持 PNG、JPEG、WebP、GIF。主题目录需包含 theme.json。
      </p>
    </fieldset>
    <fieldset class="setting-group" :disabled="busy">
      <legend>点击音效</legend>
      <div class="button-row">
        <label class="file-button ui-button">选择音效<input type="file" accept=".mp3,.wav,.ogg" @change="importFile($event, 'sound')"></label>
        <button class="ui-button" :disabled="!theme.sound" @click="play">
          试听
        </button>
        <button class="ui-button" :disabled="!model.sound" @click="model = { ...model, sound: null }">
          使用主题音效
        </button>
      </div>
      <label class="ui-form-row"><span>静音</span><input v-model="model.muted" type="checkbox"></label>
      <label class="ui-form-row"><span>音量 {{ Math.round(model.volume * 100) }}%</span><input v-model.number="model.volume" type="range" min="0" max="1" step="0.05"></label>
      <p class="ui-help">
        只有点击角色播放音效，拖动和外部推送保持安静。
      </p>
    </fieldset>
    <p v-if="busy" role="status" class="ui-help">
      正在导入素材…
    </p>
  </div>
</template>
