import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  applyElevation,
  parseElevationSetting,
  type ElevationCardState,
  type ElevationStep,
} from '../src/lib/host/elevation.ts'

const ev = (conn_id: number, state: 'ask' | 'auto' | 'done'): ElevationStep => ({
  type: 'event',
  event: { conn_id, state },
})
const fold = (steps: ElevationStep[], from: ElevationCardState = null) => steps.reduce(applyElevation, from)

test('ask shows the card for that session; done hides it', () => {
  assert.equal(fold([ev(3, 'ask')]), 3)
  assert.equal(fold([ev(3, 'ask'), ev(3, 'done')]), null)
})

test('auto never shows the card (remembered "Permitir sempre")', () => {
  assert.equal(fold([ev(3, 'auto')]), null)
  assert.equal(fold([ev(3, 'ask'), ev(3, 'auto')]), null)
})

test('an answer hides the card', () => {
  assert.equal(fold([ev(3, 'ask'), { type: 'answered' }]), null)
})

test('the card goes with its session, not with another one', () => {
  assert.equal(fold([ev(3, 'ask'), { type: 'removed', conn_id: 4 }]), 3)
  assert.equal(fold([ev(3, 'ask'), { type: 'removed', conn_id: 3 }]), null)
})

test('hydration fills an empty card only; a live event wins', () => {
  assert.equal(fold([{ type: 'hydrate', conn_id: 5 }]), 5)
  assert.equal(fold([{ type: 'hydrate', conn_id: null }]), null)
  assert.equal(fold([ev(3, 'ask'), { type: 'hydrate', conn_id: 5 }]), 3)
})

test('done for an unseen episode is harmless', () => {
  assert.equal(fold([ev(9, 'done')]), null)
})

test('setting parses to Y, N or ask', () => {
  assert.equal(parseElevationSetting('Y'), 'Y')
  assert.equal(parseElevationSetting('N'), 'N')
  assert.equal(parseElevationSetting(''), '')
  assert.equal(parseElevationSetting('yes'), '')
  assert.equal(parseElevationSetting(undefined), '')
})
