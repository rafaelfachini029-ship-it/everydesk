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

export interface KeyGate {
  /** The event's target is the remote screen (or nothing in particular: the page body). */
  canvasFocused: boolean
  /** The event's target is inside the pill. */
  pillFocused: boolean
  /** The first frame has been painted. */
  revealed: boolean
}

/** A key press goes to the remote only after the reveal, from the screen, never from the pill. */
export function shouldForwardKey(g: KeyGate): boolean {
  return g.revealed && g.canvasFocused && !g.pillFocused
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

/** Alt pressed and released with nothing else in between. */
export function createAltTap() {
  let armed = false
  return {
    down(key: string, repeat: boolean) {
      armed = key === 'Alt' && !repeat ? true : key === 'Alt' ? armed : false
    },
    /** Returns true on the release that completes a tap. */
    up(key: string): boolean {
      const tap = armed && key === 'Alt'
      armed = false
      return tap
    },
    cancel() {
      armed = false
    },
  }
}

/** The hide countdown: the same idle timer as Pill's collapse, with HIDE_MS. */
export function createHideTimer(onHide: () => void, deps?: IdleTimerDeps): IdleTimer {
  return createIdleTimer(HIDE_MS, onHide, deps)
}
