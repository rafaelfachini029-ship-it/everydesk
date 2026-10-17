// Moment 2 (connecting): pure helpers, no DOM/Tauri, so `node --test` can load them.
//  - humanizeMsgbox: the engine's msgbox text → one plain Portuguese sentence;
//  - phaseFor: the session events seen so far → the phase shown on screen;
//  - REACH / traceOptions: how far the light trace goes per phase, and its timing
//    (through motion.ts, so "reduzir movimento" turns it into an instant change).

import { DUR, EASE, motionOptions } from '../../design/motion.ts'
import type { MsgBox } from '../session.ts'

export interface Humanized {
  title: string
  body: string
  /** Show "Tentar de novo". */
  retryable: boolean
  /** The raw engine text, only for messages we do not recognise (quiet disclosure). */
  detail?: string
}

const NOT_FOUND = 'Não encontrei essa máquina. Confira o código.'
const ASK_PASSWORD = 'Essa máquina pede uma senha.'
export const UNKNOWN = 'Não deu certo. Tente de novo.'

const say = (title: string, body = '', retryable = true): Humanized => ({ title, body, retryable })

/** `kind`/`text` of a `session:msgbox` (the English title adds nothing and is ignored). */
export function humanizeMsgbox(kind: string, text: string): Humanized {
  if (kind === 'input-password') return say(ASK_PASSWORD, '', false)
  if (kind === 're-input-password') return say(ASK_PASSWORD, 'Essa senha não confere. Digite de novo.', false)

  const t = text.trim()
  const l = t.toLowerCase()
  // The server rejected our clock: the only case where the clock is named. Its text is
  // already the sentence the person needs, split into title and instruction.
  if (l.includes('o relógio deste computador está errado'))
    return say('O relógio deste computador está errado.', 'Acerte a data e a hora e tente de novo.')
  if (l.includes('pedido não respondido'))
    return say('A outra máquina não respondeu.', 'Ninguém aceitou o pedido do outro lado.')
  if (l.includes('reconfirmação não respondida')) return say('O dono da máquina não confirmou a tempo.')
  if (l.includes('encerrada pelo dono')) return say('O dono da máquina encerrou a conexão.', '', false)
  if (l.includes('closed manually by the peer')) return say('A outra máquina recusou.')
  if (l.includes('aguardando o aplicativo everydesk'))
    return say('A outra máquina ainda está abrindo o EveryDesk.', 'Espere alguns segundos.')
  // No reply from the server or the peer. The engine appends a clock hint to these; a
  // plain not-found never mentions the clock.
  if (/rendezvous|timeout|timed out|does not exist|não existe/.test(l)) return say(NOT_FOUND)
  if (/\boffline\b/.test(l)) return say('A outra máquina está desligada ou sem internet agora.')
  return { ...say(UNKNOWN), detail: t || undefined }
}

// ---- phases ---------------------------------------------------------------------------

export type ConnEvent =
  | { type: 'msgbox'; box: MsgBox }
  | { type: 'displays' }
  | { type: 'connected' }
  | { type: 'frame' }
  | { type: 'login' }
  | { type: 'closed'; box: MsgBox | null }

export type PhaseName = 'procurando' | 'pedindo' | 'senha' | 'conectando' | 'pronto' | 'erro'

export interface Phase {
  name: PhaseName
  /** For `senha` and `erro`. */
  message?: Humanized
}

const isError = (kind: string) => kind.includes('error')
const boxText = (b: MsgBox) => b.text || b.title

/** Folds the events of one attempt into what the screen says. A frame wins over
 *  everything (the session view owns any later message); before it, the first error ends
 *  the attempt; otherwise the furthest step reached shows. */
export function phaseFor(events: readonly ConnEvent[]): Phase {
  let asked = false
  let connected = false
  let password: Humanized | null = null
  let error: Humanized | null = null
  for (const e of events) {
    switch (e.type) {
      case 'frame':
        return { name: 'pronto' }
      case 'displays':
      case 'connected':
        connected = true
        password = null
        break
      case 'login':
        password = null
        connected = true
        break
      case 'closed':
        error ??= e.box && isError(e.box.kind) ? humanizeMsgbox(e.box.kind, boxText(e.box)) : say(UNKNOWN)
        break
      case 'msgbox': {
        const k = e.box.kind
        if (k === 'input-password' || k === 're-input-password') {
          password = humanizeMsgbox(k, e.box.text)
          connected = false
        } else if (k.startsWith('wait-remote-accept')) asked = true
        else if (k === 'success') connected = true
        else if (isError(k)) error ??= humanizeMsgbox(k, boxText(e.box))
      }
    }
  }
  // A frame after an error is impossible in practice; the loop already returned for it.
  if (error) return { name: 'erro', message: error }
  if (password) return { name: 'senha', message: password }
  if (connected) return { name: 'conectando' }
  return { name: asked ? 'pedindo' : 'procurando' }
}

// ---- the light trace ------------------------------------------------------------------

/** Fraction of the path between the two cards that is lit. Searching lights only the
 *  start; once the other machine answers (asking, password, connecting) the light touches
 *  its card. The reach and the destination dot carry the state, so nothing is lost under
 *  reduced motion (only the travel is dropped). An error freezes the trace where it was. */
export const REACH: Record<Exclude<PhaseName, 'erro'>, number> = {
  procurando: 0.4,
  pedindo: 1,
  senha: 1,
  conectando: 1,
  pronto: 1,
}

/** The destination's dot lights up only once the session is actually opening. */
export const arrived = (p: PhaseName) => p === 'conectando' || p === 'pronto'

/** One pass of the spark along the lit part (a liveness cue in place of a spinner). */
export const SPARK_MS = 1400

export function traceOptions(reduced?: boolean) {
  return {
    reach: motionOptions({ duration: DUR.slow, easing: EASE.out }, reduced),
    spark: motionOptions({ duration: SPARK_MS, easing: 'cubic-bezier(0.4, 0, 0.2, 1)', iterations: Infinity, fill: 'none' }, reduced),
  }
}
