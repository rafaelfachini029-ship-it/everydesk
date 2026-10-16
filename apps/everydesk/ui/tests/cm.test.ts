// Run with `pnpm test` (Node's built-in runner; Node >= 23 strips the TS types).
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { CLICK_APPROVE_TIMEOUT_SECS, remainingSeconds } from '../src/lib/cm.ts'

test('accept window is 60 s', () => {
  assert.equal(CLICK_APPROVE_TIMEOUT_SECS, 60)
})

test('remainingSeconds counts down from 60 to 0', () => {
  const t0 = 1_000_000
  assert.equal(remainingSeconds(t0, t0), 60)
  assert.equal(remainingSeconds(t0, t0 + 1), 60)
  assert.equal(remainingSeconds(t0, t0 + 1000), 59)
  assert.equal(remainingSeconds(t0, t0 + 59_001), 1)
  assert.equal(remainingSeconds(t0, t0 + 60_000), 0)
  assert.equal(remainingSeconds(t0, t0 + 90_000), 0)
})

test('remainingSeconds clamps a clock that went backwards', () => {
  assert.equal(remainingSeconds(5000, 0), 60)
  assert.equal(remainingSeconds(0, 3000, 10), 7)
})

test('identity notice, remember gating and default', async () => {
  const { identityNotice, canRemember, rememberByDefault } = await import('../src/lib/cm.ts')
  const id = (state: 'verified' | 'unverified' | 'key_changed', known: boolean) => ({
    conn_id: 1,
    state,
    known,
  })
  const offer = { conn_id: 1, peer_id: 'a', name: 'Marta', key_changed: false }
  assert.equal(identityNotice(id('verified', true), 'Marta'), null)
  assert.deepEqual(identityNotice(id('unverified', false), 'Marta'), {
    level: 'info',
    text: 'Identidade não verificada',
  })
  assert.deepEqual(identityNotice(undefined, 'Marta')?.level, 'info')
  const loud = { level: 'loud', text: 'Atenção: alguém diz ser Marta, mas a chave não confere' }
  assert.deepEqual(identityNotice(id('unverified', true), 'Marta'), loud)
  assert.deepEqual(identityNotice(id('key_changed', true), 'Marta'), loud)
  // Never "lembrar" without a verified identity.
  assert.equal(canRemember(id('unverified', true), offer), false)
  assert.equal(canRemember(undefined, offer), false)
  assert.equal(canRemember(id('verified', false), undefined), false)
  assert.equal(canRemember(id('verified', false), offer), true)
  assert.equal(canRemember(id('key_changed', true), { ...offer, key_changed: true }), true)
  // Accept pairs by default, except over a key change.
  assert.equal(rememberByDefault(offer), true)
  assert.equal(rememberByDefault({ ...offer, key_changed: true }), false)
})
