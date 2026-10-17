import assert from 'node:assert/strict'
import { test } from 'node:test'
import { AMBIGUOUS_NAME, NO_CODE, fold, resolveTarget, suggest } from '../src/lib/home/go.ts'
import { normalizeCode } from '../src/lib/session.ts'
import type { Contact } from '../src/lib/policy.ts'
import { shouldCollapse } from '../src/lib/home/reveal.ts'

const c = (peer_id: string, name: string, online = false): Contact => ({
  peer_id,
  name,
  policy: null,
  online,
})

const book: Contact[] = [
  c('azul-tigre-42', 'Escritório'),
  c('agua-lua-7', 'Mãe', true),
  c('verde-onca-13', 'João Silva'),
  c('123456789', ''),
]

test('dictation normalizes to the code', () => {
  assert.equal(normalizeCode('Azul Tigre 42'), 'azul-tigre-42')
  assert.equal(normalizeCode('  Água   Lua_7 '), 'agua-lua-7')
})

test('fold drops case and accents', () => {
  assert.equal(fold('ÁGUA Mãe'), 'agua mae')
})

test('empty query suggests nothing', () => {
  assert.deepEqual(suggest(book, ''), [])
  assert.deepEqual(suggest(book, '   '), [])
})

test('prefix on code, dictated or typed', () => {
  assert.deepEqual(
    suggest(book, 'azul').map((x) => x.peer_id),
    ['azul-tigre-42'],
  )
  assert.deepEqual(
    suggest(book, 'Azul Tig').map((x) => x.peer_id),
    ['azul-tigre-42'],
  )
  assert.deepEqual(
    suggest(book, 'Água').map((x) => x.peer_id),
    ['agua-lua-7'],
  )
  assert.deepEqual(
    suggest(book, '1234').map((x) => x.peer_id),
    ['123456789'],
  )
})

test('prefix on name, case and accent insensitive, any word', () => {
  assert.deepEqual(suggest(book, 'mae').map((x) => x.peer_id), ['agua-lua-7'])
  assert.deepEqual(suggest(book, 'ESCRI').map((x) => x.peer_id), ['azul-tigre-42'])
  assert.deepEqual(suggest(book, 'silva').map((x) => x.peer_id), ['verde-onca-13'])
})

test('no infix match', () => {
  assert.deepEqual(suggest(book, 'tigre'), [])
  assert.deepEqual(suggest(book, 'ilva'), [])
})

test('online first, at most six', () => {
  const many = Array.from({ length: 9 }, (_, i) => c(`a-${i}`, `A ${i}`, i === 8))
  const out = suggest(many, 'a')
  assert.equal(out.length, 6)
  assert.equal(out[0].peer_id, 'a-8')
})

test('resolve: exact code, single suggestion, or the typed code', () => {
  assert.deepEqual(resolveTarget(book, 'Azul Tigre 42'), { id: 'azul-tigre-42' })
  assert.deepEqual(resolveTarget(book, 'mãe'), { id: 'agua-lua-7' })
  assert.deepEqual(resolveTarget(book, 'Roxo Gato 9'), { id: 'roxo-gato-9' })
  assert.deepEqual(resolveTarget(book, '   '), { error: NO_CODE })
})

test('resolve: a name shared by two or more contacts asks for the code', () => {
  const twins: Contact[] = [...book, c('roxo-gato-9', 'Mãe'), c('rosa-sapo-3', 'Ana Silva')]
  assert.deepEqual(resolveTarget(twins, 'mãe'), { error: AMBIGUOUS_NAME })
  assert.equal(AMBIGUOUS_NAME, 'Mais de um contato com esse nome. Use o código.')
  // "silva" matches João Silva and Ana Silva: never sent as the code "silva"
  assert.deepEqual(resolveTarget(twins, 'silva'), { error: AMBIGUOUS_NAME })
  // an exact code still wins, and an unknown code still goes as typed
  assert.deepEqual(resolveTarget(twins, 'roxo gato 9'), { id: 'roxo-gato-9' })
  assert.deepEqual(resolveTarget(twins, 'Lilás Boi 5'), { id: normalizeCode('Lilás Boi 5') })
})

test('cards stay open while focus or a pointer press is inside', () => {
  assert.equal(shouldCollapse({ focusWithin: true, pointerInside: false }), false)
  // WKWebView: clicking a card button blurs the field without focusing the button
  assert.equal(shouldCollapse({ focusWithin: false, pointerInside: true }), false)
  assert.equal(shouldCollapse({ focusWithin: true, pointerInside: true }), false)
  assert.equal(shouldCollapse({ focusWithin: false, pointerInside: false }), true)
})
