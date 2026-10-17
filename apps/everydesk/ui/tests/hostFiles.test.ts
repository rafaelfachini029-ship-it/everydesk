import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  NOTE_MS,
  NO_FILES,
  PERMISSION_ROWS,
  applyFile,
  currentNote,
  fileVerb,
  formatSize,
  hydrateFiles,
  receivedLabel,
  type CmFile,
} from '../src/lib/host/files.ts'

const f = (id: number, state: CmFile['state'], at = 0, conn_id = 1): CmFile => ({
  conn_id,
  id,
  name: 'foto.jpg',
  size: 2.1 * 1024 * 1024,
  state,
  at,
})

test('C1: the badge line reads the transfer in plain words', () => {
  assert.equal(formatSize(2.1 * 1024 * 1024), '2,1 MB')
  assert.equal(formatSize(340 * 1024), '340 KB')
  assert.equal(formatSize(12), '12 bytes')
  assert.equal(formatSize(3.5 * 1024 ** 3), '3,5 GB')
  assert.equal(formatSize(0), '')
  assert.equal(fileVerb(f(1, 'done')), 'enviou foto.jpg (2,1 MB)')
  assert.equal(fileVerb(f(1, 'started')), 'está enviando foto.jpg (2,1 MB)')
  assert.equal(fileVerb({ name: 'a', size: 0, state: 'done' }), 'enviou a')
  assert.match(fileVerb(f(1, 'cancelled')), /cancelado/)
  assert.match(fileVerb(f(1, 'error')), /não chegou/)
  assert.equal(receivedLabel(1), '1 arquivo recebido nesta sessão')
  assert.equal(receivedLabel(3), '3 arquivos recebidos nesta sessão')
})

test('C1: the line fades after 6 s; the counter keeps what arrived', () => {
  let s = applyFile(NO_FILES, f(1, 'started'), 1000)
  assert.equal(s.received, 0)
  assert.equal(currentNote(s, 1000)?.state, 'started')
  s = applyFile(s, f(1, 'done'), 2000)
  assert.equal(s.received, 1)
  assert.equal(currentNote(s, 2000 + NOTE_MS - 1)?.state, 'done')
  assert.equal(currentNote(s, 2000 + NOTE_MS), null)
  s = applyFile(s, f(2, 'cancelled'), 9000)
  assert.equal(s.received, 1)
})

test('C1: hydrated from cm_pending().files: count per connection, note only if recent', () => {
  const list = [f(1, 'started', 0), f(1, 'done', 100), f(2, 'done', 200, 9), f(3, 'done', 10_000)]
  const h = hydrateFiles(list, 1, 10_500)
  assert.equal(h.received, 2)
  assert.equal(h.last?.file.id, 3)
  assert.equal(hydrateFiles(list, 1, 10_000 + NOTE_MS).last, null)
  assert.deepEqual(hydrateFiles(undefined, 1, 0), NO_FILES)
  assert.equal(hydrateFiles(list, 9, 0).received, 1)
})

test('host permission switches: four, plain names, engine permission names', () => {
  assert.deepEqual(
    PERMISSION_ROWS.map((r) => [r.name, r.label]),
    [
      ['file', 'Receber arquivos'],
      ['audio', 'Compartilhar o som'],
      ['recording', 'Permitir gravar'],
      ['clipboard', 'Área de transferência'],
    ],
  )
})
