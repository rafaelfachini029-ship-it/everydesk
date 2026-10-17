import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createIdleTimer } from '../src/lib/ui/pill.ts'

test('collapses after idleMs without interaction', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  let collapsed = 0
  const timer = createIdleTimer(2000, () => collapsed++)
  timer.poke()
  t.mock.timers.tick(1999)
  assert.equal(collapsed, 0)
  t.mock.timers.tick(1)
  assert.equal(collapsed, 1)
})

test('each interaction restarts the countdown', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  let collapsed = 0
  const timer = createIdleTimer(2000, () => collapsed++)
  timer.poke()
  t.mock.timers.tick(1500)
  timer.poke()
  t.mock.timers.tick(1500)
  assert.equal(collapsed, 0)
  t.mock.timers.tick(500)
  assert.equal(collapsed, 1)
})

test('held (pointer or focus inside) never collapses; release restarts', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  let collapsed = 0
  const timer = createIdleTimer(2000, () => collapsed++)
  timer.poke()
  timer.hold()
  timer.poke()
  t.mock.timers.tick(10_000)
  assert.equal(collapsed, 0)
  timer.release()
  t.mock.timers.tick(2000)
  assert.equal(collapsed, 1)
})

test('cancel stops a pending collapse', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  let collapsed = 0
  const timer = createIdleTimer(2000, () => collapsed++)
  timer.poke()
  timer.cancel()
  t.mock.timers.tick(5000)
  assert.equal(collapsed, 0)
})

test('a getter idleMs is read at each restart', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  let ms = 2000
  let collapsed = 0
  const timer = createIdleTimer(() => ms, () => collapsed++)
  ms = 500
  timer.poke()
  t.mock.timers.tick(500)
  assert.equal(collapsed, 1)
})
