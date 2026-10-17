import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import {
  DENIED,
  DONE_FADE_MS,
  EMPTY,
  NOTE_MS,
  formatSpeed,
  jobLine,
  jobsReducer,
  overall,
  percent,
  trailBusy,
  type JobEvent,
  type JobsAction,
  type JobsState,
} from '../src/lib/session/jobs.ts'

const run = (actions: JobsAction[], s: JobsState = EMPTY) => actions.reduce(jobsReducer, s)
const added = (id: number, name: string, total: number): JobsAction => ({ type: 'added', ev: { id, name, to: `/h/Desktop/${name}`, total } })
const ev = (id: number, kind: JobEvent['kind'], finished = 0, speed = 0, err: string | null = null, file_num = 0): JobsAction => ({
  type: 'job',
  ev: { id, file_num, kind, speed, finished, err },
  at: 1000,
})
const override = (id: number, file_num = 0): JobsAction => ({
  type: 'override',
  ev: { id, file_num, to: '/local/a.txt', is_upload: true, is_identical: false },
})
const job = (s: JobsState, id: number) => s.jobs.find((j) => j.id === id)!

test('progress → done', () => {
  let s = run([added(1, 'a.txt', 1000), ev(1, 'progress', 400, 2048)])
  assert.equal(job(s, 1).status, 'sending')
  assert.equal(percent(job(s, 1)), 40)
  assert.equal(jobLine(job(s, 1)), '40 % · 2 KB/s')
  s = jobsReducer(s, ev(1, 'progress', 1000, 2048))
  assert.equal(percent(job(s, 1)), 99, 'never 100 before the host says done')
  s = jobsReducer(s, ev(1, 'done'))
  assert.equal(job(s, 1).status, 'done')
  assert.equal(percent(job(s, 1)), 100)
  assert.equal(jobLine(job(s, 1)), 'Enviado')
  // a late progress never reopens it
  assert.equal(job(jobsReducer(s, ev(1, 'progress', 10, 1)), 1).status, 'done')
})

test('progress → error, with a message in Portuguese', () => {
  const s = run([added(2, 'b.pdf', 10), ev(2, 'progress', 5), ev(2, 'error', 0, 0, 'Permission denied')])
  assert.equal(job(s, 2).status, 'error')
  assert.equal(jobLine(job(s, 2)), 'A outra máquina não deixou gravar o arquivo.')
  assert.equal(job(jobsReducer(s, ev(2, 'done')), 2).status, 'error')
})

test('the folder lookup (job 0, file -1) is never a row', () => {
  const s = run([added(1, 'a', 1), ev(0, 'error', 0, 0, 'No such file', -1)])
  assert.deepEqual(s.jobs.map((j) => j.id), [1])
})

test('progress before job_added still shows a row; an ended unknown job does not', () => {
  let s = run([ev(5, 'progress', 10)])
  assert.equal(job(s, 5).name, 'Arquivo')
  s = jobsReducer(s, added(5, 'c.zip', 100))
  assert.equal(job(s, 5).name, 'c.zip')
  assert.equal(job(s, 5).sent, 10)
  assert.equal(run([ev(9, 'done')]).jobs.length, 0)
})

// Review Focus 2: the host already has the file → "Substituir?"; "Não" skips only that file
// and the job ends clean; the other job keeps moving.
test('override pending blocks only that job; Não keeps the host file and the job ends clean', () => {
  let s = run([added(1, 'a.txt', 100), added(2, 'b.txt', 100), override(1, 0)])
  assert.equal(job(s, 1).status, 'ask')
  assert.deepEqual(job(s, 1).ask, { fileNum: 0 })
  assert.equal(jobLine(job(s, 1)), 'Já existe. Substituir?')
  s = jobsReducer(s, ev(2, 'progress', 50, 1))
  assert.equal(job(s, 2).status, 'sending')
  assert.equal(job(s, 2).sent, 50)
  // a stray progress for the asking job does not drop the question
  s = jobsReducer(s, ev(1, 'progress', 0, 0))
  assert.equal(job(s, 1).status, 'ask')
  s = jobsReducer(s, { type: 'answered', id: 1, overwrite: false })
  assert.equal(job(s, 1).status, 'sending')
  assert.equal(job(s, 1).kept, 1)
  s = jobsReducer(s, ev(1, 'done'))
  assert.equal(job(s, 1).status, 'done')
  assert.equal(jobLine(job(s, 1)), 'Mantido o que já estava lá')
  assert.equal(job(s, 2).status, 'sending')
})

