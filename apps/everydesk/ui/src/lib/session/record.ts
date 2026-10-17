/**
 * Session recording, both sides (spec §5.3, "Gravação da sessão com um clique, indicador
 * visível para os dois lados"). Pure state so `node --test` can exercise it; no DOM or Tauri
 * imports.
 *
 * Controller (RecordTool in the pill, RecordLamp in the collapsed pill): `session_record
 * {start}` starts / stops the engine's recorder; the engine answers with `session:record {on}`
 * (only on a change). The state follows the event, never the click: a host that does not
 * allow recording (`session:permission {name: "recording", value: false}`) makes the engine
 * drop the start without a word, so an optimistic "on" would lie.
 *   off --session:record on--> on (since = now) --session:record off--> off + "Abrir pasta"
 *   any --session:closed--> off, no offer (Review Focus 4: the session dropped while
 *   recording; the engine finalizes the file as its video thread ends)
 * Files: `recordings_dir` (engine's `video_save_directory`), `open_recordings_dir` opens it.
 *
 * Host (RecordMark on the ConnectedBadge): `cm:record {conn_id, on}`, hydrated from
 * `cm_pending().records`; the mark goes with the connection (`cm:removed`).
 */
import { writable } from 'svelte/store'

export const RECORD_EVENTS = { session: 'session:record', cm: 'cm:record', closed: 'session:closed' } as const

/** The `session:permission` name that takes recording away. */
export const RECORD_PERMISSION = 'recording'

export interface Recording {
  on: boolean
  /** When the recording started (ms, `Date.now()` clock); null while off. */
  since: number | null
  /** A recording just ended by itself or by a click: "Abrir pasta" is offered for a moment. */
  saved: boolean
  /** Said once (a command failed). */
  notice: string | null
}

export const RECORD_OFF: Recording = { on: false, since: null, saved: false, notice: null }

export type RecordAction =
  /** `session:record {on}`; `at` = when it arrived. */
  | { type: 'event'; on: boolean; at: number }
  /** `session:closed`: the session is gone, so is the recording. */
  | { type: 'closed' }
  /** A command failed (start, stop or opening the folder). */
  | { type: 'failed'; message: string }
  /** The "Abrir pasta" offer was used or timed out. */
  | { type: 'offered' }
  /** The notice was shown long enough. */
  | { type: 'seen' }
  /** A new session (or none). */
  | { type: 'reset' }

export function recordReducer(s: Recording, a: RecordAction): Recording {
  switch (a.type) {
    case 'event':
      if (a.on) return s.on ? s : { on: true, since: a.at, saved: false, notice: null }
      return s.on ? { on: false, since: null, saved: true, notice: null } : s
    case 'closed':
    case 'reset':
      return RECORD_OFF
    case 'failed':
      return { ...s, notice: a.message }
    case 'offered':
      return s.saved ? { ...s, saved: false } : s
    case 'seen':
      return s.notice === null ? s : { ...s, notice: null }
  }
}

/** Elapsed recording time as "00:12" (mm:ss), "1:02:03" past an hour. */
export function formatElapsed(ms: number): string {
  const t = Math.max(0, Math.floor(ms / 1000))
  const h = Math.floor(t / 3600)
  const m = Math.floor((t % 3600) / 60)
  const sec = t % 60
  const two = (n: number) => String(n).padStart(2, '0')
  return h > 0 ? `${h}:${two(m)}:${two(sec)}` : `${two(m)}:${two(sec)}`
}

/** The same time for screen readers: "12 segundos", "1 minuto e 5 segundos". */
export function spokenElapsed(ms: number): string {
  const t = Math.max(0, Math.floor(ms / 1000))
  const h = Math.floor(t / 3600)
  const m = Math.floor((t % 3600) / 60)
  const sec = t % 60
  const unit = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`
  const parts = [
    h ? unit(h, 'hora', 'horas') : '',
    m ? unit(m, 'minuto', 'minutos') : '',
    sec || (!h && !m) ? unit(sec, 'segundo', 'segundos') : '',
  ].filter(Boolean)
  return parts.length > 1 ? `${parts.slice(0, -1).join(', ')} e ${parts[parts.length - 1]}` : parts[0]
}

export const RECORD_DENIED = 'A outra máquina não permite gravar'

/** The tool's accessible name and hover hint. */
export function recordHint(on: boolean, denied: boolean): string {
  if (on) return 'Parar de gravar'
  return denied ? RECORD_DENIED : 'Gravar a sessão'
}

/** The collapsed pill's lamp, for screen readers. */
export function lampLabel(elapsedMs: number): string {
  return `Gravando há ${spokenElapsed(elapsedMs)}`
}

/** How long "Abrir pasta" stays after a recording ends. */
export const OFFER_MS = 8000

export const RECORD_SAVED = 'Gravação salva.'

/** The controller's recording, shared by the pill's tool and its lamp. */
export const recording = writable<Recording>(RECORD_OFF)
export const dispatchRecord = (a: RecordAction) => recording.update((s) => recordReducer(s, a))

// ---- host --------------------------------------------------------------------------------

export interface CmRecord {
  conn_id: number
  on: boolean
}

/** Whether `connId` is being recorded, from `cm_pending().records` (off when absent). */
export function hydrateRecord(list: readonly CmRecord[] | undefined, connId: number): boolean {
  return !!list?.some((r) => r.conn_id === connId && r.on)
}

export function hostRecordSay(who: string, on: boolean): string {
  return on ? `${who} está gravando esta sessão.` : `${who} parou de gravar.`
}
