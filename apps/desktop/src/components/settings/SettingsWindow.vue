<script setup lang="ts">
import type { ContentDocument } from '@silverwolf/core'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Preferences } from '../../types'
import { emptyContent } from '@silverwolf/core'
import { listen } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import { assetUrl, builtinTheme, defaultPreferences, desktop, native } from '../../bridge'
import DialogSurface from '../content/DialogSurface.vue'
import AppearanceSettings from './AppearanceSettings.vue'
import ContentSettings from './ContentSettings.vue'
import HttpSettings from './HttpSettings.vue'

const draft = ref<Preferences>(structuredClone(defaultPreferences))
const baseline = shallowRef(JSON.stringify(defaultPreferences))
const tab = shallowRef<'appearance' | 'content' | 'http' | 'window'>('appearance')
const theme = shallowRef(builtinTheme)
const error = shallowRef('')
const notice = shallowRef('')
const saving = shallowRef(false)
const importing = shallowRef(false)
const testing = shallowRef(false)
const invalid = shallowRef(false)
const preview = shallowRef<ContentDocument>({ ...emptyContent('preview'), text: '你好！这里是对话框预览。\n试试更换主题，或测试一个内容源。' })
const dirty = computed(() => JSON.stringify(draft.value) !== baseline.value)
let unlisten: UnlistenFn | undefined
let generation = 0
let disposed = false
let requestId: string | undefined
watch(() => JSON.stringify([draft.value.content, draft.value.codexPath]), () => {
  if (requestId) {
    void desktop.cancelSource(requestId)
    requestId = undefined
    testing.value = false
  }
})
watch(() => JSON.stringify(draft.value.appearance), async () => {
  const current = ++generation
  try {
    const value = await desktop.theme(JSON.parse(JSON.stringify(draft.value.appearance)))
    if (current === generation && !disposed)
      theme.value = value
  }
  catch (cause) {
    error.value = String(cause)
  }
}, { immediate: true })
async function save(): Promise<void> {
  saving.value = true
  error.value = ''
  notice.value = ''
  try {
    const previous: Preferences = JSON.parse(baseline.value)
    const latest = await desktop.preferences()
    const next = JSON.parse(JSON.stringify(draft.value)) as Preferences
    const patch = Object.fromEntries(Object.keys(next).filter(key => JSON.stringify(next[key as keyof Preferences]) !== JSON.stringify(previous[key as keyof Preferences])).map(key => [key, next[key as keyof Preferences]]))
    draft.value = await desktop.save({ ...latest, ...patch })
    baseline.value = JSON.stringify(draft.value)
    notice.value = '设置已保存'
  }
  catch (cause) {
    error.value = String(cause)
  }
  finally {
    saving.value = false
  }
}
async function test(): Promise<void> {
  testing.value = true
  error.value = ''
  const id = crypto.randomUUID()
  if (requestId)
    void desktop.cancelSource(requestId)
  requestId = id
  try {
    const config = JSON.parse(JSON.stringify(draft.value.content))
    if (config.provider === 'local') {
      const entry = draft.value.content.entries[0]
      if (!entry)
        throw new Error('请先添加内容。')
      preview.value = { ...emptyContent(id), text: entry.text, image: entry.image ? assetUrl(entry.image) : null }
    }
    else {
      const content = await desktop.source(config, id, draft.value.codexPath)
      if (!disposed && requestId === id)
        preview.value = content
    }
  }
  catch (cause) {
    if (!disposed && requestId === id)
      error.value = String(cause)
  }
  finally {
    if (requestId === id)
      testing.value = false
  }
}
onMounted(async () => {
  if (native) {
    unlisten = await listen<Preferences>('preferences-changed', (event) => {
      if (!dirty.value) {
        draft.value = event.payload
        baseline.value = JSON.stringify(event.payload)
      }
    })
  }
  const current = await desktop.preferences()
  if (disposed) {
    unlisten?.()
    return
  }
  draft.value = current
  baseline.value = JSON.stringify(current)
  await desktop.settingsActivity(true)
})
onUnmounted(() => {
  disposed = true
  unlisten?.()
  if (requestId)
    void desktop.cancelSource(requestId)
  void desktop.settingsActivity(false)
})
</script>

