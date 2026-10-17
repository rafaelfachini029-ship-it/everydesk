// Moment 4 cards (incoming request / reconfirm): pure helpers.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  INCOMING_LINE,
  FILES_LINE,
  RECONFIRM_LINE,
  REMIND_AT_SECS,
  STACK_STEP_PX,
  expire,
  mayTakeFocus,
  reminderAnnouncement,
  requestAnnouncement,
  ringAnnouncement,
  stackOrder,
  identityLine,
  initials,
  rememberDefault,
  stackOffset,
} from '../src/lib/cards/cards.ts'
import type { Identity } from '../src/lib/cm.ts'

const id = (state: Identity['state'], known = false): Identity => ({ conn_id: 1, state, known })

test('stackOffset: each card in the stack gets its own offset', () => {
  const seen = new Set<number>()
  for (let i = 0; i < 8; i++) seen.add(stackOffset(i))
  assert.equal(seen.size, 8)
  assert.equal(stackOffset(0), 0)
  assert.equal(stackOffset(1), STACK_STEP_PX)
  assert.ok(stackOffset(2) > stackOffset(1))
  // nonsense indices fall back to the front position
  assert.equal(stackOffset(-1), 0)
  assert.equal(stackOffset(Number.NaN), 0)
})

test('rememberDefault: on only for a verified identity without a key change', () => {
  assert.equal(rememberDefault(id('verified'), false), true)
  assert.equal(rememberDefault(id('verified', true), false), true)
  assert.equal(rememberDefault(id('verified'), true), false)
  assert.equal(rememberDefault(id('unverified'), false), false)
  assert.equal(rememberDefault(id('unverified', true), false), false)
  assert.equal(rememberDefault(id('key_changed', true), false), false)
  assert.equal(rememberDefault(id('key_changed', true), true), false)
  // identity not received yet counts as unverified
  assert.equal(rememberDefault(undefined, false), false)
})

test('identityLine: verified is quiet, unverified warns, a mismatched key is loud', () => {
  assert.deepEqual(identityLine(id('verified', true), 'Marta'), {
    tone: 'ok',
    text: 'Identidade verificada',
  })
  assert.deepEqual(identityLine(id('unverified'), 'Marta'), {
    tone: 'warn',
    text: 'Identidade não verificada',
  })
  assert.deepEqual(identityLine(undefined, 'Marta'), {
    tone: 'warn',
    text: 'Identidade não verificada',
  })
  const loud = { tone: 'danger', text: 'Atenção: alguém diz ser Marta, mas a chave não confere' }
  assert.deepEqual(identityLine(id('unverified', true), 'Marta'), loud)
  assert.deepEqual(identityLine(id('key_changed', true), 'Marta'), loud)
  assert.deepEqual(identityLine(id('key_changed', false), 'Marta'), loud)
})

test('copy: plain words, no "ID"', () => {
  assert.equal(INCOMING_LINE, 'quer ver e controlar a sua tela')
  assert.equal(RECONFIRM_LINE, 'continua conectado. Manter?')
  for (const s of [INCOMING_LINE, FILES_LINE, RECONFIRM_LINE]) assert.doesNotMatch(s, /\bID\b/)
})

test('initials', () => {
  assert.equal(initials('Marta Souza'), 'MS')
  assert.equal(initials('ana'), 'A')
  assert.equal(initials('  Notebook da Ana  '), 'NA')
  assert.equal(initials('lua-azul-sete'), 'LS')
  assert.equal(initials('Élio Ávila'), 'ÉÁ')
  assert.equal(initials(''), '')
})

test('expire: a card leaves when its countdown reaches 0', () => {
  const t0 = 1_000_000
  const cards = [
    { conn_id: 1, start: t0 }, // 60 s
    { conn_id: 2, start: t0 + 30_000 },
    { conn_id: 3, start: t0, ring_secs: 20 },
  ]
  assert.deepEqual(
    expire(cards, t0 + 19_999).map((c) => c.conn_id),
    [1, 2, 3],
  )
  assert.deepEqual(
    expire(cards, t0 + 20_000).map((c) => c.conn_id),
    [1, 2],
  )
  assert.deepEqual(
    expire(cards, t0 + 60_000).map((c) => c.conn_id),
    [2],
  )
  assert.deepEqual(expire(cards, t0 + 90_000), [])
})

// The --warn contrast lives with the other tokens now (tests/tokens.test.ts).

test('stackOrder: farthest card first in the DOM (Tab follows the eye), offsets from the corner', () => {
  const order = stackOrder(['oldest', 'middle', 'newest'])
  assert.deepEqual(
    order.map((o) => o.item),
    ['newest', 'middle', 'oldest'],
  )
  assert.deepEqual(
    order.map((o) => stackOffset(o.corner)),
    [2 * STACK_STEP_PX, STACK_STEP_PX, 0],
  )
  assert.deepEqual(stackOrder([]), [])
})

test('announcements say who, what and how long', () => {
  assert.equal(
    requestAnnouncement('Marta', false, 60),
    `Marta ${INCOMING_LINE}. Aceitar ou recusar: 60 segundos. F6 leva ao pedido.`,
  )
  assert.match(requestAnnouncement('Ana', true, 60), new RegExp(FILES_LINE))
  assert.equal(ringAnnouncement('Marta', 30), `Marta ${RECONFIRM_LINE} Manter ou encerrar: 30 segundos. F6 leva ao pedido.`)
  assert.equal(reminderAnnouncement('Marta', REMIND_AT_SECS), 'Faltam 20 segundos para responder a Marta.')
})

test('a new card takes the focus only when nobody is typing', () => {
  assert.equal(mayTakeFocus(null), true)
  assert.equal(mayTakeFocus({ tagName: 'BODY' }), true)
  assert.equal(mayTakeFocus({ tagName: 'BUTTON' }), true)
  for (const t of ['INPUT', 'textarea', 'SELECT', 'CANVAS']) assert.equal(mayTakeFocus({ tagName: t }), false, t)
  assert.equal(mayTakeFocus({ tagName: 'DIV', isContentEditable: true }), false)
})
