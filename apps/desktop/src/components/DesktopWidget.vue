<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Preferences } from '../types'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, nextTick, onMounted, onUnmounted, shallowRef, useTemplateRef } from 'vue'
import { defaultPreferences, desktop, native } from '../bridge'
import { useUsage } from '../composables/useUsage'
import QuotaBubble from './QuotaBubble.vue'
import WidgetSettings from './WidgetSettings.vue'
import WolfCharacter from './WolfCharacter.vue'

const { snapshot, error, loading, now, refresh, reconnect } = useUsage()
const expanded = shallowRef(false)
const preferences = shallowRef<Preferences>({ ...defaultPreferences })
const settingsOpen = shallowRef(false)
const uiError = shallowRef('')
const saving = shallowRef(false)
const bubble = useTemplateRef<HTMLElement>('bubble')
const character = useTemplateRef<HTMLElement>('character')
const transform = computed(() => ({ transform: `scale(${preferences.value.scale})` }))
let observer: ResizeObserver | undefined
let unlisten: UnlistenFn | undefined
let resizeUnlisten: UnlistenFn | undefined
let disposed = false
let suppressClick = false
let pointerCleanup: (() => void) | undefined
let frame = 0

async function updateRegions(): Promise<void> {
  await nextTick()
  const elements = [character.value, expanded.value ? bubble.value : null]
  try {
    await desktop.regions(elements.filter((element): element is HTMLElement => !!element).map((element) => {
      const rect = element.getBoundingClientRect()
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height + 8 * preferences.value.scale }
    }))
  }
  catch { uiError.value = '窗口交互区域更新失败，请从托盘退出后重开。' }
}

function scheduleRegions(): void {
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => {
    void updateRegions()
  })
}

function toggle(): void {
  if (suppressClick) {
    suppressClick = false
    return
  }
  expanded.value = !expanded.value
  scheduleRegions()
}

async function save(patch: Partial<Preferences>): Promise<void> {
  if (saving.value)
    return
  saving.value = true
  uiError.value = ''
  try {
    preferences.value = await desktop.save({ ...preferences.value, ...patch })
    scheduleRegions()
    if ('codexPath' in patch)
      reconnect()
  }
  catch { uiError.value = '设置保存失败，请检查配置目录或 CLI 路径。' }
  finally { saving.value = false }
}

function scaleBy(delta: number): void {
  void save({ scale: Math.min(1.6, Math.max(0.6, Math.round((preferences.value.scale + delta) * 10) / 10)) })
}

