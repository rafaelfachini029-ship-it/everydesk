import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import {
  ALT_IDLE,
  ALT_TAP_MS,
  HIDE_MS,
  PILL_TOOLS,
  altTapReducer,
  createHideTimer,
  formatFps,
  formatLatency,
  isDeadZone,
  nearTopEdge,
  shouldForwardKey,
  shouldPeek,
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

test('keys reach the remote only from the revealed screen, never from the pill or an open message', () => {
  const all = [true, false]
  for (const canvasFocused of all)
    for (const pillFocused of all)
      for (const revealed of all)
        for (const overlayOpen of all)
          assert.equal(
            shouldForwardKey({ canvasFocused, pillFocused, revealed, overlayOpen }),
            canvasFocused && !pillFocused && revealed && !overlayOpen,
          )
  // no overlay field: same as closed
  assert.equal(shouldForwardKey({ canvasFocused: true, pillFocused: false, revealed: true }), true)
})

test('the pill holds only tools that work: no "em breve" tools in Stage 3', () => {
  assert.deepEqual([...PILL_TOOLS], ['monitores', 'qualidade', 'encerrar'])
  for (const gone of ['conversa', 'arquivos', 'gravar', 'voz']) assert.ok(!(PILL_TOOLS as readonly string[]).includes(gone), gone)
  const src = readFileSync(new URL('../src/lib/session/SessionPill.svelte', import.meta.url), 'utf8')
  assert.doesNotMatch(src, /em breve|SOON|icons\/(messages-square|folder|circle-dot|mic)'/)
  assert.match(src, />Encerrar</)
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

test('Alt+key combo: Alt down goes first, then the key; release forwards normally', () => {
  let r = altTapReducer(ALT_IDLE, { type: 'down', alt: true, code: 'AltLeft', repeat: false, t: 0 })
  assert.deepEqual([r.send, r.forward, r.summon], [[], false, false])
  r = altTapReducer(r.state, { type: 'down', alt: false, code: 'Tab', repeat: false, t: 50 })
  assert.deepEqual(r.send, [{ code: 'AltLeft', down: true }])
  assert.equal(r.forward, true)
  r = altTapReducer(r.state, { type: 'up', alt: false, t: 80 })
  assert.equal(r.forward, true)
  r = altTapReducer(r.state, { type: 'up', alt: true, t: 100 })
  assert.deepEqual([r.send, r.forward, r.summon, r.state], [[], true, false, ALT_IDLE])
})

test('bare Alt tap: nothing reaches the remote, the pill is summoned', () => {
  let r = altTapReducer(ALT_IDLE, { type: 'down', alt: true, code: 'AltRight', repeat: false, t: 1000 })
  r = altTapReducer(r.state, { type: 'down', alt: true, code: 'AltRight', repeat: true, t: 1100 })
  assert.deepEqual([r.send, r.forward], [[], false])
  r = altTapReducer(r.state, { type: 'up', alt: true, t: 1000 + ALT_TAP_MS - 1 })
  assert.deepEqual([r.send, r.forward, r.summon, r.state], [[], false, true, ALT_IDLE])
})

test('Alt held alone past the threshold still reaches the remote (down, up)', () => {
  let r = altTapReducer(ALT_IDLE, { type: 'down', alt: true, code: 'AltLeft', repeat: false, t: 0 })
  r = altTapReducer(r.state, { type: 'up', alt: true, t: ALT_TAP_MS })
  assert.deepEqual(r.send, [
    { code: 'AltLeft', down: true },
    { code: 'AltLeft', down: false },
  ])
  assert.deepEqual([r.forward, r.summon], [false, false])
  // with OS auto-repeat the down goes out as soon as the hold is long enough
  r = altTapReducer(ALT_IDLE, { type: 'down', alt: true, code: 'AltLeft', repeat: false, t: 0 })
  r = altTapReducer(r.state, { type: 'down', alt: true, code: 'AltLeft', repeat: true, t: ALT_TAP_MS })
  assert.deepEqual(r.send, [{ code: 'AltLeft', down: true }])
  r = altTapReducer(r.state, { type: 'up', alt: true, t: ALT_TAP_MS + 500 })
  assert.deepEqual([r.send, r.forward, r.summon], [[], true, false])
})

test('click or wheel with Alt commits it; reset drops a held-back Alt', () => {
  let r = altTapReducer(ALT_IDLE, { type: 'down', alt: true, code: 'AltLeft', repeat: false, t: 0 })
  r = altTapReducer(r.state, { type: 'interrupt' })
  assert.deepEqual(r.send, [{ code: 'AltLeft', down: true }])
  r = altTapReducer(r.state, { type: 'up', alt: true, t: 50 })
  assert.deepEqual([r.forward, r.summon], [true, false])
  r = altTapReducer(ALT_IDLE, { type: 'down', alt: true, code: 'AltLeft', repeat: false, t: 0 })
  r = altTapReducer(r.state, { type: 'reset' })
  assert.deepEqual([r.send, r.state], [[], ALT_IDLE])
  r = altTapReducer(r.state, { type: 'up', alt: true, t: 50 })
  assert.deepEqual([r.send, r.summon], [[], false])
  // outside a held-back Alt, other keys and interrupts pass untouched
  assert.equal(altTapReducer(ALT_IDLE, { type: 'down', alt: false, code: 'KeyA', repeat: false, t: 0 }).forward, true)
  assert.equal(altTapReducer(ALT_IDLE, { type: 'interrupt' }).send.length, 0)
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

test('the top edge peeks the pill, but never mid-drag (a held button or pointer capture)', () => {
  assert.equal(shouldPeek({ y: 0, held: false, captured: false }), true)
  assert.equal(shouldPeek({ y: 8, held: false, captured: false }), true)
  assert.equal(shouldPeek({ y: 9, held: false, captured: false }), false)
  assert.equal(shouldPeek({ y: 0, held: true, captured: false }), false)
  assert.equal(shouldPeek({ y: 0, held: false, captured: true }), false)
  assert.equal(shouldPeek({ y: 0, held: true, captured: true }), false)
  assert.equal(shouldPeek({ y: -3, held: false, captured: false }), false)
})
