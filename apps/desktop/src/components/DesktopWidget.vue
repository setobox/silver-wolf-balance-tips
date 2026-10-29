<script setup lang="ts">
import type { LayoutRect } from '@silverwolf/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Preferences } from '../types'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, nextTick, onMounted, onUnmounted, shallowRef, useTemplateRef, watch } from 'vue'
import { builtinTheme, defaultPreferences, desktop, native } from '../bridge'
import { useContentDismiss } from '../composables/useContentDismiss'
import { useContentSource } from '../composables/useContentSource'
import { usePushQueue } from '../composables/usePushQueue'
import { useWidgetMotion } from '../composables/useWidgetMotion'
import { motionRegion, staticRegion } from '../widgetGeometry'
import DialogSurface from './content/DialogSurface.vue'
import SettingsPopover from './SettingsPopover.vue'
import WidgetActions from './WidgetActions.vue'
import WidgetSettings from './WidgetSettings.vue'
import WolfCharacter from './WolfCharacter.vue'

const expanded = shallowRef(false)
const settingsOpen = shallowRef(false)
const preferences = shallowRef<Preferences>(structuredClone(defaultPreferences))
const theme = shallowRef(builtinTheme)
const initialized = shallowRef(false)
const visible = shallowRef(false)
const fullSettingsOpen = shallowRef(false)
const rendererReady = shallowRef(false)
const templateInteraction = shallowRef(false)
const now = shallowRef(Date.now())
const source = useContentSource(preferences, initialized, visible)
const { refresh } = source
const eligible = computed(() => initialized.value && visible.value && !fullSettingsOpen.value && !settingsOpen.value)
const pushes = usePushQueue(expanded, eligible)
const content = computed(() => pushes.active.value?.content ?? source.content.value)
const loading = computed(() => pushes.active.value ? pushes.active.value.status === 'preparing' : source.loading.value)
const error = computed(() => pushes.active.value ? '' : source.error.value)
const stageSize = computed(() => theme.value.layout)
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
const { press, rebound } = useWidgetMotion(motion, () => theme.value.layout)
const transform = computed(() => ({ transform: `scale(${preferences.value.scale})` }))
let observer: ResizeObserver | undefined
let unlisten: UnlistenFn | undefined
let resizeUnlisten: UnlistenFn | undefined
let sideUnlisten: UnlistenFn | undefined
let activityUnlisten: UnlistenFn | undefined
let statusTimer: ReturnType<typeof setInterval> | undefined
let audio: HTMLAudioElement | undefined
let themeGeneration = 0
let disposed = false
let suppressClick = false
let pointerCleanup: (() => void) | undefined
let frame = 0
const { syncHover, restart } = useContentDismiss({
  expanded,
  seconds: () => pushes.active.value?.durationSeconds ?? preferences.value.dismissSeconds,
  loading,
  settingsOpen,
  bubble,
  actions,
  paused: () => !visible.value || !rendererReady.value || templateInteraction.value || fullSettingsOpen.value,
  dismiss: collapse,
})
function rectStyle(rect: LayoutRect): Record<string, string> {
  return { left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.width}px`, height: `${rect.height}px` }
}
watch(() => JSON.stringify(preferences.value.appearance), async () => {
  const generation = ++themeGeneration
  const next = await desktop.theme(preferences.value.appearance)
  if (generation === themeGeneration && !disposed) {
    theme.value = next
    scheduleRegions()
  }
}, { immediate: true })
watch(() => content.value?.id, () => {
  restart()
}, { flush: 'post' })
watch([() => pushes.active.value?.id, eligible, rendererReady, loading], () => {
  const message = pushes.active.value
  if (message && eligible.value && rendererReady.value && !loading.value) {
    void desktop.showing(message.id).catch(() => {
    })
  }
})
async function mediaError(): Promise<void> {
  if (pushes.active.value)
    await pushes.finish('图片加载失败')
  else
    uiError.value = '图片加载失败，请检查素材或重新获取内容。'
}
function previewChanged(event: StorageEvent): void {
  if (event.key === 'silverwolf-preview')
    void desktop.preferences().then(value => preferences.value = value)
}
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
      return motionRegion({ x: element.offsetLeft, y: element.offsetTop, width: element.offsetWidth, height: element.offsetHeight }, preferences.value.scale, origin, stageSize.value)
    })
    if (settingsOpen.value && settingsRegion.value) {
      const element = settingsRegion.value
      regions.push(staticRegion({ x: element.offsetLeft, y: element.offsetTop, width: element.offsetWidth, height: element.offsetHeight }, preferences.value.scale, origin))
    }
    await desktop.regions(regions)
  }
  catch {
    uiError.value = '窗口交互区域更新失败，请从托盘退出后重开。'
  }
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
  if (pushes.active.value)
    void pushes.finish()
  else
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
  }
  catch {
    uiError.value = '设置保存失败，请检查配置目录或 CLI 路径。'
  }
  finally {
    saving.value = false
  }
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
  if (!preferences.value.appearance.muted && theme.value.sound) {
    audio?.pause()
    audio = new Audio(theme.value.sound)
    audio.volume = preferences.value.appearance.volume
    void audio.play().catch(() => {
      uiError.value = '音效播放失败，请更换音频文件。'
    })
  }
  if (!pushes.active.value && preferences.value.content.provider !== 'codex')
    void refresh()
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
    catch {
      uiError.value = '暂时无法拖动窗口。'
    }
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
    theme.value = await desktop.theme()
    initialized.value = true
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
      activityUnlisten = await listen<boolean>('settings-activity', event => fullSettingsOpen.value = event.payload)
      toolbarLeft.value = (await desktop.status()).toolbarLeft
    }
    else {
      window.addEventListener('storage', previewChanged)
    }
    statusTimer = setInterval(async () => {
      now.value = Date.now()
      try {
        const status = await desktop.status()
        if (!disposed) {
          visible.value = status.visible
          toolbarLeft.value = status.toolbarLeft
          fullSettingsOpen.value = status.settingsOpen
        }
      }
      catch {
      }
    }, 200)
    if (disposed) {
      unlisten?.()
      resizeUnlisten?.()
      sideUnlisten?.()
      activityUnlisten?.()
      clearInterval(statusTimer)
      observer?.disconnect()
      return
    }
    await updateRegions()
    await desktop.ready()
    visible.value = (await desktop.status()).visible
  }
  catch {
    uiError.value = '初始化失败，请重开挂件。'
  }
})
onUnmounted(() => {
  disposed = true
  observer?.disconnect()
  unlisten?.()
  resizeUnlisten?.()
  sideUnlisten?.()
  activityUnlisten?.()
  clearInterval(statusTimer)
  window.removeEventListener('storage', previewChanged)
  audio?.pause()
  pointerCleanup?.()
  cancelAnimationFrame(frame)
})
</script>

<template>
  <main ref="stage" class="stage" :style="{ ...transform, width: `${stageSize.width}px`, height: `${stageSize.height}px` }" @contextmenu.prevent>
    <div ref="motion" class="widget-motion" :inert="settingsOpen">
      <div v-show="expanded" ref="bubble" class="thought" :style="rectStyle(theme.layout.bubble)" @pointerenter="syncHover" @pointerleave="syncHover">
        <DialogSurface :theme="theme" :content="content" :expanded="expanded" :loading="loading" :error="error || uiError" :now="now" @ready="rendererReady = $event" @media-error="mediaError" @interaction="templateInteraction = $event" @refresh="refresh" @close="collapse" />
      </div>
      <div ref="character" class="character-position" :style="rectStyle(theme.layout.character)">
        <WolfCharacter :image="theme.character" :expanded="expanded" @toggle="characterClick" @pointerdown="pointerDown" @wheel="scaleBy($event.deltaY < 0 ? 0.1 : -0.1)" />
      </div>
      <div ref="actions" class="actions-position" :style="{ left: `${toolbarLeft ? theme.layout.actions.leftX : theme.layout.actions.x}px`, top: `${theme.layout.actions.y}px` }" :class="{ 'is-left': toolbarLeft }" @pointerenter="syncHover" @pointerleave="syncHover">
        <WidgetActions :expanded="expanded" :settings-open="settingsOpen" :loading="loading" @toggle="collapse" @refresh="refresh" @settings="showPanel(true)" />
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
