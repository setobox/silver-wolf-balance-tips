<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Preferences } from '../types'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, nextTick, onMounted, onUnmounted, shallowRef, useTemplateRef, watch } from 'vue'
import { defaultPreferences, desktop, native } from '../bridge'
import { useQuotaDismiss } from '../composables/useQuotaDismiss'
import { useUsage } from '../composables/useUsage'
import { useWidgetMotion } from '../composables/useWidgetMotion'
import { motionRegion, stageSize, staticRegion } from '../widgetGeometry'
import QuotaBubble from './QuotaBubble.vue'
import SettingsPopover from './SettingsPopover.vue'
import WidgetActions from './WidgetActions.vue'
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
const stage = useTemplateRef<HTMLElement>('stage')
const motion = useTemplateRef<HTMLElement>('motion')
const actions = useTemplateRef<HTMLElement>('actions')
const settingsRegion = useTemplateRef<HTMLElement>('settingsRegion')
const restoreFocus = shallowRef<HTMLElement | null>(null)
const toolbarLeft = shallowRef(false)
const { press, rebound } = useWidgetMotion(motion)
const transform = computed(() => ({ transform: `scale(${preferences.value.scale})` }))
let observer: ResizeObserver | undefined
let unlisten: UnlistenFn | undefined
let resizeUnlisten: UnlistenFn | undefined
let sideUnlisten: UnlistenFn | undefined
let disposed = false
let suppressClick = false
let pointerCleanup: (() => void) | undefined
let frame = 0

const { syncHover, restart } = useQuotaDismiss({
  expanded,
  seconds: () => preferences.value.quotaDismissSeconds,
  loading,
  settingsOpen,
  bubble,
  actions,
  dismiss: () => { expanded.value = false },
})

watch([expanded, settingsOpen], scheduleRegions, { flush: 'post' })
watch(settingsRegion, (element, previous) => {
  if (previous)
    observer?.unobserve(previous)
  if (element)
    observer?.observe(element)
  scheduleRegions()
}, { flush: 'post' })

async function updateRegions(): Promise<void> {
  await nextTick()
  if (!stage.value)
    return
  const origin = stage.value.getBoundingClientRect()
  const elements = [character.value, actions.value, expanded.value ? bubble.value : null]
  try {
    const regions = elements.filter((element): element is HTMLElement => !!element).map((element) => {
      return motionRegion({ x: element.offsetLeft, y: element.offsetTop, width: element.offsetWidth, height: element.offsetHeight }, preferences.value.scale, origin)
    })
    if (settingsOpen.value && settingsRegion.value) {
      const element = settingsRegion.value
      regions.push(staticRegion({ x: element.offsetLeft, y: element.offsetTop, width: element.offsetWidth, height: element.offsetHeight }, preferences.value.scale, origin))
    }
    await desktop.regions(regions)
  }
  catch { uiError.value = '窗口交互区域更新失败，请从托盘退出后重开。' }
}

function scheduleRegions(): void {
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => {
    void updateRegions()
  })
}

function collapse(): void {
  if (settingsOpen.value)
    return
  expanded.value = false
  scheduleRegions()
}

