import assert from 'node:assert/strict'
import { test } from 'node:test'
import { applyCm, badgeLabel, type CmStep } from '../src/lib/host/badge.ts'

const s = (conn_id: number, name = '', peer_id = `p${conn_id}`) => ({ conn_id, peer_id, name })
const fold = (steps: CmStep[]) => steps.reduce(applyCm, [] as ReturnType<typeof s>[])

test('connected minus removed, oldest first', () => {
  const list = fold([
    { type: 'connected', session: s(1, 'Mãe') },
    { type: 'connected', session: s(2, 'Escritório') },
    { type: 'connected', session: s(3) },
    { type: 'removed', conn_id: 2 },
  ])
  assert.deepEqual(list.map((c) => c.conn_id), [1, 3])
})

test('a repeated connected updates in place (no jump in the stack)', () => {
  const list = fold([
    { type: 'connected', session: s(1, 'Mãe') },
    { type: 'connected', session: s(2) },
    { type: 'connected', session: s(1, 'Mamãe') },
  ])
  assert.deepEqual(list.map((c) => c.conn_id), [1, 2])
  assert.equal(list[0].name, 'Mamãe')
})

test('removing an unknown id is a no-op; re-connecting after removal goes to the end', () => {
  assert.deepEqual(fold([{ type: 'removed', conn_id: 9 }]), [])
  const list = fold([
    { type: 'connected', session: s(1) },
    { type: 'connected', session: s(2) },
    { type: 'removed', conn_id: 1 },
    { type: 'connected', session: s(1) },
  ])
  assert.deepEqual(list.map((c) => c.conn_id), [2, 1])
})

test('applyCm never mutates its input', () => {
  const a = [s(1)]
  applyCm(a, { type: 'connected', session: s(1, 'x') })
  applyCm(a, { type: 'removed', conn_id: 1 })
  assert.deepEqual(a, [s(1)])
})

test('badgeLabel: name, else the code', () => {
  assert.equal(badgeLabel(s(1, 'Mãe', 'agua-lua-7')), 'Mãe')
  assert.equal(badgeLabel(s(1, ' ', 'agua-lua-7')), 'agua-lua-7')
})
