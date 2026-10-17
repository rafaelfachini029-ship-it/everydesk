import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  TRAIL_FILL_MS,
  TRAIL_NOTE_MS,
  dragDepth,
  dropTarget,
  hasFiles,
  overlayActive,
  trailPhase,
  type DragStep,
} from '../src/lib/session/dropzone.ts'

const run = (steps: DragStep[]) => steps.reduce(dragDepth, 0)

test('nested children: enter child before leaving parent never drops to 0', () => {
  // window → canvas → veil child → back to canvas
  const steps: DragStep[] = ['enter', 'enter', 'leave', 'enter', 'leave']
  let d = 0
  for (const s of steps) {
    d = dragDepth(d, s)
    assert.ok(overlayActive(d), `still over the window after ${s}`)
  }
  assert.equal(d, 1)
})

test('leaving the window through every level ends the drag', () => {
  assert.equal(run(['enter', 'enter', 'enter', 'leave', 'leave', 'leave']), 0)
  assert.equal(overlayActive(run(['enter', 'leave'])), false)
})

test('drop and reset end the drag at any depth', () => {
  assert.equal(run(['enter', 'enter', 'drop']), 0)
  assert.equal(run(['enter', 'enter', 'enter', 'reset']), 0)
})

test('a stray leave never goes negative (and a later enter shows again)', () => {
  assert.equal(run(['leave', 'leave']), 0)
  assert.equal(overlayActive(run(['leave', 'enter'])), true)
})

test('idle: overlay inactive', () => {
  assert.equal(overlayActive(0), false)
})

test('hasFiles: only drags carrying files', () => {
  assert.equal(hasFiles({ types: ['Files'] }), true)
  assert.equal(hasFiles({ types: ['text/uri-list', 'Files'] }), true)
  assert.equal(hasFiles({ types: ['text/plain'] }), false)
  assert.equal(hasFiles({ types: [] }), false)
  assert.equal(hasFiles({ types: null }), false)
  assert.equal(hasFiles(null), false)
  assert.equal(hasFiles(undefined), false)
  // DOMStringList-like
  assert.equal(hasFiles({ types: { length: 1, 0: 'Files' } as ArrayLike<string> }), true)
})

test('dropTarget names the contact, or the code', () => {
  assert.equal(dropTarget('Mãe', 'agua-lua-7'), 'Solte para enviar para Mãe')
  assert.equal(dropTarget(null, 'agua-lua-7'), 'Solte para enviar para agua-lua-7')
  assert.equal(dropTarget('  ', 'agua-lua-7'), 'Solte para enviar para agua-lua-7')
})

test('trail: fills, holds the note, then goes idle', () => {
  assert.equal(trailPhase(-1), 'idle')
  assert.equal(trailPhase(0), 'fill')
  assert.equal(trailPhase(TRAIL_FILL_MS - 1), 'fill')
  assert.equal(trailPhase(TRAIL_FILL_MS), 'note')
  assert.equal(trailPhase(TRAIL_FILL_MS + TRAIL_NOTE_MS - 1), 'note')
  assert.equal(trailPhase(TRAIL_FILL_MS + TRAIL_NOTE_MS), 'idle')
})
