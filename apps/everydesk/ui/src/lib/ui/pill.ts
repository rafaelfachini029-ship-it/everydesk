/**
 * Collapse timer for Pill.svelte. Expanding "pokes" the timer; while the pointer or focus is
 * inside it is "held" and never fires; on release it fires after `idleMs` of no interaction.
 */

export interface IdleTimerDeps {
  setTimeout: (fn: () => void, ms: number) => unknown
  clearTimeout: (id: unknown) => void
}

export interface IdleTimer {
  /** Interaction happened: restart the countdown (unless held). */
  poke(): void
  /** Pointer/focus is inside: stop counting until release(). */
  hold(): void
  /** Pointer/focus left: start counting again. */
  release(): void
  cancel(): void
}

export function createIdleTimer(
  /** Milliseconds, or a getter so a changing prop is read at each restart. */
  idleMs: number | (() => number),
  onIdle: () => void,
  deps: IdleTimerDeps = {
    setTimeout: (fn, ms) => globalThis.setTimeout(fn, ms),
    clearTimeout: (id) => globalThis.clearTimeout(id as ReturnType<typeof setTimeout>),
  },
): IdleTimer {
  let id: unknown = null
  let held = false

  const cancel = () => {
    if (id !== null) deps.clearTimeout(id)
    id = null
  }
  const start = () => {
    cancel()
    if (held) return
    id = deps.setTimeout(() => {
      id = null
      onIdle()
    }, Math.max(0, typeof idleMs === 'function' ? idleMs() : idleMs))
  }

  return {
    poke: start,
    hold() {
      held = true
      cancel()
    },
    release() {
      held = false
      start()
    },
    cancel,
  }
}
