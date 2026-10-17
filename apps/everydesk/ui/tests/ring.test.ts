import assert from 'node:assert/strict'
import { test } from 'node:test'
import { ringDashOffset, ringFraction, ringGeometry } from '../src/lib/ui/ring.ts'

test('geometry: radius sits inside the stroke', () => {
  const g = ringGeometry(48, 4)
  assert.equal(g.r, 22)
  assert.equal(g.center, 24)
  assert.ok(Math.abs(g.c - 2 * Math.PI * 22) < 1e-9)
})

test('full time shows the whole arc, none shows nothing', () => {
  const { c } = ringGeometry(48, 4)
  assert.equal(ringDashOffset(60, 60, c), 0)
  assert.ok(Math.abs(ringDashOffset(60, 0, c) - c) < 1e-9)
  assert.ok(Math.abs(ringDashOffset(60, 30, c) - c / 2) < 1e-9)
  assert.ok(Math.abs(ringDashOffset(60, 45, c) - c / 4) < 1e-9)
})

test('fraction clamps bad input', () => {
  assert.equal(ringFraction(60, 90), 1)
  assert.equal(ringFraction(60, -5), 0)
  assert.equal(ringFraction(0, 10), 0)
  assert.equal(ringFraction(60, Number.NaN), 0)
})
