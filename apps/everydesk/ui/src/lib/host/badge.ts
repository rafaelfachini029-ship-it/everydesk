/**
 * Accessed side: who is connected right now, for ConnectedBadge. The list is fed by
 * IncomingCard's existing `cm:*` listeners and `cm_pending` hydration (one set of listeners);
 * this module only holds the store and the pure reducer. No DOM or Tauri dependency.
 */
import { writable } from 'svelte/store'
import type { Connected } from '../cm.ts'

export type CmStep = { type: 'connected'; session: Connected } | { type: 'removed'; conn_id: number }

/**
 * Connected minus removed, oldest first. A repeated `connected` for the same conn_id updates
 * it in place, so a badge never jumps in the stack.
 */
export function applyCm(list: readonly Connected[], step: CmStep): Connected[] {
  if (step.type === 'removed') return list.filter((c) => c.conn_id !== step.conn_id)
  const s = step.session
  const i = list.findIndex((c) => c.conn_id === s.conn_id)
  if (i < 0) return [...list, s]
  const next = list.slice()
  next[i] = s
  return next
}

/** The name the owner knows, or the code. */
export function badgeLabel(c: Connected): string {
  return c.name?.trim() || c.peer_id
}

export const sessions = writable<Connected[]>([])