test('override Sim goes on sending; download overrides and unknown jobs are ignored', () => {
  let s = run([added(1, 'a.txt', 100), override(1), { type: 'answered', id: 1, overwrite: true }, ev(1, 'progress', 100)])
  assert.equal(job(s, 1).status, 'sending')
  assert.equal(job(s, 1).kept, 0)
  s = jobsReducer(s, { type: 'override', ev: { id: 1, file_num: 0, to: '', is_upload: false, is_identical: false } })
  assert.equal(job(s, 1).status, 'sending')
  assert.equal(jobsReducer(s, override(7)), s)
})

// Review Focus 1: the host turned file permission off → the note says so, nothing hangs.
test('permission off → inline note; a refused send says why; permission back clears it', () => {
  let s = jobsReducer(EMPTY, { type: 'permission', name: 'file', value: false })
  assert.equal(s.note, DENIED)
  assert.equal(DENIED, 'A outra máquina não permite receber arquivos.')
  assert.ok(trailBusy(s))
  // stays while the permission is off
  assert.equal(jobsReducer(s, { type: 'tick', now: 1e12 }).note, DENIED)
  assert.equal(jobsReducer(s, { type: 'permission', name: 'keyboard', value: true }), s)
  s = jobsReducer(s, { type: 'permission', name: 'file', value: true })
  assert.equal(s.note, null)
  // the send itself rejected (Rust returns the same text): a note that fades
  s = jobsReducer(s, { type: 'refused', message: DENIED, at: 1000 })
  assert.equal(s.note, DENIED)
  assert.equal(jobsReducer(s, { type: 'tick', now: 1000 + NOTE_MS - 1 }).note, DENIED)
  s = jobsReducer(s, { type: 'tick', now: 1000 + NOTE_MS })
  assert.equal(s.note, null)
  assert.equal(trailBusy(s), false)
})

test('the Rust refusal text matches the note', () => {
  const rs = readFileSync(new URL('../../src/files.rs', import.meta.url), 'utf8')
  assert.ok(rs.includes(`"${DENIED}"`))
})

test('done rows fade after 3 s; errors stay; cancel and jobs_cleared remove', () => {
  let s = run([added(1, 'a', 1), added(2, 'b', 1), added(3, 'c', 1), ev(1, 'done'), ev(2, 'error', 0, 0, 'x')])
  s = jobsReducer(s, { type: 'tick', now: 1000 + DONE_FADE_MS - 1 })
  assert.equal(s.jobs.length, 3)
  s = jobsReducer(s, { type: 'tick', now: 1000 + DONE_FADE_MS })
  assert.deepEqual(s.jobs.map((j) => j.id), [2, 3])
  s = jobsReducer(s, { type: 'cancel', id: 3 })
  assert.deepEqual(s.jobs.map((j) => j.id), [2])
  assert.equal(jobsReducer(s, { type: 'cleared' }).jobs.length, 0)
})

test('overall progress of what is still moving', () => {
  assert.equal(overall([]), null)
  const s = run([added(1, 'a', 100), added(2, 'b', 300), ev(1, 'progress', 100), ev(2, 'progress', 100)])
  assert.equal(overall(s.jobs), 0.5)
  assert.equal(overall(jobsReducer(s, ev(1, 'done')).jobs), 1 / 3)
})

test('speed reads "1,2 MB/s"', () => {
  assert.equal(formatSpeed(1.2 * 1024 * 1024), '1,2 MB/s')
  assert.equal(formatSpeed(340 * 1024), '340 KB/s')
  assert.equal(formatSpeed(512), '512 B/s')
  assert.equal(formatSpeed(0), '')
  assert.equal(formatSpeed(Number.NaN), '')
})
