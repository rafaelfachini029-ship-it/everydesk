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

export const NO_CODE = 'Digite o código da outra máquina.'
export const AMBIGUOUS_NAME = 'Mais de um contato com esse nome. Use o código.'

/** Contacts whose name (any word of it) starts with what was typed. */
function nameHits(contacts: Contact[], query: string): Contact[] {
  const q = fold(query)
  if (!q) return []
  return contacts.filter((c) => {
    const name = fold(c.name)
    return name.startsWith(q) || name.split(' ').some((w) => w.startsWith(q))
  })
}

/** Where Enter goes: a contact with exactly that code, else the only suggestion (so a
 *  name works), else the typed text as a code. A name shared by two or more contacts is
 *  never sent as a code: it asks for the code instead. */
export function resolveTarget(contacts: Contact[], query: string): { id: string } | { error: string } {
  const code = normalizeCode(query)
  if (!code) return { error: NO_CODE }
  if (contacts.some((c) => c.peer_id === code)) return { id: code }
  const s = suggest(contacts, query)
  if (s.length === 1) return { id: s[0].peer_id }
  if (nameHits(contacts, query).length >= 2) return { error: AMBIGUOUS_NAME }
  return { id: code }
}
