<script setup lang="ts">
import type { UsageError, UsageSnapshot } from '@silverwolf/core'
import { isStale, resetLabel, windowLabel } from '@silverwolf/core'

defineProps<{ snapshot: UsageSnapshot | null, error: UsageError | null, loading: boolean, now: number }>()
</script>

<template>
  <section class="quota" aria-label="Codex 剩余额度" aria-live="polite" :aria-busy="loading">
    <p v-if="loading" class="message">
      正在读取额度<span class="pulse">…</span>
    </p>
    <div v-else-if="error" class="message">
      <p class="error-title">
        暂时无法读取
      </p>
      <p>{{ error.message }}</p>
    </div>
    <template v-else-if="snapshot">
      <p v-if="!snapshot.meters.length" class="message">
        当前账户没有返回额度窗口。
      </p>
      <div v-for="meter in snapshot.meters" :key="meter.id" class="meter">
        <template v-if="meter.kind === 'quota'">
          <div class="meter-heading">
            <span class="period" :title="meter.label">{{ windowLabel(meter.durationMinutes) }}<small v-if="meter.label !== 'Codex'">{{ meter.label }}</small></span>
            <div class="meter-value">
              <span class="reset" :class="{ stale: isStale(snapshot.observedAt, meter, now) }" :title="`${isStale(snapshot.observedAt, meter, now) ? '额度待更新 · ' : ''}重置倒计时：${resetLabel(meter.resetsAt, now)}`" :aria-label="`重置倒计时：${resetLabel(meter.resetsAt, now)}`">{{ resetLabel(meter.resetsAt, now) }}</span>
              <strong class="amount" :class="{ low: meter.remainingPercent < 20 }">{{ Math.round(meter.remainingPercent) }}<span>%</span></strong>
            </div>
          </div>
          <div class="track" role="progressbar" :aria-label="`${meter.label} ${windowLabel(meter.durationMinutes)}剩余额度`" :aria-valuenow="meter.remainingPercent" :aria-valuemin="0" :aria-valuemax="100">
            <div class="fill" :class="{ low: meter.remainingPercent < 20 }" :style="{ width: `${meter.remainingPercent}%` }" />
          </div>
        </template>
      </div>
    </template>
    <p v-else class="message">
      点击刷新，查看剩余额度。
    </p>
  </section>
</template>

<style scoped>
.quota { width: 100%; }
.meter + .meter { margin-top: 14px; }
.meter-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.period { min-width: 0; color: var(--muted); font-size: 14px; font-weight: 600; }
.period small { display: block; max-width: 80px; overflow: hidden; text-overflow: ellipsis; font-size: 9px; font-weight: 400; white-space: nowrap; }
.meter-value { display: flex; align-items: baseline; flex-shrink: 0; gap: 7px; }
.amount { color: var(--accent); font-size: 32px; font-weight: 750; line-height: 1; letter-spacing: -1.5px; font-variant-numeric: tabular-nums; }
.amount span { font-size: 28px; }
.amount.low { color: #ae7a20; }
.track { height: 6px; margin-top: 4px; overflow: hidden; border-radius: 8px; background: #eae6f1; }
.fill { height: 100%; border-radius: inherit; background: var(--accent); transition: width .25s; }
.fill.low { background: #c99b47; }
.reset { color: var(--muted); font-size: 12px; line-height: 1.2; font-variant-numeric: tabular-nums; white-space: nowrap; }
.reset.stale { text-decoration: underline dotted; text-underline-offset: 3px; }
.message { margin: 18px 0; color: var(--muted); font-size: 12px; line-height: 1.7; }
.message p { margin: 0 0 5px; }
.error-title { color: #a36c28; font-weight: 600; }
.pulse { animation: pulse 1s alternate infinite; }
@keyframes pulse { to { opacity: .3; } }
</style>
