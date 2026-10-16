// Accessed side (connection manager): event names, payloads and the pure countdown helper
// used by IncomingCard. No DOM/Tauri imports, so `node --test` can exercise it.

/** Matches `CLICK_APPROVE_TIMEOUT_SECS` in src/server/connection.rs. */
export const CLICK_APPROVE_TIMEOUT_SECS = 60

export const CM_EVENTS = {
  incoming: 'cm:incoming',
  connected: 'cm:connected',
  removed: 'cm:removed',
  pairOffer: 'cm:pair_offer',
  reconfirm: 'cm:reconfirm',
  identity: 'cm:identity',
} as const

export type IdentityState = 'verified' | 'unverified' | 'key_changed'

/** `cm:identity`: who is asking for the pending prompt `conn_id`. */
export interface Identity {
  conn_id: number
  state: IdentityState
  /** The claimed id is in one of the books (host pairings or contacts). */
  known: boolean
}

export interface IdentityNotice {
  /** 'loud': someone claims a known id without its key; 'info': just unverified. */
  level: 'loud' | 'info'
  text: string
}

/** What the card says about the identity; `null` for a verified one. No identity yet counts
 *  as unverified (the claim is all we have). */
export function identityNotice(id: Identity | undefined, name: string): IdentityNotice | null {
  const state = id?.state ?? 'unverified'
  if (state === 'verified') return null
  if (id?.known || state === 'key_changed') {
    return { level: 'loud', text: `Atenção: alguém diz ser ${name}, mas a chave não confere` }
  }
  return { level: 'info', text: 'Identidade não verificada' }
}

/** "Lembrar este contato" exists only for a pair offer whose identity is not unverified. */
export function canRemember(id: Identity | undefined, offer: PairOffer | undefined): boolean {
  return !!offer && !!id && id.state !== 'unverified'
}

/** Spec 4.4: accepting a verified offer pairs by default (policy "Sempre perguntar"); a key
 *  change never defaults to remembering. */
export function rememberByDefault(offer: PairOffer): boolean {
  return !offer.key_changed
}

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

/** `cm:pair_offer`: the pending prompt `conn_id` can be remembered as a contact. */
export interface PairOffer {
  conn_id: number
  peer_id: string
  name: string
  /** Already paired under another key: warn, and re-pin only with `replace_key`. */
  key_changed: boolean
}

/** `cm:reconfirm`: "still connected?" ring for an authorized session. */
export interface Reconfirm {
  conn_id: number
  ring_secs: number
}

export interface ReconfirmSnapshot extends Reconfirm {
  age_ms: number
}

/** `cm_pending` result: what the UI shows, for hydrating after mount. */
export interface CmSnapshot {
  pending: PendingSnapshot[]
  connected: Connected[]
  offers: PairOffer[]
  identities: Identity[]
  reconfirms: ReconfirmSnapshot[]
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
