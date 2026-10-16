// Run with `pnpm test` (Node's built-in runner; Node >= 23 strips the TS types).
import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  clientToDisplay,
  domButton,
  keyName,
  mouseMask,
  parseFrameHeader,
  wheelSteps,
} from '../src/lib/session.ts'

test('mouseMask matches RustDesk (button << 3) | event', () => {
  assert.equal(mouseMask(0, 0), 0)
  assert.equal(mouseMask(1, 1), 9)
  assert.equal(mouseMask(1, 2), 10)
  assert.equal(mouseMask(2, 1), 17)
  assert.equal(mouseMask(4, 2), 34)
  assert.equal(mouseMask(0, 3), 3)
  assert.deepEqual([0, 1, 2, 3].map(domButton), [1, 4, 2, 0])
})

test('parseFrameHeader reads 4 little-endian u32', () => {
  const buf = new ArrayBuffer(16 + 2 * 2 * 4)
  const v = new DataView(buf)
  ;[1, 2, 2, 8].forEach((n, i) => v.setUint32(i * 4, n, true))
  assert.deepEqual(parseFrameHeader(buf), { display: 1, width: 2, height: 2, stride: 8 })
  assert.equal(parseFrameHeader(buf.slice(0, 20)), null) // truncated pixels
  assert.equal(parseFrameHeader(new ArrayBuffer(8)), null)
})

test('clientToDisplay: letterboxed contain scaling + display origin', () => {
  // 1920x1080 frame in an 800x600 box at (100,50): scale 800/1920, 75px bars top/bottom.
  const box = { left: 100, top: 50, width: 800, height: 600 }
  const disp = { x: 1920, y: 0, w: 1920, h: 1080 } // second monitor
  assert.deepEqual(clientToDisplay(100, 125, box, 1920, 1080, disp), { x: 1920, y: 0 })
  assert.deepEqual(clientToDisplay(500, 350, box, 1920, 1080, disp), { x: 1920 + 960, y: 540 })
  assert.equal(clientToDisplay(500, 100, box, 1920, 1080, disp), null) // in the bar
  assert.equal(clientToDisplay(900, 350, box, 1920, 1080, disp), null) // right edge
  // Peer scales: frame 2x the logical display size.
  const hidpi = { x: -1440, y: 0, w: 1440, h: 900 }
  const b2 = { left: 0, top: 0, width: 2880, height: 1800 }
  assert.deepEqual(clientToDisplay(2000, 1000, b2, 2880, 1800, hidpi), { x: -440, y: 500 })
  // Unknown display: frame coordinates.
  assert.deepEqual(clientToDisplay(10, 20, b2, 2880, 1800, undefined), { x: 10, y: 20 })
})

test('wheelSteps: one step, dominant axis, Flutter sign', () => {
  assert.deepEqual(wheelSteps(0, 120), { x: 0, y: -1 })
  assert.deepEqual(wheelSteps(0, -3), { x: 0, y: 1 })
  assert.deepEqual(wheelSteps(-50, 10), { x: 1, y: 0 })
})

test('keyName maps codes to input_key names', () => {
  assert.equal(keyName('Enter', 'Enter'), 'VK_RETURN')
  assert.equal(keyName('Backspace', 'Backspace'), 'VK_BACK')
  assert.equal(keyName('KeyA', 'A'), 'VK_A')
  assert.equal(keyName('Digit7', '&'), 'VK_7')
  assert.equal(keyName('Numpad3', '3'), 'VK_NUMPAD3')
  assert.equal(keyName('F12', 'F12'), 'VK_F12')
  assert.equal(keyName('ShiftRight', 'Shift'), 'RShift')
  assert.equal(keyName('Period', '.'), '.')
  assert.equal(keyName('IntlRo', 'ç'), 'ç')
  assert.equal(keyName('Fn', 'Fn'), null)
})

test('accumulateWheel: trackpad bursts become ~1 step per 100 px', async () => {
  const { accumulateWheel } = await import('../src/lib/session.ts')
  const acc = { x: 0, y: 0 }
  let total = 0
  for (let i = 0; i < 25; i++) total += accumulateWheel(acc, 0, 10).y // 250 px down
  assert.equal(total, -2) // same sign as wheelSteps(0, +)
  assert.equal(acc.y, 50)
  // Reversing direction discards the leftover instead of cancelling the new motion.
  assert.deepEqual(accumulateWheel(acc, 0, -100), { x: 0, y: 1 })
  // Classic mouse notch in pixel mode (Chrome reports 100) and in line mode (3 lines).
  assert.deepEqual(accumulateWheel({ x: 0, y: 0 }, 0, 100), { x: 0, y: -1 })
  assert.deepEqual(accumulateWheel({ x: 0, y: 0 }, 0, 3, 1), { x: 0, y: -1 })
  assert.deepEqual(accumulateWheel({ x: 0, y: 0 }, -120, 0), { x: 1, y: 0 })
  assert.deepEqual(accumulateWheel({ x: 0, y: 0 }, 0, 4), { x: 0, y: 0 })
})

test('normalizeCode mirrors memorable::normalize', async () => {
  const { normalizeCode } = await import('../src/lib/session.ts')
  assert.equal(normalizeCode('Água Lua 42'), 'agua-lua-42')
  assert.equal(normalizeCode('  vela_rio   37 '), 'vela-rio-37')
  assert.equal(normalizeCode('VELA-RIO-37'), 'vela-rio-37')
  assert.equal(normalizeCode('CORAÇÃO pé 37'), 'coracao-pe-37')
  assert.equal(normalizeCode(' 259402742 '), '259402742')
  assert.equal(normalizeCode('   '), '')
})
