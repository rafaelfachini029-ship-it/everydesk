import assert from 'node:assert/strict'
import { test } from 'node:test'
import { CODE_MAX_PX, CODE_MIN_PX, codeFontSize, codeParts, serverStatus } from '../src/lib/home/bigcode.ts'

test('small window: code fits the width', () => {
  const px = codeFontSize(600, 400, 13)
  assert.ok(px >= CODE_MIN_PX && px < 80, `${px}`)
  // 13 mono glyphs at ~0.62em must fit within 600 minus gutters
  assert.ok(px * 13 * 0.62 <= 600 - 64)
})

test('huge window: capped at the maximum', () => {
  assert.equal(codeFontSize(2560, 1440, 13), CODE_MAX_PX)
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
