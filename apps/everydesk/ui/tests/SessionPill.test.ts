import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  HIDE_MS,
  createAltTap,
  createHideTimer,
  formatFps,
  formatLatency,
  isDeadZone,
  nearTopEdge,
  shouldForwardKey,
} from '../src/lib/session/sessionPill.ts'
import { mergeQuality } from '../src/lib/session.ts'

test('latency reads "32 ms"', () => {
  assert.equal(formatLatency(32), '32 ms')
  assert.equal(formatLatency(31.6), '32 ms')
  assert.equal(formatLatency(0), '0 ms')
  assert.equal(formatLatency(-4), '0 ms')
  assert.equal(formatLatency(null), '— ms')
  assert.equal(formatLatency(Number.NaN), '— ms')
  assert.equal(formatFps(29.7), '30 fps')
  assert.equal(formatFps(null), null)
})

test('keys reach the remote only from the revealed screen, never from the pill', () => {
  const all = [true, false]
  for (const canvasFocused of all)
    for (const pillFocused of all)
      for (const revealed of all)
        assert.equal(
          shouldForwardKey({ canvasFocused, pillFocused, revealed }),
          canvasFocused && !pillFocused && revealed,
        )
})

test('dead zone is the pill rect; a hidden pill (null) swallows nothing', () => {
  const pill = { left: 500, top: 12, right: 780, bottom: 60 }
  assert.equal(isDeadZone(600, 30, pill), true)
  assert.equal(isDeadZone(500, 12, pill), true)
  assert.equal(isDeadZone(780, 30, pill), false)
  assert.equal(isDeadZone(600, 61, pill), false)
  assert.equal(isDeadZone(499, 30, pill), false)
  assert.equal(isDeadZone(600, 30, null), false)
})

test('top edge summons the pill', () => {
  assert.equal(nearTopEdge(0), true)
  assert.equal(nearTopEdge(8), true)
  assert.equal(nearTopEdge(9), false)
  assert.equal(nearTopEdge(-1), false)
})

test('Alt tap: alone counts, Alt+key or a held repeat-only press does not break it wrongly', () => {
  const t = createAltTap()
  t.down('Alt', false)
  assert.equal(t.up('Alt'), true)
  t.down('Alt', false)
  t.down('Alt', true)
  assert.equal(t.up('Alt'), true)
  t.down('Alt', false)
  t.down('Tab', false)
  assert.equal(t.up('Alt'), false)
  t.down('Alt', false)
  t.cancel()
  assert.equal(t.up('Alt'), false)
  assert.equal(t.up('Alt'), false)
})

test('pill hides HIDE_MS (2 s) after leaving; coming back to the top restarts it', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  assert.equal(HIDE_MS, 2000)
  let hidden = 0
  const timer = createHideTimer(() => hidden++)
  timer.poke()
  t.mock.timers.tick(1500)
  timer.poke() // pointer at the top edge again
  t.mock.timers.tick(1999)
  assert.equal(hidden, 0)
  t.mock.timers.tick(1)
  assert.equal(hidden, 1)
  timer.hold() // pointer over the pill
  t.mock.timers.tick(10_000)
  assert.equal(hidden, 1)
  timer.release()
  t.mock.timers.tick(2000)
  assert.equal(hidden, 2)
})

test('quality readings merge: missing fields keep the last value', () => {
  const a = mergeQuality({ fps: null, delay: null, speed: null }, { delay: 32 })
  assert.deepEqual(a, { fps: null, delay: 32, speed: null })
  const b = mergeQuality(a, { fps: 30, delay: null })
  assert.deepEqual(b, { fps: 30, delay: 32, speed: null })
})