function showPanel(settings: boolean): void {
  if (settings && !settingsOpen.value)
    restoreFocus.value = document.activeElement instanceof HTMLElement ? document.activeElement : null
  settingsOpen.value = settings
  expanded.value = true
  restart()
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

function characterClick(event: MouseEvent): void {
  if (suppressClick) {
    suppressClick = false
    return
  }
  if (settingsOpen.value)
    return
  if (event.detail === 0)
    rebound()
  showPanel(false)
}

function scaleBy(delta: number): void {
  void save({ scale: Math.min(1.6, Math.max(0.6, Math.round((preferences.value.scale + delta) * 10) / 10)) })
}

function pointerDown(event: PointerEvent): void {
  if (event.button !== 0)
    return
  suppressClick = false
  press(event)
  pointerCleanup?.()
  const start = { x: event.screenX, y: event.screenY }
  function clear(): void {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', clear)
    window.removeEventListener('pointercancel', clear)
    window.removeEventListener('blur', clear)
  }
  async function move(next: PointerEvent): Promise<void> {
    if (next.pointerId !== event.pointerId || Math.hypot(next.screenX - start.x, next.screenY - start.y) < 5)
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
  window.addEventListener('blur', clear, { once: true })
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
    if (actions.value)
      observer.observe(actions.value)
    if (native) {
      unlisten = await listen<Preferences>('preferences-changed', (event) => {
        preferences.value = event.payload
        scheduleRegions()
      })
      resizeUnlisten = await getCurrentWindow().onResized(scheduleRegions)
      sideUnlisten = await listen<boolean>('toolbar-side-changed', (event) => {
        toolbarLeft.value = event.payload
        scheduleRegions()
      })
      toolbarLeft.value = (await desktop.status()).toolbarLeft
    }
    if (disposed) {
      unlisten?.()
      resizeUnlisten?.()
      sideUnlisten?.()
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
  sideUnlisten?.()
  pointerCleanup?.()
  cancelAnimationFrame(frame)
})
</script>

<template>
  <main ref="stage" class="stage" :style="{ ...transform, width: `${stageSize.width}px`, height: `${stageSize.height}px` }" @contextmenu.prevent>
    <div ref="motion" class="widget-motion" :inert="settingsOpen">
      <div v-show="expanded" ref="bubble" class="thought" @pointerenter="syncHover" @pointerleave="syncHover">
        <svg class="thought-outline" viewBox="0 0 300 230" preserveAspectRatio="none" aria-hidden="true">
          <path d="M48 32C58 10 89 6 110 19 132 5 169 7 187 24 215 12 246 25 254 46 280 48 298 71 291 95 304 118 293 146 275 155 275 182 248 202 223 198 205 220 173 218 156 207 132 226 100 217 90 202 61 211 37 193 33 173 9 165 1 143 12 121 1 97 9 71 31 64 27 47 33 38 48 32Z" />
        </svg>
        <div class="thought-content">
          <header class="thought-header">
            <h1>剩余额度</h1>
          </header>
          <div class="thought-scroll">
            <QuotaBubble :snapshot="snapshot" :error="error" :loading="loading" :now="now" />
            <p v-if="uiError" role="alert" class="ui-error">
              {{ uiError }}
            </p>
          </div>
          <p class="caption" role="status">
            <span v-if="snapshot" class="update-time">
              {{ new Date(snapshot.observedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) }} 更新
            </span>
            <span v-else>{{ native ? '尚未更新' : '浏览器预览' }}</span>
            <span v-if="loading" class="loading-status"><span class="loading-spinner" aria-hidden="true" />更新中</span>
          </p>
        </div>
        <span class="thought-dot large" aria-hidden="true" />
        <span class="thought-dot small" aria-hidden="true" />
      </div>
      <div ref="character" class="character-position">
        <WolfCharacter :expanded="expanded" @toggle="characterClick" @pointerdown="pointerDown" @wheel="scaleBy($event.deltaY < 0 ? 0.1 : -0.1)" />
      </div>
      <div ref="actions" class="actions-position" :class="{ 'is-left': toolbarLeft }" @pointerenter="syncHover" @pointerleave="syncHover">
        <WidgetActions :expanded="expanded" :settings-open="settingsOpen" :loading="loading" @toggle="collapse" @refresh="refresh" @settings="showPanel(true)" @back="showPanel(false)" />
      </div>
    </div>
    <div v-if="settingsOpen" ref="settingsRegion" class="settings-position">
      <SettingsPopover :restore-focus="restoreFocus" @close="showPanel(false)">
        <WidgetSettings :preferences="preferences" :saving="saving" @save="save" @scale="scaleBy" />
        <p v-if="uiError" role="alert" class="ui-error">
          {{ uiError }}
        </p>
      </SettingsPopover>
    </div>
  </main>
</template>

<style scoped>
.stage { position: absolute; right: 0; bottom: 0; transform-origin: bottom right; }
.widget-motion { position: absolute; inset: 0; transform-origin: center bottom; }
.thought { position: absolute; left: 12px; top: 18px; width: 284px; height: 254px; z-index: 1; }
.thought-outline { position: absolute; width: 284px; height: 224px; inset: 0 auto auto 0; overflow: visible; pointer-events: none; }
.thought-outline path { fill: white; stroke: #383047; stroke-width: 5.5; stroke-linejoin: round; }
.thought-content { position: absolute; top: 34px; right: 36px; bottom: 49px; left: 36px; display: flex; flex-direction: column; }
.thought-header { position: relative; display: flex; align-items: center; flex-shrink: 0; gap: 3px; height: 28px; margin-bottom: 8px; }
.thought-header h1 { position: absolute; left: 0; right: 0; margin: 0; color: #75618f; font-size: 16px; font-weight: 700; white-space: nowrap; text-align: center; pointer-events: none; }
.thought-scroll { min-height: 0; flex: 1; overflow-y: auto; scrollbar-width: thin; scrollbar-color: #ccc2db transparent; margin-bottom: 24px; padding: 0 2px; }
.caption { position: absolute; bottom: 10px; left: 0; right: 0; height: 16px; display: flex; justify-content: center; align-items: center; gap: 8px; margin: 0; color: var(--muted); font-size: var(--font-help); line-height: 16px; white-space: nowrap; pointer-events: none; }
.update-time { font-variant-numeric: tabular-nums; }
.loading-status { display: inline-flex; align-items: center; gap: 4px; }
.loading-spinner { width: 10px; height: 10px; border: 1.5px solid var(--line); border-top-color: var(--accent); border-radius: 50%; animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.settings-position { position: absolute; left: 24px; top: 24px; width: 304px; z-index: 3; }
.thought-dot { position: absolute; display: block; border: 4px solid #383047; border-radius: 50%; background: white; pointer-events: none; }
.thought-dot.large { left: 112px; top: 220px; width: 23px; height: 17px; transform: rotate(-15deg); }
.thought-dot.small { left: 141px; top: 242px; width: 13px; height: 10px; border-width: 3px; }
.character-position { position: absolute; right: 56px; bottom: 0; }
.actions-position { position: absolute; left: 328px; top: 282px; width: 32px; opacity: 0; transition: opacity .15s; }
.actions-position::before { content: ''; position: absolute; top: 0; bottom: 0; left: -8px; width: 8px; }
.character-position:hover ~ .actions-position,
.character-position:has(:focus-visible) ~ .actions-position,
.actions-position:hover,
.actions-position:has(:focus-visible) { opacity: 1; }
.actions-position.is-left { left: 42px; }
.actions-position.is-left::before { left: auto; right: -8px; }
</style>