<template>
  <main class="settings-app">
    <header class="settings-header">
      <div>
        <p class="eyebrow">
          SILVERWOLF
        </p><h1>挂件设置</h1>
      </div><span class="ui-help">{{ dirty ? '有未保存的修改' : '你的桌面，你的角色' }}</span>
    </header>
    <div class="settings-body">
      <nav class="settings-nav" aria-label="设置分类">
        <button v-for="item in [{ id: 'appearance' as const, name: '外观', icon: 'i-lucide-palette' }, { id: 'content' as const, name: '内容', icon: 'i-lucide-message-circle' }, { id: 'http' as const, name: '外部推送', icon: 'i-lucide-radio' }, { id: 'window' as const, name: '窗口', icon: 'i-lucide-panels-top-left' }]" :key="item.id" class="nav-button" :aria-current="tab === item.id ? 'page' : undefined" @click="tab = item.id">
          <span :class="item.icon" class="ui-button-icon" aria-hidden="true" />{{ item.name }}
        </button>
      </nav>
      <div class="settings-scroll">
        <fieldset class="settings-fields" :disabled="saving">
          <AppearanceSettings v-if="tab === 'appearance'" v-model="draft.appearance" :theme="theme" @error="error = $event" @busy="importing = $event" />
          <ContentSettings v-else-if="tab === 'content'" v-model="draft.content" v-model:cli="draft.codexPath" :testing="testing" @test="test" @error="error = $event" @invalid="invalid = $event" />
          <HttpSettings v-else-if="tab === 'http'" v-model="draft.http" @error="error = $event" />
          <section v-else class="settings-section">
            <h2 class="section-title">
              放在合适的位置
            </h2><label class="ui-form-row"><span>跟随 Codex 窗口</span><input v-model="draft.follow" type="checkbox"></label><p class="ui-help">
              关闭时显示在桌面；开启后，没有可见的 Codex 窗口时挂件隐藏，推送在队列等待。
            </p><label class="field-label">角色大小 {{ Math.round(draft.scale * 100) }}%<input v-model.number="draft.scale" type="range" min="0.6" max="1.6" step="0.1"></label><label class="ui-form-row"><span>日常内容常驻</span><input type="checkbox" :checked="draft.dismissSeconds === null" @change="draft.dismissSeconds = ($event.target as HTMLInputElement).checked ? null : 8"></label><label v-if="draft.dismissSeconds !== null" class="field-label">自动收起（秒）<input v-model.number="draft.dismissSeconds" class="ui-input ui-number" type="number" min="1" max="3600"></label><p class="ui-help">
              悬停、键盘操作和设置窗口打开时暂停计时。HTTP 消息使用请求中的展示时长。
            </p>
          </section>
        </fieldset>
      </div>
      <aside class="preview-pane">
        <span class="preview-label">实时预览</span><div class="preview-bubble">
          <DialogSurface :theme="theme" :content="preview" expanded :loading="testing" error="" :now="Date.now()" @refresh="test" @media-error="error = '预览图片无法加载'" />
        </div><img :src="theme.character" class="preview-character" alt="角色预览"><p class="ui-help">
          {{ theme.name }}
        </p>
      </aside>
    </div>
    <footer class="settings-footer">
      <p v-if="error" class="ui-error" role="alert">
        {{ error }}
      </p><p v-else class="ui-help" role="status">
        {{ notice || '保存后应用到桌面挂件' }}
      </p><button class="ui-button save-button" :disabled="saving || importing || invalid || !dirty" @click="save">
        {{ saving ? '保存中…' : '保存设置' }}
      </button>
    </footer>
  </main>
