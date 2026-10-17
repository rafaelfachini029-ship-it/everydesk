// Pure helpers behind GoField.svelte (tested in tests/GoField.test.ts).
import type { Contact } from '../policy.ts'
import { normalizeCode } from '../session.ts'

export const MAX_SUGGESTIONS = 6

/** Lowercase, accents dropped, spaces collapsed: "ÁGUA  Mãe" → "agua mae". */
export function fold(s: string): string {
  return s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/\s+/g, ' ')
    .trim()
}

/** Contacts whose code or name (any word of it) starts with what was typed; online ones
 *  first, at most MAX_SUGGESTIONS. An empty query suggests nothing. */
export function suggest(contacts: Contact[], query: string): Contact[] {
  const q = fold(query)
  if (!q) return []
  const code = normalizeCode(query)
  const hits = contacts.filter((c) => {
    if (c.peer_id.startsWith(code)) return true
    const name = fold(c.name)
    return name.startsWith(q) || name.split(' ').some((w) => w.startsWith(q))
  })
  return hits
    .map((c, i) => ({ c, i }))
    .sort((a, b) => Number(b.c.online) - Number(a.c.online) || a.i - b.i)
    .slice(0, MAX_SUGGESTIONS)
    .map(({ c }) => c)
}

/** Where Enter goes: a contact with exactly that code, else the only suggestion (so a
 *  name works), else the typed text as a code. "" when nothing was typed. */
export function resolveTarget(contacts: Contact[], query: string): string {
  const code = normalizeCode(query)
  if (!code) return ''
  if (contacts.some((c) => c.peer_id === code)) return code
  const s = suggest(contacts, query)
  return s.length === 1 ? s[0].peer_id : code
}

/** A session's last error message → one plain sentence saying what happened and what to do. */
export function humanError(text: string): string {
  const t = text.toLowerCase()
  if (/does not exist|not exist|não existe/.test(t))
    return 'Esse código não existe. Confira com quem está do outro lado.'
  if (/offline|desligad/.test(t))
    return 'A outra máquina está desligada ou sem internet agora.'
  if (/timeout|timed out|tempo/.test(t)) return 'A outra máquina não respondeu. Tente de novo em instantes.'
  if (/password|senha/.test(t)) return 'A senha não confere. Peça a senha de novo e tente outra vez.'
  if (/reject|recus|denied|negad/.test(t)) return 'Do outro lado, o pedido foi recusado.'
  return 'Não foi possível conectar. Confira o código e tente de novo.'
}
