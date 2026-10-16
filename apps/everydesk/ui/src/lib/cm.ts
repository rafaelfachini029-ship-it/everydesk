// Accessed side (connection manager): event names, payloads and the pure countdown helper
// used by IncomingCard. No DOM/Tauri imports, so `node --test` can exercise it.

/** Matches `CLICK_APPROVE_TIMEOUT_SECS` in src/server/connection.rs. */
export const CLICK_APPROVE_TIMEOUT_SECS = 60

export const CM_EVENTS = {
  incoming: 'cm:incoming',
  connected: 'cm:connected',
  removed: 'cm:removed',
} as const

export interface Incoming {
  conn_id: number
  peer_id: string
  name: string
  is_file_transfer: boolean
}

export interface Connected {
  conn_id: number
  peer_id: string
  name: string
}

export interface PendingSnapshot extends Incoming {
  /** How long the prompt has been up (ms) when `cm_pending` answered. */
  age_ms: number
}

/** `cm_pending` result: what the UI shows, for hydrating after mount. */
export interface CmSnapshot {
  pending: PendingSnapshot[]
  connected: Connected[]
}

export interface Removed {
  conn_id: number
}

/** Whole seconds left of the accept window opened at `start` (ms), clamped to [0, timeout]. */
export function remainingSeconds(
  start: number,
  now: number,
  timeoutSecs: number = CLICK_APPROVE_TIMEOUT_SECS,
): number {
  const left = Math.ceil((start + timeoutSecs * 1000 - now) / 1000)
  return Math.max(0, Math.min(timeoutSecs, left))
}
