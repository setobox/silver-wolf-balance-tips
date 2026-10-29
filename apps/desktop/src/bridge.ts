import type { ContentDocument, ContentPreferences, QueueSnapshot, RuntimeTheme, ServerStatus, UsageSnapshot } from '@silverwolf/core'
import type { HitRegion, HostStatus, Preferences } from './types'
import { defaultContentPreferences, defaultLayout } from '@silverwolf/core'
import { invoke, isTauri } from '@tauri-apps/api/core'

export const native = isTauri()
export const defaultPreferences: Preferences = {
  scale: 1,
  follow: false,
  codexPath: '',
  dismissSeconds: 8,
  appearance: { themeId: 'builtin', character: null, sound: null, muted: false, volume: 0.5 },
  content: defaultContentPreferences(),
  http: { enabled: false, port: 38427 },
}
export const builtinTheme: RuntimeTheme = { id: 'builtin', name: '银狼 · 云朵', character: '/silverwolf.png', dialog: null, sound: null, layout: defaultLayout }
export function assetUrl(id: string): string {
  return `http://media.localhost/assets/${id}`
}
function previewPreferences(): Preferences {
  try {
    return JSON.parse(localStorage.getItem('silverwolf-preview') ?? 'null') ?? structuredClone(defaultPreferences)
  }
  catch {
    return structuredClone(defaultPreferences)
  }
}
async function previewSave(preferences: Preferences): Promise<Preferences> {
  localStorage.setItem('silverwolf-preview', JSON.stringify(preferences))
  return preferences
}
function nativeOnly<T>(): Promise<T> {
  return Promise.reject(new Error('请在桌面应用中使用此功能。'))
}
function constrainThemeOrigin(theme: RuntimeTheme): RuntimeTheme {
  if (native && theme.dialog && !document.querySelector('meta[data-theme-origin]')) {
    const policy = document.createElement('meta')
    policy.httpEquiv = 'Content-Security-Policy'
    policy.dataset.themeOrigin = ''
    // Narrow the build-time loopback wildcard before mounting any theme frame.
    // This also prevents a frame from navigating to other local services.
    policy.content = `frame-src ${new URL(theme.dialog).origin}`
    document.head.append(policy)
  }
  return theme
}
export const desktop = {
  preferences: (): Promise<Preferences> => native ? invoke('get_preferences') : Promise.resolve(previewPreferences()),
  save: (preferences: Preferences): Promise<Preferences> => native ? invoke('set_preferences', { preferences }) : previewSave(preferences),
  status: (): Promise<HostStatus> => native ? invoke('get_host_status') : Promise.resolve({ visible: true, attached: false, toolbarLeft: false, settingsOpen: false }),
  read: (): Promise<UsageSnapshot> => native ? invoke('read_usage') : Promise.reject(Object.assign(new Error('浏览器预览不读取账户，请在桌面应用中查看额度。'), { code: 'cli_missing' })),
  ready: (): Promise<void> => native ? invoke('ready') : Promise.resolve(),
  regions: (regions: HitRegion[]): Promise<void> => native ? invoke('set_hit_regions', { regions }) : Promise.resolve(),
  beginDrag: (): Promise<void> => native ? invoke('begin_drag') : Promise.resolve(),
  theme: (appearance?: Preferences['appearance']): Promise<RuntimeTheme> => native ? invoke<RuntimeTheme>('get_theme', { appearance: appearance ?? null }).then(constrainThemeOrigin) : Promise.resolve(builtinTheme),
  importAsset: (name: string, bytes: number[], kind: 'image' | 'sound'): Promise<string> => native ? invoke('import_asset', { name, bytes, kind }) : nativeOnly(),
  importTheme: (files: {
    path: string
    bytes: number[]
  }[] | null, archive: number[] | null): Promise<string> => native ? invoke('import_theme', { files, archive }) : nativeOnly(),
  source: (config: ContentPreferences, requestId: string, codexPath?: string): Promise<ContentDocument> => native ? invoke('read_source', { config, requestId, codexPath: codexPath ?? null }) : nativeOnly(),
  cancelSource: (requestId: string): Promise<void> => native ? invoke('cancel_source', { requestId }) : Promise.resolve(),
  queue: (): Promise<QueueSnapshot> => native ? invoke('get_queue') : Promise.resolve({ revision: 0, messages: [] }),
  showing: (id: string): Promise<boolean> => native ? invoke('mark_showing', { id }) : Promise.resolve(true),
  finish: (id: string, error: string | null = null): Promise<void> => native ? invoke('finish_message', { id, error }) : Promise.resolve(),
  clearQueue: (): Promise<void> => native ? invoke('clear_queue') : Promise.resolve(),
  serverStatus: (): Promise<ServerStatus> => native ? invoke('get_server_status') : Promise.resolve({ running: false, port: 38427, error: null, visible: true, queueLength: 0 }),
  token: (): Promise<string> => native ? invoke('get_http_token') : Promise.resolve('仅桌面应用可用'),
  openSettings: (): Promise<void> => native
    ? invoke('open_settings')
    : Promise.resolve().then(() => {
        window.open('?view=settings', 'silverwolf-settings')
      }),
  settingsActivity: (open: boolean): Promise<void> => native ? invoke('set_settings_activity', { open }) : Promise.resolve(),
}
