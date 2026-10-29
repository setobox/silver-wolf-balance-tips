/** Captures the user's expanded state once per uninterrupted batch. */
export function createPushPresentation() {
  let interrupted = false
  let previousExpanded = false
  return {
    update(hasMessage: boolean, eligible: boolean, expanded: boolean): boolean {
      if (hasMessage && eligible) {
        if (!interrupted) {
          previousExpanded = expanded
          interrupted = true
        }
        return true
      }
      if (!hasMessage && interrupted) {
        interrupted = false
        return previousExpanded
      }
      return expanded
    },
  }
}