</template>

<style>
.settings-app { --surface: #fff; --panel: #f7f5fa; height: 100%; display: flex; flex-direction: column; background: var(--surface); color: var(--ink); font-size: 12px; }
.settings-header { padding: 24px 28px 20px; border-bottom: 1px solid var(--line); display: flex; align-items: end; justify-content: space-between; gap: 16px; }
.settings-header h1 { font-size: 22px; margin: 6px 0 0; font-weight: 650; }
.eyebrow { margin: 0; font-size: 10px; letter-spacing: 2px; color: var(--accent); }
.settings-body { flex: 1; min-height: 0; display: grid; grid-template-columns: 126px minmax(280px, 1fr) 250px; }
.settings-nav { padding: 16px 8px; background: var(--panel); border-right: 1px solid var(--line); }
.nav-button { display: flex; align-items: center; gap: 9px; width: 100%; padding: 10px; margin-bottom: 4px; border: 0; border-radius: 7px; text-align: left; color: var(--muted); background: transparent; font-size: 12px; }
.nav-button:hover, .nav-button[aria-current] { background: var(--control-active); color: var(--ink); }
.settings-scroll { padding: 22px; overflow-y: auto; }
.settings-fields { min-width: 0; margin: 0; border: 0; padding: 0; }
.settings-section { display: flex; flex-direction: column; gap: 14px; }
.section-title { margin: 0; font-size: 16px; font-weight: 600; }
.setting-group { display: flex; flex-direction: column; gap: 12px; min-width: 0; border: 1px solid var(--line); border-radius: 8px; padding: 14px; margin: 0; }
.setting-group legend { padding: 0 5px; font-size: 12px; }
.setting-group p { margin: 0; }
.button-row { display: flex; flex-wrap: wrap; gap: 6px; }
.file-button { display: inline-flex; align-items: center; position: relative; cursor: pointer; overflow: hidden; }
.file-button input { position: absolute; inset: 0; opacity: 0; width: 100%; cursor: pointer; }
.file-button:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
.field-label { display: flex; flex-direction: column; gap: 6px; }
.text-area { height: auto; resize: vertical; padding: 8px; line-height: 1.6; min-height: 60px; }
.text-entry { display: flex; flex-direction: column; gap: 8px; padding-bottom: 14px; border-bottom: 1px solid var(--line); }
.category-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.category-grid label { display: flex; align-items: center; gap: 4px; }
.preview-pane { display: flex; flex-direction: column; align-items: center; padding: 20px 8px; border-left: 1px solid var(--line); background: var(--panel); overflow: auto; }
.preview-label { font-size: 11px; color: var(--muted); align-self: flex-start; margin-left: 12px; }
.preview-bubble { width: 230px; height: 254px; flex-shrink: 0; margin-top: 16px; }
.preview-character { width: 180px; height: 185px; object-fit: contain; margin-top: -8px; }
.settings-footer { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 14px 24px; border-top: 1px solid var(--line); }
.settings-footer p { margin: 0; overflow-wrap: anywhere; }
.save-button { background: var(--accent); color: white; padding: 0 18px; height: 32px; flex-shrink: 0; }
@media (prefers-color-scheme: dark) {
  .settings-app { --ink: #e8e3ef; --muted: #aaa2b7; --accent: #b49bd7; --surface: #211e28; --panel: #28242f; --line: #453d51; --control-bg: #302a3a; --control-hover: #3c3349; --control-active: #41354f; --error: #e9b881; color-scheme: dark; }
  .settings-app .save-button { color: #211e28; }
  .preview-bubble { --ink: #242134; --muted: #807a88; --accent: #8774ac; --line: #ded8e8; color: var(--ink); color-scheme: light; }
}
@media (max-width: 760px) { .settings-body { grid-template-columns: 110px minmax(270px, 1fr) 210px; } .settings-scroll { padding: 18px; } .preview-bubble { width: 200px; } }
</style>
