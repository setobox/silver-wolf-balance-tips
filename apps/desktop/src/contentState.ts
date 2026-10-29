import type { ContentDocument } from '@silverwolf/core'

export interface ContentState {
  content: ContentDocument | null
  loading: boolean
  error: string
}
export function createContentState(changed: (state: ContentState) => void) {
  let state: ContentState = { content: null, loading: false, error: '' }
  let controller: AbortController | undefined
  let generation = 0
  let disposed = false
  function publish(patch: Partial<ContentState>): void {
    state = { ...state, ...patch }
    if (!disposed)
      changed(state)
  }
  return {
    async refresh(read: (signal: AbortSignal) => Promise<ContentDocument>, clearOnError: boolean): Promise<void> {
      controller?.abort()
      controller = new AbortController()
      const current = ++generation
      publish({ loading: true, error: '' })
      try {
        const content = await read(controller.signal)
        if (!disposed && current === generation)
          publish({ content, loading: false })
      }
      catch (error) {
        if (!disposed && current === generation)
          publish({ content: clearOnError ? null : state.content, loading: false, error: error instanceof Error ? error.message : String(error) })
      }
    },
    reset(): void {
      controller?.abort()
      generation++
      publish({ content: null, loading: false, error: '' })
    },
    dispose(): void {
      disposed = true
      generation++
      controller?.abort()
    },
  }
}
