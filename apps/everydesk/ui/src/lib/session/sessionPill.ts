/**
 * Pure helpers for the floating session pill (spec §6.2 moment 3): latency text, the
 * keyboard gate, the pointer dead zone, the top-edge summon and the Alt tap. No DOM or
 * Tauri dependency so `node --test` can exercise them.
 */
import { createIdleTimer, type IdleTimer, type IdleTimerDeps } from '../ui/pill.ts'

/** The pill hides this long after the pointer and focus leave it. */
export const HIDE_MS = 2000
/** Pointer this close to the top of the window (px) brings the pill back. */
export const TOP_EDGE_PX = 8

/** "32 ms"; "— ms" before the first `session:quality` reading. */
export function formatLatency(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return '— ms'
  return `${Math.max(0, Math.round(ms))} ms`
}

/** "30 fps", or null when unknown. */
export function formatFps(fps: number | null | undefined): string | null {
  if (fps === null || fps === undefined || !Number.isFinite(fps)) return null
  return `${Math.max(0, Math.round(fps))} fps`
}

/** What the expanded pill holds in Stage 3: only tools that work. Chat, files, recording and
 *  voice come back in Stage 4, when they do something ("em dúvida, remover", spec §6.3).
 *  Quality is a reading (fps), not a control. */
export const PILL_TOOLS = ['monitores', 'qualidade', 'encerrar'] as const

export interface KeyGate {
  /** The event's target is the remote screen (or nothing in particular: the page body). */
  canvasFocused: boolean
  /** The event's target is inside the pill. */
  pillFocused: boolean
  /** The first frame has been painted. */
  revealed: boolean
  /** A message over the session (password, notice) is open: it owns the keyboard. */
  overlayOpen?: boolean
}

/** A key press goes to the remote only after the reveal, from the screen, never from the pill,
 *  and never while a message over the session is open. */
export function shouldForwardKey(g: KeyGate): boolean {
  return g.revealed && g.canvasFocused && !g.pillFocused && !g.overlayOpen
}

export interface Rect {
  left: number
  top: number
  right: number
  bottom: number
}

/**
 * The pointer is over the pill: nothing may reach the remote. `pill` is null while the pill is
 * hidden, so a hidden pill never swallows input.
 */
export function isDeadZone(x: number, y: number, pill: Rect | null): boolean {
  if (!pill) return false
  return x >= pill.left && x < pill.right && y >= pill.top && y < pill.bottom
}

export function nearTopEdge(clientY: number, edge = TOP_EDGE_PX): boolean {
  return clientY >= 0 && clientY <= edge
}

/** The top edge brings the pill back, but never mid-drag: with a button held (or the screen
 *  holding the pointer) a peek would put the pill under the pointer and cut the drag short. */
export function shouldPeek(p: { y: number; held: boolean; captured: boolean }): boolean {
  return !p.held && !p.captured && nearTopEdge(p.y)
}

// ---- Alt tap ------------------------------------------------------------------------
//
// A bare Alt tap summons the pill and must never reach the remote (on Windows it would
// focus the active app's menu bar). So a bare Alt keydown is held back: a key or a click
// while it is down commits it (Alt down goes first, then the combo, as usual); a release
// within ALT_TAP_MS swallows both and summons; a longer lone hold still reaches the remote.

/** A lone Alt released sooner than this is a tap. */
export const ALT_TAP_MS = 400

export type AltState =
  | { kind: 'idle' }
  /** Bare Alt down, not sent yet. `code` is the side (AltLeft/AltRight). */
  | { kind: 'pending'; code: string; at: number }
  /** Alt down was sent: from here on Alt behaves like any key. */
  | { kind: 'sent' }

export type AltEvent =
  | { type: 'down'; alt: boolean; code: string; repeat: boolean; t: number }
  | { type: 'up'; alt: boolean; t: number }
  /** A click or wheel while Alt is down: it is a modifier, not a tap. */
  | { type: 'interrupt' }
  /** Focus left the screen (blur, input, pill): forget a held-back Alt, nothing was sent. */
  | { type: 'reset' }

export interface AltStep {
  state: AltState
  /** Synthetic Alt key events to send first, in order. `code` is the held-back side. */
  send: { code: string; down: boolean }[]
  /** Let the current key event take its normal path (forward / release). */
  forward: boolean
  summon: boolean
}

export const ALT_IDLE: AltState = { kind: 'idle' }

export function altTapReducer(st: AltState, ev: AltEvent): AltStep {
  const step = (state: AltState, forward: boolean, send: AltStep['send'] = [], summon = false): AltStep => ({
    state,
    send,
    forward,
    summon,
  })
  if (ev.type === 'reset') return step(ALT_IDLE, false)
  if (st.kind !== 'pending') {
    if (ev.type === 'down' && ev.alt && !ev.repeat && st.kind === 'idle')
      return step({ kind: 'pending', code: ev.code, at: ev.t }, false)
    if (ev.type === 'up' && ev.alt) return step(ALT_IDLE, true)
    return step(st, ev.type !== 'interrupt')
  }
  // pending: a bare Alt is held back
  const commit = [{ code: st.code, down: true }]
  switch (ev.type) {
    case 'interrupt':
      return step({ kind: 'sent' }, false, commit)
    case 'down':
      if (!ev.alt) return step({ kind: 'sent' }, true, commit)
      // Alt auto-repeat (where the OS repeats modifiers): a long hold reaches the remote.
      if (ev.t - st.at >= ALT_TAP_MS) return step({ kind: 'sent' }, false, commit)
      return step(st, false)
    case 'up':
      if (!ev.alt) return step(st, true)
      if (ev.t - st.at < ALT_TAP_MS) return step(ALT_IDLE, false, [], true)
      return step(ALT_IDLE, false, [...commit, { code: st.code, down: false }])
  }
}

/** The hide countdown: the same idle timer as Pill's collapse, with HIDE_MS. */
export function createHideTimer(onHide: () => void, deps?: IdleTimerDeps): IdleTimer {
  return createIdleTimer(HIDE_MS, onHide, deps)
}
