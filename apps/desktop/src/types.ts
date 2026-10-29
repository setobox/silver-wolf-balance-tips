import type { AppearancePreferences, ContentPreferences } from '@silverwolf/core'

export interface Preferences {
  scale: number
  follow: boolean
  codexPath: string
  dismissSeconds: number | null
  appearance: AppearancePreferences
  content: ContentPreferences
  http: { enabled: boolean, port: number }
}

export interface HostStatus {
  visible: boolean
  attached: boolean
  toolbarLeft: boolean
  settingsOpen: boolean
}

export interface HitRegion {
  x: number
  y: number
  width: number
  height: number
}
