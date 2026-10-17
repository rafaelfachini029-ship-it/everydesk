// Moment 4 (spec 6.2): pure helpers of the request cards (IncomingCard, ReconfirmCard).
// No DOM/Tauri imports, so `node --test` can run them.
import { identityNotice, remainingSeconds, type Identity } from '../cm.ts'

export const INCOMING_LINE = 'quer ver e controlar a sua tela'
export const FILES_LINE = 'quer enviar ou receber arquivos'
export const RECONFIRM_LINE = 'continua conectado. Manter?'

/** Horizontal step between stacked cards: each card further from the corner sits one step
 *  further in, so two requests never line up as one. */
export const STACK_STEP_PX = 14

/** Inset (px, from the right edge) of the card at `index` (0 = the one nearest the corner). */
export function stackOffset(index: number): number {
  return Number.isInteger(index) && index > 0 ? index * STACK_STEP_PX : 0
}

/** "Lembrar este contato" starts ON only for a verified identity and no key change. */
export function rememberDefault(identity: Identity | undefined, keyChanged: boolean): boolean {
  return identity?.state === 'verified' && !keyChanged
}

export type Tone = 'ok' | 'warn' | 'danger'

/** The identity line of the card. Built on `identityNotice` (the Stage 2 wording). */
export function identityLine(identity: Identity | undefined, name: string): { tone: Tone; text: string } {
  const n = identityNotice(identity, name)
  if (!n) return { tone: 'ok', text: 'Identidade verificada' }
  return { tone: n.level === 'loud' ? 'danger' : 'warn', text: n.text }
}

/** One or two letters for the disc: first and last word ("lua-azul-sete" -> "LS"). */
export function initials(name: string): string {
  const words = name.trim().split(/[\s\-_.]+/u).filter(Boolean)
  if (words.length === 0) return ''
  const first = [...words[0]][0]
  const last = words.length > 1 ? [...words[words.length - 1]][0] : ''
  return (first + last).toLocaleUpperCase('pt-BR')
}

/** Drops the cards whose countdown reached 0 (`ring_secs` for a reconfirm, else 60 s). */
export function expire<T extends { start: number; ring_secs?: number }>(cards: T[], now: number): T[] {
  return cards.filter((c) => remainingSeconds(c.start, now, c.ring_secs) > 0)
}
