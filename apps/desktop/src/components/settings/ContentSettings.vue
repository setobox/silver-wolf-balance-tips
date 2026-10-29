<script setup lang="ts">
import type { ContentPreferences } from '@silverwolf/core'
import { onUnmounted, shallowRef, watch } from 'vue'
import { desktop } from '../../bridge'

defineProps<{
  testing: boolean
}>()
const emit = defineEmits<{
  test: [
  ]
  error: [
        message: string,
  ]
  invalid: [
        value: boolean,
  ]
}>()
const model = defineModel<ContentPreferences>({ required: true })
const cli = defineModel<string>('cli', { required: true })
const headers = shallowRef('')
const query = shallowRef('')
const invalidHeaders = shallowRef(false)
const invalidQuery = shallowRef(false)
onUnmounted(() => emit('invalid', false))
function addEntry(): void {
  model.value.entries.push({ id: crypto.randomUUID(), text: '', image: null })
}
watch(() => model.value.api, (api) => {
  headers.value = JSON.stringify(api.headers, null, 2)
  query.value = JSON.stringify(api.query, null, 2)
}, { immediate: true })
function mapInput(kind: 'headers' | 'query', value: string): void {
  if (kind === 'headers')
    headers.value = value
  else
    query.value = value
  const invalid = kind === 'headers' ? invalidHeaders : invalidQuery
  try {
    const parsed: unknown = JSON.parse(value)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed) || Object.values(parsed).some(v => typeof v !== 'string'))
      throw new Error('invalid')
    model.value.api[kind] = parsed as Record<string, string>
    invalid.value = false
  }
  catch {
    invalid.value = true
  }
  emit('invalid', invalidHeaders.value || invalidQuery.value)
}
function move(index: number, offset: number): void {
  const entries = [...model.value.entries]
  const target = index + offset
  if (target < 0 || target >= entries.length) {
    return
  }
  [entries[index], entries[target]] = [entries[target]!, entries[index]!]
  model.value.entries = entries
}
async function attach(event: Event, id: string): Promise<void> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file)
    return
  try {
    if (file.size > 8 * 1024 * 1024)
      throw new Error('图片不能超过 8 MB')
    const asset = await desktop.importAsset(file.name, Array.from(new Uint8Array(await file.arrayBuffer())), 'image')
    const entry = model.value.entries.find(e => e.id === id)
    if (entry)
      entry.image = asset
  }
  catch (error) {
    emit('error', String(error))
  }
}
</script>

<template>
  <div class="settings-section">
    <h2 class="section-title">
      平时想看什么
    </h2>
    <label class="field-label">日常内容源<select v-model="model.provider" class="ui-input"><option value="local">我的文字与图片</option><option value="hitokoto">一言 / 动漫语录</option><option value="http">自定义语录 API</option><option value="codex">Codex 剩余额度</option></select></label>
    <template v-if="model.provider === 'local'">
      <label class="ui-form-row"><span>随机取句</span><input v-model="model.random" type="checkbox"></label>
      <div v-for="(entry, index) in model.entries" :key="entry.id" class="text-entry">
        <label class="field-label">第 {{ index + 1 }} 条<textarea v-model="entry.text" class="ui-input text-area" rows="3" placeholder="写一句话，也可以只添加图片" /></label>
        <div class="button-row">
          <button class="ui-button" :disabled="index === 0" aria-label="上移" @click="move(index, -1)">
            ↑
          </button><button class="ui-button" :disabled="index === model.entries.length - 1" aria-label="下移" @click="move(index, 1)">
            ↓
          </button>
          <label class="file-button ui-button">{{ entry.image ? '更换图片' : '添加图片' }}<input type="file" accept="image/png,image/jpeg,image/webp,image/gif" @change="attach($event, entry.id)"></label>
          <button v-if="entry.image" class="ui-button" @click="entry.image = null">
            移除图片
          </button>
          <button class="ui-button" @click="model.entries.splice(index, 1)">
            删除
          </button>
        </div>
      </div>
      <button class="ui-button" @click="addEntry">
        ＋ 添加一条
      </button>
    </template>
    <fieldset v-if="model.provider === 'hitokoto'" class="setting-group">
      <legend>一言分类（未选时随机）</legend>
      <div class="category-grid">
        <label v-for="category in [{ id: 'a', name: '动画' }, { id: 'b', name: '漫画' }, { id: 'c', name: '游戏' }, { id: 'd', name: '文学' }, { id: 'e', name: '原创' }, { id: 'f', name: '网络' }, { id: 'g', name: '其他' }, { id: 'h', name: '影视' }, { id: 'i', name: '诗词' }, { id: 'j', name: '网易云' }, { id: 'k', name: '哲学' }, { id: 'l', name: '抖机灵' }]" :key="category.id"><input v-model="model.categories" type="checkbox" :value="category.id">{{ category.name }}</label>
      </div>
      <button class="ui-button" @click="model.categories = ['a', 'b', 'c']">
        动漫 / 漫画 / 游戏预设
      </button>
    </fieldset>
    <template v-if="model.provider === 'http'">
      <label class="field-label">接口地址（GET）<input v-model="model.api.url" class="ui-input ui-number" placeholder="https://example.com/quote"></label>
      <label class="field-label">响应格式<select v-model="model.api.format" class="ui-input"><option value="json">JSON</option><option value="text">纯文本</option></select></label>
      <template v-if="model.api.format === 'json'">
        <label v-for="field in [{ key: 'textPointer' as const, label: '文字字段' }, { key: 'authorPointer' as const, label: '作者字段' }, { key: 'sourcePointer' as const, label: '出处字段' }, { key: 'imagePointer' as const, label: '图片 URL 字段' }]" :key="field.key" class="field-label">{{ field.label }}<input v-model="model.api[field.key]" class="ui-input ui-number" placeholder="例如 /data/text，留空不读取"></label>
        <p class="ui-help">
          字段使用 JSON Pointer，例如 /data/0/text 读取数组第一条的 text。
        </p>
      </template>
      <label class="field-label">查询参数（JSON，值为字符串）<textarea :value="query" class="ui-input text-area ui-number" rows="3" :aria-invalid="invalidQuery" @input="mapInput('query', ($event.target as HTMLTextAreaElement).value)" /></label>
      <label class="field-label">请求头（JSON，值为字符串）<textarea :value="headers" class="ui-input text-area ui-number" rows="3" :aria-invalid="invalidHeaders" @input="mapInput('headers', ($event.target as HTMLTextAreaElement).value)" /></label>
      <p v-if="invalidHeaders || invalidQuery" class="ui-error">
        请输入有效的 JSON 对象，例如 {"key":"value"}。
      </p>
    </template>
    <label v-if="model.provider === 'codex'" class="field-label">Codex CLI 路径<input v-model="cli" class="ui-input ui-number" placeholder="留空自动查找"><span class="ui-help">仅选择 Codex 内容源时读取额度；可见时每 60 秒刷新。</span></label>
    <template v-else>
      <label class="ui-form-row"><span>定时换句</span><input type="checkbox" :checked="model.intervalSeconds !== null" @change="model.intervalSeconds = ($event.target as HTMLInputElement).checked ? 60 : null"></label>
      <label v-if="model.intervalSeconds !== null" class="field-label">间隔（秒，10–86400）<input v-model.number="model.intervalSeconds" class="ui-input ui-number" type="number" min="10" max="86400"></label>
    </template>
    <button class="ui-button" :disabled="testing || invalidHeaders || invalidQuery" @click="emit('test')">
      {{ testing ? '正在测试…' : '测试此内容源' }}
    </button>
  </div>
</template>
