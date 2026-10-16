// Trust policy of a contact (Rust `PolicyView`, internally tagged by `kind`) and the
// labels of the utilitarian selectors. No DOM/Tauri imports, so `node --test` can run it.

export type Policy =
  | { kind: 'AskAlways' }
  | { kind: 'StayConnected' }
  | { kind: 'Reconfirm'; every_min: number }

/** `contacts_list` row. `policy` null: a contact we only control (not editable, "—"). */
export interface Contact {
  peer_id: string
  name: string
  policy: Policy | null
  online: boolean
}

/** `contacts:presence` payload item. */
export interface Presence {
  peer_id: string
  online: boolean
}

/** Presets of the selector; "Outro…" takes any whole number of minutes in
 *  RECONFIRM_MIN..=RECONFIRM_MAX (the backend enforces the same range). */
export const RECONFIRM_PRESETS_MIN = [20, 45, 80] as const
export const RECONFIRM_MIN = 1
export const RECONFIRM_MAX = 1440
/** `<option value>` of "Outro…" (not a policy: the UI then asks for the minutes). */
export const OTHER_KEY = 'other'

export function isValidPeriod(n: number): boolean {
  return Number.isInteger(n) && n >= RECONFIRM_MIN && n <= RECONFIRM_MAX
}

/** Stable `<option value>` for a policy. */
export function policyKey(p: Policy): string {
  switch (p.kind) {
    case 'AskAlways':
      return 'ask'
    case 'StayConnected':
      return 'stay'
    case 'Reconfirm':
      return `r${p.every_min}`
  }
}

/** Inverse of `policyKey`: `r<N>` for N in 1..=1440; anything else is AskAlways (the
 *  safe default; the backend also rejects other periods). */
export function policyFromKey(key: string): Policy {
  if (key === 'stay') return { kind: 'StayConnected' }
  const m = /^r([1-9][0-9]{0,3})$/.exec(key)
  if (m) {
    const every_min = Number(m[1])
    if (isValidPeriod(every_min)) return { kind: 'Reconfirm', every_min }
  }
  return { kind: 'AskAlways' }
}

export function policyLabel(p: Policy | null): string {
  if (!p) return '—'
  switch (p.kind) {
    case 'AskAlways':
      return 'Sempre perguntar'
    case 'StayConnected':
      return 'Entrar direto'
    case 'Reconfirm':
      return `Confirmar a cada ${p.every_min} min`
  }
}

/** Selector options: the presets, `current` when it is a custom period (so the select can
 *  show it), and "Outro…". */
export function policyOptions(current: Policy | null = null): { key: string; label: string }[] {
  const list: Policy[] = [
    { kind: 'AskAlways' },
    { kind: 'StayConnected' },
    ...RECONFIRM_PRESETS_MIN.map((every_min) => ({ kind: 'Reconfirm' as const, every_min })),
  ]
  if (current && !list.some((p) => policyKey(p) === policyKey(current))) list.push(current)
  return [
    ...list.map((p) => ({ key: policyKey(p), label: policyLabel(p) })),
    { key: OTHER_KEY, label: 'Outro…' },
  ]
}

/** Applies a presence update to the list (ids not in the update keep their dot). */
export function applyPresence(list: Contact[], update: Presence[]): Contact[] {
  const m = new Map(update.map((p) => [p.peer_id, p.online]))
  return list.map((c) => (m.has(c.peer_id) ? { ...c, online: m.get(c.peer_id)! } : c))
}
