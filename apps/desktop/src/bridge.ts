import type { UsageSnapshot } from '@silverwolf/core'
import type { HitRegion, HostStatus, Preferences } from './types'
import { invoke, isTauri } from '@tauri-apps/api/core'

export const native = isTauri()
export const defaultPreferences: Preferences = { scale: 1, follow: true, codexPath: '' }

export const desktop = {
  preferences: (): Promise<Preferences> => native ? invoke('get_preferences') : Promise.resolve({ ...defaultPreferences }),
  save: (preferences: Preferences): Promise<Preferences> => native ? invoke('set_preferences', { preferences }) : Promise.resolve(preferences),
  status: (): Promise<HostStatus> => native ? invoke('get_host_status') : Promise.resolve({ visible: true, attached: false }),
  read: (): Promise<UsageSnapshot> => native ? invoke('read_usage') : Promise.reject(Object.assign(new Error('浏览器预览不读取账户，请在桌面应用中查看额度。'), { code: 'cli_missing' })),
  ready: (): Promise<void> => native ? invoke('ready') : Promise.resolve(),
  regions: (regions: HitRegion[]): Promise<void> => native ? invoke('set_hit_regions', { regions }) : Promise.resolve(),
  beginDrag: (): Promise<void> => native ? invoke('begin_drag') : Promise.resolve(),
}
