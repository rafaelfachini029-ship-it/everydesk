import assert from 'node:assert/strict'
import { test } from 'node:test'
import { remainingSeconds } from '../src/lib/cm.ts'
import {
  applyPresence,
  isValidPeriod,
  policyFromKey,
  policyKey,
  policyLabel,
  policyOptions,
  type Contact,
  type Policy,
} from '../src/lib/policy.ts'

test('policy labels', () => {
  assert.equal(policyLabel({ kind: 'AskAlways' }), 'Sempre perguntar')
  assert.equal(policyLabel({ kind: 'StayConnected' }), 'Entrar direto')
  assert.equal(policyLabel({ kind: 'Reconfirm', every_min: 20 }), 'Confirmar a cada 20 min')
  assert.equal(policyLabel(null), '—')
})

test('standard options in order', () => {
  assert.deepEqual(
    policyOptions().map((o) => o.label),
    [
      'Sempre perguntar',
      'Entrar direto',
      'Confirmar a cada 20 min',
      'Confirmar a cada 45 min',
      'Confirmar a cada 80 min',
      'Outro…',
    ],
  )
  assert.deepEqual(
    policyOptions().map((o) => o.key),
    ['ask', 'stay', 'r20', 'r45', 'r80', 'other'],
  )
  // A stored custom period is listed so the select can show it; a preset is not repeated.
  assert.deepEqual(
    policyOptions({ kind: 'Reconfirm', every_min: 30 }).map((o) => o.key),
    ['ask', 'stay', 'r20', 'r45', 'r80', 'r30', 'other'],
  )
  assert.equal(policyOptions({ kind: 'Reconfirm', every_min: 45 }).length, 6)
})

test('policy key round-trip and wire shape', () => {
  const all: Policy[] = [
    { kind: 'AskAlways' },
    { kind: 'StayConnected' },
    { kind: 'Reconfirm', every_min: 80 },
  ]
  for (const p of all) assert.deepEqual(policyFromKey(policyKey(p)), p)
  assert.equal(JSON.stringify(policyFromKey('r20')), '{"kind":"Reconfirm","every_min":20}')
  assert.deepEqual(policyFromKey('bogus'), { kind: 'AskAlways' })
  assert.deepEqual(policyFromKey('r0'), { kind: 'AskAlways' })
  // Any whole number of minutes in 1..=1440 ("Outro…").
  assert.deepEqual(policyFromKey('r1'), { kind: 'Reconfirm', every_min: 1 })
  assert.deepEqual(policyFromKey('r30'), { kind: 'Reconfirm', every_min: 30 })
  assert.deepEqual(policyFromKey('r45'), { kind: 'Reconfirm', every_min: 45 })
  assert.deepEqual(policyFromKey('r1440'), { kind: 'Reconfirm', every_min: 1440 })
  for (const k of ['r1441', 'r9999', 'r01', 'r-5', 'r1.5', 'r', 'other']) {
    assert.deepEqual(policyFromKey(k), { kind: 'AskAlways' }, k)
  }
  assert.ok(isValidPeriod(1) && isValidPeriod(1440))
  assert.ok(!isValidPeriod(0) && !isValidPeriod(1441) && !isValidPeriod(2.5) && !isValidPeriod(NaN))
})

test('presence updates only listed ids', () => {
  const list: Contact[] = [
    { peer_id: 'a', name: 'A', policy: null, online: false },
    { peer_id: 'b', name: 'B', policy: { kind: 'AskAlways' }, online: true },
  ]
  const out = applyPresence(list, [{ peer_id: 'a', online: true }])
  assert.deepEqual(out.map((c) => c.online), [true, true])
})

test('reconfirm ring reuses remainingSeconds with ring_secs', () => {
  const t0 = 1_000_000
  assert.equal(remainingSeconds(t0, t0, 60), 60)
  assert.equal(remainingSeconds(t0, t0 + 59_500, 60), 1)
  assert.equal(remainingSeconds(t0, t0 + 60_000, 60), 0)
  assert.equal(remainingSeconds(t0, t0 + 10_000, 30), 20)
})
