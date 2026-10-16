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
