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

export const RECONFIRM_CHOICES_MIN = [20, 45, 80] as const

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

/** Inverse of `policyKey`; anything unknown is AskAlways (the safe default). */
export function policyFromKey(key: string): Policy {
  if (key === 'stay') return { kind: 'StayConnected' }
  const m = /^r(\d+)$/.exec(key)
  if (m && Number(m[1]) > 0) return { kind: 'Reconfirm', every_min: Number(m[1]) }
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

/** Selector options; a stored non-standard Reconfirm period is kept as an extra option. */
export function policyOptions(current?: Policy | null): { key: string; label: string }[] {
  const list: Policy[] = [
    { kind: 'AskAlways' },
    { kind: 'StayConnected' },
    ...RECONFIRM_CHOICES_MIN.map((every_min) => ({ kind: 'Reconfirm' as const, every_min })),
  ]
  if (current && !list.some((p) => policyKey(p) === policyKey(current))) list.push(current)
  return list.map((p) => ({ key: policyKey(p), label: policyLabel(p) }))
}

/** Applies a presence update to the list (ids not in the update keep their dot). */
export function applyPresence(list: Contact[], update: Presence[]): Contact[] {
  const m = new Map(update.map((p) => [p.peer_id, p.online]))
  return list.map((c) => (m.has(c.peer_id) ? { ...c, online: m.get(c.peer_id)! } : c))
}
