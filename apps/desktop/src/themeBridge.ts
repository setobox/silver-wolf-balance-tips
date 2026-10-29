export type ThemeMessage = {
  type: 'ready' | 'refresh' | 'close' | 'error'
} | {
  type: 'interaction'
  active: boolean
} | {
  type: 'rendered'
  contentId: string
}
export function parseThemeMessage(data: unknown, session: string): ThemeMessage | null {
  if (!data || typeof data !== 'object')
    return null
  const value = data as Record<string, unknown>
  if (value.channel !== 'silverwolf-theme-v1' || value.session !== session)
    return null
  if (['ready', 'refresh', 'close', 'error'].includes(String(value.type)))
    return { type: value.type as 'ready' | 'refresh' | 'close' | 'error' }
  if (value.type === 'interaction' && typeof value.active === 'boolean')
    return { type: 'interaction', active: value.active }
  if (value.type === 'rendered' && typeof value.contentId === 'string' && value.contentId.length <= 100)
    return { type: 'rendered', contentId: value.contentId }
  return null
}
