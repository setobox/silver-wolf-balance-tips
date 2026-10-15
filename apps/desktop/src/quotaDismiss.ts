export function validDismissSeconds(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 3600
}

export interface DismissState {
  expanded: boolean
  seconds: number | null
  loading: boolean
  settingsOpen: boolean
  hovered: boolean
  keyboardFocused: boolean
}

interface Scheduler {
  schedule: (callback: () => void, delay: number) => () => void
}

export function createDismissTimer(dismiss: () => void, scheduler: Scheduler = {
  schedule(callback, delay) {
    const timer = setTimeout(callback, delay)
    return () => clearTimeout(timer)
  },
}) {
  let cancel: (() => void) | undefined
  let generation = 0
  let disposed = false

  function clear(): void {
    generation++
    cancel?.()
    cancel = undefined
  }

  return {
    update(state: DismissState) {
      clear()
      if (disposed || !state.expanded || state.seconds === null || !validDismissSeconds(state.seconds)
        || state.loading || state.settingsOpen || state.hovered || state.keyboardFocused) {
        return
      }
      const current = generation
      cancel = scheduler.schedule(() => {
        if (current !== generation || disposed)
          return
        clear()
        dismiss()
      }, state.seconds * 1000)
    },
    dispose() {
      disposed = true
      clear()
    },
  }
}
