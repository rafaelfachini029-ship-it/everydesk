/**
 * File-send jobs shown in the pill's trail (spec §5.2, §6.2 moment 5). A pure reducer over the
 * session events, so `node --test` exercises it without DOM or Tauri.
 *
 * Events: `session:job_added {id,name,to,total,preparing}` (right after the drop with
 * `preparing`, then again when the job starts), `session:job_failed {id,message}` (that file
 * was not sent), `session:jobs_dropped {ids}` (the drop was refused), `session:job
 * {id,file_num,kind,speed,finished,err}`, `session:override {id,file_num,to,is_upload,
 * is_identical}`, `session:jobs_cleared`, `session:permission {name,value}`.
 */

export const DENIED = 'A outra máquina não permite receber arquivos.'
/** A finished row stays this long, then fades. */
export const DONE_FADE_MS = 3000
/** A note (refused send) stays this long. */
export const NOTE_MS = 6000

/** `preparing`: the destination on the host is still being found. */
export type JobStatus = 'preparing' | 'sending' | 'ask' | 'done' | 'error'

export interface Job {
  id: number
  name: string
  total: number
  sent: number
  /** bytes/s, last reading */
  speed: number
  status: JobStatus
  err: string | null
  /** The file the host already has, while we ask "Substituir?". */
  ask: { fileNum: number } | null
  /** Files of this job left as they were on the host ("Não"). */
  kept: number
  /** When it ended (done or error), for the fade. */
  endedAt: number | null
}

export interface JobsState {
  jobs: Job[]
  note: string | null
  noteAt: number
}

export const EMPTY: JobsState = { jobs: [], note: null, noteAt: 0 }

export interface JobEvent {
  id: number
  file_num: number
  kind: 'progress' | 'done' | 'error'
  speed: number
  finished: number
  err: string | null
}
export interface OverrideEvent {
  id: number
  file_num: number
  to: string
  is_upload: boolean
  is_identical: boolean
}
export interface JobAdded {
  id: number
  name: string
  to: string
  total: number
  preparing?: boolean
}

export type JobsAction =
  | { type: 'added'; ev: JobAdded }
  | { type: 'job'; ev: JobEvent; at: number }
  | { type: 'override'; ev: OverrideEvent }
  | { type: 'answered'; id: number; overwrite: boolean }
  | { type: 'cancel'; id: number }
  | { type: 'failed'; id: number; message: string; at: number }
  | { type: 'dropped'; ids: number[] }
  | { type: 'cleared' }
  | { type: 'permission'; name: string; value: boolean }
  | { type: 'refused'; message: string; at: number }
  | { type: 'tick'; now: number }

function newJob(id: number, name = 'Arquivo', total = 0, status: JobStatus = 'sending'): Job {
  return { id, name, total, sent: 0, speed: 0, status, err: null, ask: null, kept: 0, endedAt: null }
}

const patch = (s: JobsState, id: number, f: (j: Job) => Job): JobsState => ({
  ...s,
  jobs: s.jobs.map((j) => (j.id === id ? f(j) : j)),
})

/** Host error text (English, from the engine) → what the person reads. */
export function friendlyError(err: string | null | undefined): string {
  const e = (err ?? '').toLowerCase()
  if (e.includes('permission') || e.includes('denied') || e.includes('access'))
    return 'A outra máquina não deixou gravar o arquivo.'
  if (e.includes('space') || e.includes('disk full')) return 'Falta espaço na outra máquina.'
  if (e.includes('not found') || e.includes('no such')) return 'O arquivo não foi encontrado aqui.'
  return 'O envio parou. Tente de novo.'
}