function pointerDown(event: PointerEvent): void {
  if (event.button !== 0)
    return
  suppressClick = false
  pointerCleanup?.()
  const start = { x: event.screenX, y: event.screenY }
  function clear(): void {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', clear)
    window.removeEventListener('pointercancel', clear)
  }
  async function move(next: PointerEvent): Promise<void> {
    if (Math.hypot(next.screenX - start.x, next.screenY - start.y) < 5)
      return
    clear()
    suppressClick = true
    if (!native)
      return
    try {
      await desktop.beginDrag()
    }
    catch { uiError.value = '暂时无法拖动窗口。' }
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', clear, { once: true })
  window.addEventListener('pointercancel', clear, { once: true })
  pointerCleanup = clear
}

onMounted(async () => {
  try {
    preferences.value = await desktop.preferences()
    observer = new ResizeObserver(scheduleRegions)
    if (bubble.value)
      observer.observe(bubble.value)
    if (character.value)
      observer.observe(character.value)
    if (native) {
      unlisten = await listen<Preferences>('preferences-changed', (event) => {
        preferences.value = event.payload
        scheduleRegions()
      })
      resizeUnlisten = await getCurrentWindow().onResized(scheduleRegions)
    }
    if (disposed) {
      unlisten?.()
      resizeUnlisten?.()
      observer?.disconnect()
      return
    }
    await updateRegions()
    await desktop.ready()
  }
  catch { uiError.value = '初始化失败，请重开挂件。' }
})
onUnmounted(() => {
  disposed = true
  observer?.disconnect()
  unlisten?.()
  resizeUnlisten?.()
  pointerCleanup?.()
  cancelAnimationFrame(frame)
})
</script>

<template>
  <main class="stage" :style="transform" @contextmenu.prevent>
    <div v-show="expanded" ref="bubble" class="thought">
      <svg class="thought-outline" viewBox="0 0 320 244" preserveAspectRatio="none" aria-hidden="true">
        <path d="M314 110C314 48 250 6 160 6S6 48 6 110c0 53 38 90 92 103 2 12 13 20 26 20 12 0 23-5 28-13 91 1 162-43 162-110Z" />
      </svg>
      <div class="thought-content">
        <header class="thought-header">
          <button v-if="settingsOpen" class="icon-button back" aria-label="返回额度" @click="settingsOpen = false">
            ‹
          </button>
          <h1>{{ settingsOpen ? '挂件设置' : '剩余额度' }}</h1>
          <div class="actions">
            <button v-if="!settingsOpen" class="icon-button refresh" :disabled="loading" aria-label="刷新额度" title="刷新额度" @click="refresh">
              ↻
            </button>
            <button v-if="!settingsOpen" class="icon-button" aria-label="挂件设置" title="挂件设置" @click="settingsOpen = true">
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 17h16M8 4v6m8 4v6" /></svg>
            </button>
            <button class="icon-button close" aria-label="收起额度" title="收起额度" @click="toggle">
              ×
            </button>
          </div>
        </header>
        <div class="thought-scroll" :class="{ 'is-settings': settingsOpen }">
          <WidgetSettings v-if="settingsOpen" :preferences="preferences" :saving="saving" @save="save" @scale="scaleBy" />
          <QuotaBubble v-else :snapshot="snapshot" :error="error" :loading="loading" :now="now" />
          <p v-if="uiError" role="alert" class="ui-error">
            {{ uiError }}
          </p>
        </div>
        <p class="caption">
          <template v-if="!native">
            浏览器预览 · 不读取账户
          </template>
          <template v-else-if="settingsOpen">
            拖动人物移动 · 滚轮缩放
          </template>
          <template v-else-if="snapshot">
            {{ new Date(snapshot.observedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) }} 更新
          </template>
          <template v-else>
            尚未更新
          </template>
        </p>
      </div>
      <span class="thought-dot large" aria-hidden="true" />
      <span class="thought-dot small" aria-hidden="true" />
    </div>
    <div ref="character" class="character-position">
      <WolfCharacter :expanded="expanded" @toggle="toggle" @pointerdown="pointerDown" @wheel="scaleBy($event.deltaY < 0 ? 0.1 : -0.1)" />
    </div>
  </main>
</template>

<style scoped>
.stage { position: absolute; right: 0; bottom: 0; width: 336px; height: 490px; transform-origin: bottom right; }
.thought { position: absolute; left: 4px; bottom: 204px; width: 302px; height: 282px; z-index: 1; }
.thought-outline { position: absolute; width: 302px; height: 236px; inset: 0 auto auto 0; overflow: visible; pointer-events: none; }
.thought-outline path { fill: white; stroke: #383047; stroke-width: 5.5; stroke-linejoin: round; }
.thought-content { position: absolute; top: 31px; right: 35px; bottom: 65px; left: 35px; display: flex; flex-direction: column; }
.thought-header { position: relative; display: flex; align-items: center; flex-shrink: 0; gap: 3px; height: 28px; margin-bottom: 8px; }
.thought-header h1 { position: absolute; left: 0; right: 0; margin: 0; color: #75618f; font-size: 16px; font-weight: 700; white-space: nowrap; text-align: center; pointer-events: none; }
.actions { display: flex; align-items: center; margin-left: auto; gap: 1px; }
.icon-button { display: grid; place-items: center; width: 23px; height: 25px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: #918799; font-size: 20px; line-height: 1; }
.icon-button:hover { background: #f0ecf5; color: #574567; }
.icon-button svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; }
.back { width: 17px; font-size: 25px; }
.thought-scroll { min-height: 0; flex: 1; overflow-y: auto; scrollbar-width: thin; scrollbar-color: #ccc2db transparent; margin-bottom: 24px; padding: 0 2px; }
.caption { position: absolute; bottom: 12px; left: 30px; right: 30px; margin: 0; color: #948a9d; font-size: 9px; line-height: 12px; text-align: center; pointer-events: none; }
.thought-dot { position: absolute; display: block; border: 4px solid #383047; border-radius: 50%; background: white; pointer-events: none; }
.thought-dot.large { left: 94px; top: 240px; width: 28px; height: 20px; transform: rotate(-8deg); }
.thought-dot.small { left: 125px; top: 271px; width: 15px; height: 11px; border-width: 3.5px; }
.character-position { position: absolute; right: 0; bottom: 0; }
.ui-error { color: #a36c28; font-size: 10px; line-height: 1.5; }
</style>
