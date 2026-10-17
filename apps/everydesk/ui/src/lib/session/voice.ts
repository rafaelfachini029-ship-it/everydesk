/**
 * Voice call, both sides (spec §5.3, "Voz com um clique"). Pure state so `node --test` can
 * exercise it; no DOM or Tauri imports.
 *
 * Controller (VoiceTool in the pill): `session_voice {start}` asks or hangs up; the engine
 * answers with `session:voice {state: "waiting" | "on" | "off" | "incoming", reason}`.
 *   off --click--> waiting --host accepts--> on --click / host hangs up--> off
 *   waiting --host refuses / click--> off
 * An `off` (or a failed command) returns to off in the same step, whatever the state was
 * (Review Focus 3: a refusal or a hang-up by the host reads "sem voz" at once). The engine
 * sends the same empty reason for a refusal and a hang-up by the host; which one it was
 * follows from the state it ends: from waiting it was refused, from on it was ended.
 * There is no "who is speaking" reading: the received audio is played inside the engine
 * and never reaches the app, so the pill shows on/off only.
 *
 * Host (VoiceCard on the ConnectedBadge): `cm:voice {conn_id, incoming, active}`, hydrated
 * from `cm_pending().voices`; `cm_voice_answer {connId, accept}`, `cm_voice_close {connId}`.
 */
import { writable } from 'svelte/store'

export const VOICE_EVENTS = { session: 'session:voice', cm: 'cm:voice' } as const

export type VoiceState = 'off' | 'waiting' | 'on'

export interface Voice {
  state: VoiceState
  /** Said once after a call ends without the person asking for it (refused, hung up). */
  notice: string | null
}

export const VOICE_OFF: Voice = { state: 'off', notice: null }

export const VOICE_NOTICES = {
  recusada: 'A outra máquina recusou a voz.',
  encerrada: 'Voz encerrada.',
} as const

/** The reason the engine gives when this side hung up (client/io_loop.rs). */
export const SELF_CLOSE_REASON = 'Closed manually by the peer'

export type VoiceEvent = { state: 'waiting' | 'on' | 'off' | 'incoming'; reason?: string | null }

export type VoiceAction =
  /** `session:voice`. */
  | { type: 'event'; event: VoiceEvent }
  /** The tool was clicked while off: the request goes out. */
  | { type: 'start' }
  /** The tool was clicked while waiting or on: cancel / hang up. */
  | { type: 'stop' }
  /** `session_voice` failed: nothing is ringing. */
  | { type: 'failed'; message: string }
  /** The notice was shown long enough. */
  | { type: 'seen' }
  /** A new session (or none). */
  | { type: 'reset' }

/** What an `off` from `prev` means for the person: refused, ended, or nothing to say (they
 *  hung up themselves, or there was no call). */
export function closeNotice(prev: VoiceState, reason: string | null | undefined): string | null {
  const r = (reason ?? '').trim()
  if (prev === 'off' || r === SELF_CLOSE_REASON) return null
  if (/recus|refus|denied|reject/i.test(r)) return VOICE_NOTICES.recusada
  if (prev === 'waiting' && !r) return VOICE_NOTICES.recusada
  return VOICE_NOTICES.encerrada
}

export function voiceReducer(s: Voice, a: VoiceAction): Voice {
  switch (a.type) {
    case 'start':
      return s.state === 'off' ? { state: 'waiting', notice: null } : s
    case 'stop':
      return VOICE_OFF
    case 'failed':
      return { state: 'off', notice: a.message }
    case 'seen':
      return s.notice === null ? s : { ...s, notice: null }
    case 'reset':
      return VOICE_OFF
    case 'event':
      switch (a.event.state) {
        case 'waiting':
          return { state: 'waiting', notice: null }
        case 'on':
          return { state: 'on', notice: null }
        case 'off':
          return { state: 'off', notice: closeNotice(s.state, a.event.reason) }
        default:
          // `incoming` is the host's side; the controller never gets it.
          return s
      }
  }
}

/** One click: what the tool does from `state`. */
export function clickIntent(state: VoiceState): { start: boolean } {
  return { start: state === 'off' }
}

/** The tool's accessible name (and its hover hint, unless `voiceHint` says otherwise). */
export function toolLabel(state: VoiceState): string {
  return state === 'off' ? 'Ligar a voz' : state === 'waiting' ? 'Cancelar a chamada de voz' : 'Encerrar a voz'
}

/** The collapsed pill's voice dot, for screen readers. */
export function dotLabel(state: VoiceState): string {
  return state === 'off' ? 'Voz desligada' : state === 'waiting' ? 'Chamando a voz' : 'Voz ligada'
}

export const VOICE_DENIED = 'A outra máquina não permite som'
export const VOICE_MUTED = 'Ligue “Som da outra máquina” em Mais para ouvir'

/** The hover hint: why it does nothing, or what to fix to hear, else the label. */
export function voiceHint(state: VoiceState, o: { denied: boolean; muted: boolean }): string {
  if (o.denied && state === 'off') return VOICE_DENIED
  if (o.muted && state === 'on') return VOICE_MUTED
  return toolLabel(state)
}

/** The controller's call, shared by the pill's tool and its voice dot. */
export const voice = writable<Voice>(VOICE_OFF)
export const dispatchVoice = (a: VoiceAction) => voice.update((s) => voiceReducer(s, a))

// ---- host --------------------------------------------------------------------------------

export interface CmVoice {
  conn_id: number
  incoming: boolean
  active: boolean
}

export type HostVoice = 'none' | 'incoming' | 'active'

export function hostVoice(v: Pick<CmVoice, 'incoming' | 'active'> | undefined): HostVoice {
  if (!v) return 'none'
  return v.active ? 'active' : v.incoming ? 'incoming' : 'none'
}

/** The voice state of one connection from `cm_pending().voices` (none when absent). */
export function hydrateVoice(list: readonly CmVoice[] | undefined, connId: number): HostVoice {
  return hostVoice(list?.find((v) => v.conn_id === connId))
}

export function voiceTitle(who: string): string {
  return `${who} quer conversar por voz`
}
export const VOICE_BODY = 'Se aceitar, o microfone desta máquina fica ligado até alguém encerrar a voz.'
