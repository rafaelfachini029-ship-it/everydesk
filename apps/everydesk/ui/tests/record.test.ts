import assert from 'node:assert/strict'
import { test } from 'node:test'
import { get } from 'svelte/store'
import {
  RECORD_DENIED,
  RECORD_OFF,
  dispatchRecord,
  formatElapsed,
  hostRecordSay,
  hydrateRecord,
  lampLabel,
  recordHint,
  recordReducer,
  recording,
  spokenElapsed,
  type RecordAction,
  type Recording,
} from '../src/lib/session/record.ts'

const run = (actions: RecordAction[], s: Recording = RECORD_OFF) => actions.reduce(recordReducer, s)
const ev = (on: boolean, at = 1000): RecordAction => ({ type: 'event', on, at })

test('the event turns it on, with the time it started', () => {
  assert.deepEqual(run([ev(true, 5000)]), { on: true, since: 5000, saved: false, notice: null })
})

test('a repeated on keeps the first start time', () => {
  assert.equal(run([ev(true, 5000), ev(true, 9000)]).since, 5000)
})

test('stopping offers the folder', () => {
  const s = run([ev(true), ev(false)])
  assert.deepEqual(s, { on: false, since: null, saved: true, notice: null })
  assert.deepEqual(recordReducer(s, { type: 'offered' }), RECORD_OFF)
})

test('an off with nothing recording offers nothing', () => {
  assert.equal(run([ev(false)]), RECORD_OFF)
})

test('Review Focus 4: the session drops while recording → off, no folder offer', () => {
  const s = run([ev(true), { type: 'closed' }])
  assert.deepEqual(s, RECORD_OFF)
  // and while the offer is up
  assert.deepEqual(run([ev(true), ev(false), { type: 'closed' }]), RECORD_OFF)
})

test('a failure is said once and keeps the state the engine reported', () => {
  const s = run([ev(true), { type: 'failed', message: 'x' }])
  assert.equal(s.on, true)
  assert.equal(s.notice, 'x')
  assert.equal(recordReducer(s, { type: 'seen' }).notice, null)
})

test('a new recording drops a stale folder offer', () => {
  assert.equal(run([ev(true), ev(false), ev(true, 7)]).saved, false)
})

test('elapsed reads mm:ss, h:mm:ss past an hour', () => {
  assert.equal(formatElapsed(0), '00:00')
  assert.equal(formatElapsed(12_400), '00:12')
  assert.equal(formatElapsed(65_000), '01:05')
  assert.equal(formatElapsed(3_599_999), '59:59')
  assert.equal(formatElapsed(3_723_000), '1:02:03')
  assert.equal(formatElapsed(-50), '00:00')
})

test('elapsed for screen readers', () => {
  assert.equal(spokenElapsed(0), '0 segundos')
  assert.equal(spokenElapsed(1_000), '1 segundo')
  assert.equal(spokenElapsed(60_000), '1 minuto')
  assert.equal(spokenElapsed(65_000), '1 minuto e 5 segundos')
  assert.equal(spokenElapsed(3_723_000), '1 hora, 2 minutos e 3 segundos')
  assert.equal(lampLabel(12_000), 'Gravando há 12 segundos')
})

test('the hint says why it does nothing', () => {
  assert.equal(recordHint(false, false), 'Gravar a sessão')
  assert.equal(recordHint(false, true), RECORD_DENIED)
  assert.equal(recordHint(true, true), 'Parar de gravar')
})

test('the shared store follows the reducer', () => {
  dispatchRecord({ type: 'reset' })
  dispatchRecord(ev(true, 3))
  assert.equal(get(recording).on, true)
  dispatchRecord({ type: 'closed' })
  assert.deepEqual(get(recording), RECORD_OFF)
})

test('host: hydration per connection', () => {
  const list = [
    { conn_id: 1, on: true },
    { conn_id: 2, on: false },
  ]
  assert.equal(hydrateRecord(list, 1), true)
  assert.equal(hydrateRecord(list, 2), false)
  assert.equal(hydrateRecord(list, 3), false)
  assert.equal(hydrateRecord(undefined, 1), false)
  assert.equal(hostRecordSay('Mãe', true), 'Mãe está gravando esta sessão.')
})
