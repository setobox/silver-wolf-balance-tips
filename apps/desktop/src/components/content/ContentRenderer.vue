<script setup lang="ts">
import type { ContentDocument } from '@silverwolf/core'
import QuotaBubble from '../QuotaBubble.vue'

defineProps<{
  content: ContentDocument | null
  loading: boolean
  error: string
  now: number
}>()
</script>

<template>
  <section class="content" aria-live="polite" :aria-busy="loading">
    <QuotaBubble v-if="content?.usage" :snapshot="content.usage" :loading="loading" :error="null" :now="now" />
    <template v-else-if="content">
      <p v-if="content.text" class="content-text">
        {{ content.text }}
      </p>
      <img v-if="content.image" :src="content.image" alt="展示图片" class="content-image" draggable="false">
      <p v-if="content.author || content.source" class="content-credit">
        {{ [content.author, content.source].filter(Boolean).join(' · ') }}
      </p>
    </template>
    <p v-else-if="!error" class="empty">
      {{ loading ? '正在获取内容…' : '点击角色显示内容。' }}
    </p>
    <p v-if="error" class="ui-error" role="alert">
      {{ error }}
    </p>
  </section>
</template>

<style scoped>
.content { width: 100%; min-width: 0; }
.content-text { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 14px; line-height: 1.7; margin: 0 0 8px; user-select: text; }
.content-image { display: block; width: 100%; max-height: 400px; object-fit: contain; border-radius: 6px; }
.content-credit, .empty { color: var(--muted); font-size: 11px; line-height: 1.6; }
</style>
