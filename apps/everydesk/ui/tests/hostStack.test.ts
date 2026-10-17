import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { STACK_STEP_PX, stackOffset } from '../src/lib/cards/cards.ts'
import { addCard, placement, removeCard } from '../src/lib/host/hostStack.ts'

test('I1: host cards stack in arrival order, each one stackOffset further in', () => {
  let l = addCard([], 'elevation')
  l = addCard(l, 'voice-1')
  l = addCard(l, 'voice-2')
  assert.deepEqual(l, ['elevation', 'voice-1', 'voice-2'])
  assert.deepEqual(placement(l, 'elevation'), { corner: 0, inset: 0 })
  assert.deepEqual(placement(l, 'voice-1'), { corner: 1, inset: STACK_STEP_PX })
  assert.deepEqual(placement(l, 'voice-2'), { corner: 2, inset: stackOffset(2) })
  // two cards never share an offset
  const insets = l.map((k) => placement(l, k)!.inset)
  assert.equal(new Set(insets).size, insets.length)
  // the same card twice stays one entry
  assert.deepEqual(addCard(l, 'voice-1'), l)
  // one goes: the rest close up, keeping their order
  l = removeCard(l, 'elevation')
  assert.deepEqual(placement(l, 'voice-1'), { corner: 0, inset: 0 })
  assert.deepEqual(placement(l, 'voice-2'), { corner: 1, inset: STACK_STEP_PX })
  assert.equal(placement(l, 'elevation'), null)
})

test('I1: the voice and elevation cards both render through HostStack', () => {
  for (const f of ['VoiceCard', 'ElevationCard']) {
    const src = readFileSync(new URL(`../src/lib/host/${f}.svelte`, import.meta.url), 'utf8')
    assert.match(src, /use:hostCard=/, f)
    assert.doesNotMatch(src, /position: fixed/, f)
  }
  const app = readFileSync(new URL('../src/App.svelte', import.meta.url), 'utf8')
  assert.match(app, /<HostStack \/>/)
})
