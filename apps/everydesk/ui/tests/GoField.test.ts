import assert from 'node:assert/strict'
import { test } from 'node:test'
import { fold, humanError, resolveTarget, suggest } from '../src/lib/home/go.ts'
import { normalizeCode } from '../src/lib/session.ts'
import type { Contact } from '../src/lib/policy.ts'

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
  assert.equal(resolveTarget(book, 'Azul Tigre 42'), 'azul-tigre-42')
  assert.equal(resolveTarget(book, 'mãe'), 'agua-lua-7')
  assert.equal(resolveTarget(book, 'Roxo Gato 9'), 'roxo-gato-9')
  assert.equal(resolveTarget(book, '   '), '')
})

test('errors read as plain Portuguese', () => {
  assert.match(humanError('ID does not exist'), /não existe/)
  assert.match(humanError('Remote desktop is offline'), /desligada|internet/)
  assert.match(humanError('Connection timeout'), /não respondeu/)
  assert.match(humanError('Wrong Password'), /senha/i)
  assert.match(humanError('something weird 0x42'), /Não foi possível conectar/)
})
