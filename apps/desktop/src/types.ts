export interface Preferences {
  scale: number
  follow: boolean
  codexPath: string
  quotaDismissSeconds: number | null
}

export interface HostStatus {
  visible: boolean
  attached: boolean
  toolbarLeft: boolean
}

export interface HitRegion {
  x: number
  y: number
  width: number
  height: number
}