export function jobsReducer(s: JobsState, a: JobsAction): JobsState {
  switch (a.type) {
    case 'added': {
      const { id, name, total } = a.ev
      const preparing = a.ev.preparing === true
      if (s.jobs.some((j) => j.id === id))
        return patch(s, id, (j) => ({
          ...j,
          name,
          total: preparing ? j.total : total,
          status: j.status === 'preparing' && !preparing ? 'sending' : j.status,
        }))
      return { ...s, jobs: [...s.jobs, newJob(id, name, total, preparing ? 'preparing' : 'sending')] }
    }
    case 'job': {
      const ev = a.ev
      // id 0 / file -1 is the engine's answer to a folder lookup (where to put the files), not a job.
      if (ev.id <= 0) return s
      let st = s
      if (!st.jobs.some((j) => j.id === ev.id)) {
        if (ev.kind !== 'progress') return s // an ended job we never showed (or cancelled)
        st = { ...st, jobs: [...st.jobs, newJob(ev.id)] }
      }
      return patch(st, ev.id, (j) => {
        if (j.status === 'done' || j.status === 'error') return j
        switch (ev.kind) {
          case 'progress':
            // a pending question holds this job only: it stays asking until answered
            return {
              ...j,
              status: j.status === 'preparing' ? 'sending' : j.status,
              sent: Math.max(j.sent, ev.finished),
              speed: ev.speed,
            }
          case 'done':
            return { ...j, status: 'done', ask: null, speed: 0, endedAt: a.at }
          case 'error':
            return { ...j, status: 'error', ask: null, speed: 0, err: friendlyError(ev.err), endedAt: a.at }
        }
      })
    }
    case 'override': {
      const ev = a.ev
      if (!ev.is_upload || !s.jobs.some((j) => j.id === ev.id)) return s
      return patch(s, ev.id, (j) =>
        j.status === 'sending' ? { ...j, status: 'ask', ask: { fileNum: ev.file_num } } : j,
      )
    }
    case 'answered':
      return patch(s, a.id, (j) =>
        j.status === 'ask' ? { ...j, status: 'sending', ask: null, kept: j.kept + (a.overwrite ? 0 : 1) } : j,
      )
    case 'cancel':
      return { ...s, jobs: s.jobs.filter((j) => j.id !== a.id) }
    case 'failed': {
      // the app's own reason (already in Portuguese), shown as is
      const st = s.jobs.some((j) => j.id === a.id) ? s : { ...s, jobs: [...s.jobs, newJob(a.id)] }
      return patch(st, a.id, (j) => ({ ...j, status: 'error', ask: null, speed: 0, err: a.message, endedAt: a.at }))
    }
    case 'dropped':
      return { ...s, jobs: s.jobs.filter((j) => !a.ids.includes(j.id)) }
    case 'cleared':
      return { ...s, jobs: [] }
    case 'permission':
      if (a.name !== 'file') return s
      if (!a.value) return { ...s, note: DENIED, noteAt: Number.POSITIVE_INFINITY }
      return s.note === DENIED ? { ...s, note: null } : s
    case 'refused':
      return { ...s, note: a.message, noteAt: a.at }
    case 'tick': {
      const jobs = s.jobs.filter((j) => j.status !== 'done' || a.now - (j.endedAt ?? a.now) < DONE_FADE_MS)
      const keepNote = s.note !== null && a.now - s.noteAt < NOTE_MS
      if (jobs.length === s.jobs.length && (keepNote || s.note === null)) return s
      return { ...s, jobs, note: keepNote ? s.note : null }
    }
  }
}

/** 0..100, or null when the size is unknown. Never 100 before the host says done. */
export function percent(j: Pick<Job, 'sent' | 'total' | 'status'>): number | null {
  if (j.status === 'done') return 100
  if (j.total <= 0) return null
  return Math.min(99, Math.floor((j.sent / j.total) * 100))
}

/** Progress of everything still moving (the light along the pill), 0..1, or null when idle. */
export function overall(jobs: Job[]): number | null {
  const live = jobs.filter((j) => j.status === 'preparing' || j.status === 'sending' || j.status === 'ask')
  if (live.length === 0) return null
  const total = live.reduce((n, j) => n + j.total, 0)
  if (total <= 0) return 0
  return Math.min(1, live.reduce((n, j) => n + Math.min(j.sent, j.total), 0) / total)
}

/** "1,2 MB/s", "340 KB/s", "512 B/s"; empty when not moving. */
export function formatSpeed(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return ''
  if (bps < 1024) return `${Math.round(bps)} B/s`
  if (bps < 1024 * 1024) return `${Math.round(bps / 1024)} KB/s`
  const mb = bps / (1024 * 1024)
  return `${(mb < 100 ? mb.toFixed(1) : Math.round(mb).toString()).replace('.', ',')} MB/s`
}

/** The row's status line. */
export function jobLine(j: Job): string {
  switch (j.status) {
    case 'preparing':
      return 'Preparando…'
    case 'ask':
      return 'Já existe. Substituir?'
    case 'error':
      return j.err ?? 'O envio parou.'
    case 'done':
      return j.kept > 0 && j.sent === 0 ? 'Mantido o que já estava lá' : 'Enviado'
    case 'sending': {
      const p = percent(j)
      return [p === null ? '' : `${p} %`, formatSpeed(j.speed)].filter(Boolean).join(' · ')
    }
  }
}

/** Anything to show in the trail (keeps the pill up). */
export function trailBusy(s: JobsState): boolean {
  return s.jobs.length > 0 || s.note !== null
}
