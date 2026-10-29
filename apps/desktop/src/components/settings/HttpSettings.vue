<script setup lang="ts">
import type { ServerStatus } from '@silverwolf/core'
import { computed, onMounted, onUnmounted, shallowRef } from 'vue'
import { desktop } from '../../bridge'

const emit = defineEmits<{
  error: [
        message: string,
  ]
}>()
const model = defineModel<{
  enabled: boolean
  port: number
}>({ required: true })
const status = shallowRef<ServerStatus | null>(null)
const token = shallowRef('')
const revealed = shallowRef(false)
const copied = shallowRef(false)
const example = computed(() => `$headers = @{ Authorization = 'Bearer YOUR_TOKEN' }
$body = @{ text = '休息一下吧'; durationSeconds = 8 } | ConvertTo-Json
$message = Invoke-RestMethod -Method Post -Uri http://127.0.0.1:${model.value.port}/v1/messages -Headers $headers -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body))
Invoke-RestMethod -Uri "http://127.0.0.1:${model.value.port}/v1/messages/$($message.id)" -Headers $headers`)
let timer: ReturnType<typeof setInterval> | undefined
async function poll(): Promise<void> {
  try {
    status.value = await desktop.serverStatus()
  }
  catch (error) {
    emit('error', String(error))
  }
}
async function showToken(): Promise<void> {
  try {
    token.value = await desktop.token()
    revealed.value = !revealed.value
  }
  catch (error) {
    emit('error', String(error))
  }
}
async function copy(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text)
    copied.value = true
  }
  catch {
    emit('error', '无法复制，请手动选择文本复制。')
  }
}
async function clear(): Promise<void> {
  await desktop.clearQueue()
  await poll()
}
onMounted(() => {
  void poll()
  timer = setInterval(() => {
    void poll()
  }, 1500)
})
onUnmounted(() => clearInterval(timer))
</script>

<template>
  <div class="settings-section">
    <h2 class="section-title">
      让本机应用发来消息
    </h2>
    <label class="ui-form-row"><span>开启 HTTP 接收</span><input v-model="model.enabled" type="checkbox"></label>
    <label class="field-label">监听端口<input v-model.number="model.port" class="ui-input ui-number" type="number" min="1" max="65535"></label>
    <p class="ui-help">
      保存后生效，只监听 127.0.0.1。跟随 Codex 时，隐藏期间的消息会等待窗口恢复可见。
    </p>
    <div v-if="status" class="setting-group" role="status">
      <strong>{{ status.running ? `正在监听 127.0.0.1:${status.port}` : '服务未启动' }}</strong><p>队列 {{ status.queueLength }} 条 · 挂件{{ status.visible ? '可见' : '隐藏' }}</p><p v-if="status.error" class="ui-error">
        {{ status.error }}
      </p><button class="ui-button" :disabled="!status.queueLength" @click="clear">
        清空推送队列
      </button>
    </div>
    <div class="button-row">
      <button class="ui-button" @click="showToken">
        {{ revealed ? '隐藏令牌' : '显示访问令牌' }}
      </button><button v-if="token" class="ui-button" @click="copy(token)">
        复制令牌
      </button>
    </div>
    <input v-if="token" class="ui-input ui-number" :type="revealed ? 'text' : 'password'" :value="token" readonly aria-label="访问令牌">
    <label class="field-label">文字推送示例<textarea class="ui-input text-area ui-number" rows="6" :value="example" readonly /></label>
    <button class="ui-button" @click="copy(example)">
      复制调用示例
    </button><span v-if="copied" class="ui-help" role="status">已复制</span>
    <p class="ui-help">
      替换 YOUR_TOKEN 后在 PowerShell 中执行。202 表示已接收；GET /v1/messages/消息ID 可查询 waiting、showing、completed 或 failed。图片可上传到 /v1/assets，或使用 image: {"url":"https://…"}。
    </p>
  </div>
</template>
