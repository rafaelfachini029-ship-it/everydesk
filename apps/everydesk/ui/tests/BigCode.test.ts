import assert from 'node:assert/strict'
import { test } from 'node:test'
import { CODE_CAP_PX, CODE_MAX_PX, CODE_MIN_PX, codeFontSize, codeParts, goWidth, serverStatus } from '../src/lib/home/bigcode.ts'

test('small window: code fits the width', () => {
  const px = codeFontSize(600, 400, 13)
  assert.ok(px >= CODE_MIN_PX && px < 80, `${px}`)
  // 13 mono glyphs at ~0.62em must fit within 600 minus gutters
  assert.ok(px * 13 * 0.62 <= 600 - 64)
})

test('huge window: the plate grows with it, up to the cap (~200px at 2560)', () => {
  assert.equal(codeFontSize(2560, 1440, 13), CODE_CAP_PX)
  assert.equal(codeFontSize(4000, 2200, 13), CODE_CAP_PX)
  const mid = codeFontSize(1920, 1080, 13)
  assert.ok(mid > CODE_MAX_PX && mid < CODE_CAP_PX, `${mid}`)
})

test('the field follows the code: 640px at least, the code width on big screens', () => {
  assert.equal(goWidth(900, 400), 640) // the code is narrower: the field keeps 640
  const w1280 = goWidth(1280, 800)
  assert.ok(w1280 > 640 && w1280 < 1280 - 64, `${w1280}`) // as wide as the plate
  const huge = goWidth(2560, 1440)
  assert.ok(huge > 1400 && huge <= 2560 - 64, `${huge}`)
  // a narrow window: never wider than the room
  assert.equal(goWidth(600, 800, 13, 536), 536)
})

test('tiny window: never below the minimum', () => {
  assert.equal(codeFontSize(200, 200, 22), CODE_MIN_PX)
})

test('short height limits the size too', () => {
  assert.ok(codeFontSize(1600, 300, 13) <= 300 * 0.2)
})

test('default arguments use a typical code', () => {
  assert.equal(codeFontSize(600), codeFontSize(600, 900, 13))
})

test('code splits into words for the plate', () => {
  assert.deepEqual(codeParts('azul-tigre-42'), ['azul', 'tigre', '42'])
  assert.deepEqual(codeParts('123456789'), ['123456789'])
  assert.deepEqual(codeParts(''), [])
})

test('server status in plain words', () => {
  assert.deepEqual(serverStatus('registrado'), { label: 'disponível', online: true })
  assert.deepEqual(serverStatus('aguardando autorização do servidor'), {
    label: 'aguardando autorização',
    online: false,
  })
  assert.deepEqual(serverStatus('sem conexão com o servidor'), { label: 'sem conexão', online: false })
  assert.deepEqual(serverStatus('conectando'), { label: 'conectando…', online: false })
  assert.deepEqual(serverStatus('erro: x'), { label: 'sem conexão', online: false })
})

test('1440 wide: capped, reads as a plate (under 75% of the width)', () => {
  const px = codeFontSize(1440, 900, 13)
  assert.equal(px, CODE_MAX_PX)
  assert.ok(px * 13 * 0.62 < 1440 * 0.75)
})
