import type { QuotaMeter } from './types.ts'

export function windowLabel(minutes: number): string {
  if (!Number.isFinite(minutes) || minutes <= 0)
    return '未知周期'
  if (minutes === 10080)
    return '每周'
  if (minutes % 1440 === 0)
    return `${minutes / 1440} 天`
  if (minutes % 60 === 0)
    return `${minutes / 60} 小时`
  return `${minutes} 分钟`
}

export function isStale(observedAt: number, meter: QuotaMeter, now = Date.now()): boolean {
  return now - observedAt > 180_000 || (meter.resetsAt !== null && meter.resetsAt <= now)
}

export function resetLabel(resetsAt: number | null, now = Date.now()): string {
  if (resetsAt === null)
    return '重置时间未知'
  const minutes = Math.ceil((resetsAt - now) / 60_000)
  if (minutes <= 0)
    return '等待刷新'
  if (minutes < 60)
    return `${minutes}m`
  if (minutes < 1440)
    return `${Math.floor(minutes / 60)}h${minutes % 60}m`
  return `${Math.floor(minutes / 1440)}d${Math.floor(minutes % 1440 / 60)}h${minutes % 60}m`
}
