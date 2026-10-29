import type { UsageSnapshot } from './types.ts'

/** Only display data crosses the template boundary; never preferences or credentials. */
export interface ContentDocument {
  id: string
  title: string
  text: string
  image: string | null
  author: string
  source: string
  observedAt: number
  usage: UsageSnapshot | null
}

export interface ContentProvider {
  readonly id: string
  read: (signal: AbortSignal) => Promise<ContentDocument>
}

export interface TextEntry { id: string, text: string, image: string | null }
export interface ContentPreferences {
  provider: 'local' | 'codex' | 'hitokoto' | 'http'
  entries: TextEntry[]
  random: boolean
  intervalSeconds: number | null
  categories: string[]
  api: {
    url: string
    query: Record<string, string>
    headers: Record<string, string>
    format: 'text' | 'json'
    textPointer: string
    authorPointer: string
    sourcePointer: string
    imagePointer: string
  }
}

export interface LayoutRect { x: number, y: number, width: number, height: number }
export interface ThemeLayout {
  width: number
  height: number
  character: LayoutRect
  bubble: LayoutRect
  actions: { x: number, leftX: number, y: number }
}
export interface RuntimeTheme {
  id: string
  name: string
  character: string
  dialog: string | null
  sound: string | null
  layout: ThemeLayout
}
export interface AppearancePreferences { themeId: string, character: string | null, sound: string | null, muted: boolean, volume: number }
export interface PushMessage {
  id: string
  content: ContentDocument
  durationSeconds: number
  status: 'preparing' | 'waiting' | 'showing' | 'completed' | 'failed'
  error: string | null
}
export interface QueueSnapshot { revision: number, messages: PushMessage[] }
export interface ServerStatus { running: boolean, port: number, error: string | null, visible: boolean, queueLength: number }

export function emptyContent(id = ''): ContentDocument {
  return { id, title: '', text: '', image: null, author: '', source: '', observedAt: Date.now(), usage: null }
}

export function defaultContentPreferences(): ContentPreferences {
  return {
    provider: 'local',
    entries: [{ id: 'welcome', text: '你好！点击我换一句话，在设置中选择喜欢的外观和内容。', image: null }],
    random: false,
    intervalSeconds: null,
    categories: ['a'],
    api: { url: '', query: {}, headers: {}, format: 'json', textPointer: '/text', authorPointer: '', sourcePointer: '', imagePointer: '' },
  }
}
