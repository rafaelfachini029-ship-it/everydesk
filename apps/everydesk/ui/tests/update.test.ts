import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  aboutLine,
  initialUpdateCard,
  updateCardView,
  updateReducer,
  type UpdateAction,
  type UpdateStatus,
} from '../src/lib/home/update.ts'

const st = (o: Partial<UpdateStatus>): UpdateStatus => ({
  route: 'installer',
  phase: 'available',
  version: '0.2.0',
  progress: null,
  message: null,
  ...o,
})
const run = (...as: UpdateAction[]) => as.reduce(updateReducer, initialUpdateCard)

test('nothing to show: no status, idle, or updates disabled (macOS, placeholder key)', () => {
  assert.equal(updateCardView(initialUpdateCard).visible, false)
  assert.equal(updateCardView(run({ type: 'status', status: st({ phase: 'idle', version: null }) })).visible, false)
  assert.equal(updateCardView(run({ type: 'status', status: st({ route: 'disabled' }) })).visible, false)
})

test('available: "Nova versão x.y.z." and "Atualizar agora"', () => {
  const v = updateCardView(run({ type: 'status', status: st({}) }))
  assert.equal(v.visible, true)
  assert.equal(v.title, 'Nova versão 0.2.0.')
  assert.match(v.body, /^Atualizar agora\?/)
  assert.match(v.body, /administrador/)
  assert.deepEqual(v.primary, { label: 'Atualizar agora', force: false })
  assert.equal(v.secondary, 'Depois')
  const p = updateCardView(run({ type: 'status', status: st({ route: 'portable' }) }))
  assert.match(p.body, /fecha e abre de novo/)
})

test('"Depois" hides this version; a newer one brings the card back', () => {
  const later = run({ type: 'status', status: st({}) }, { type: 'later' })
  assert.equal(updateCardView(later).visible, false)
  const again = updateReducer(later, { type: 'status', status: st({}) })
  assert.equal(updateCardView(again).visible, false)
  const newer = updateReducer(later, { type: 'status', status: st({ version: '0.3.0' }) })
  assert.equal(updateCardView(newer).visible, true)
})

test('session open: asks to end it, then sends force; Cancelar goes back', () => {
  const s = run({ type: 'status', status: st({}) }, { type: 'session-active' })
  const v = updateCardView(s)
  assert.equal(v.title, 'Há uma sessão ativa.')
  assert.match(v.body, /encerrada para atualizar/)
  assert.deepEqual(v.primary, { label: 'Encerrar e atualizar', force: true })
  assert.equal(v.secondary, 'Cancelar')
  assert.equal(v.tone, 'warn')
  assert.equal(updateCardView(updateReducer(s, { type: 'cancel' })).title, 'Nova versão 0.2.0.')
  // the download starting ends the question
  const dl = updateReducer(s, { type: 'status', status: st({ phase: 'downloading' }) })
  assert.equal(dl.confirming, false)
})

test('downloading shows progress, no buttons; installing says how', () => {
  const v = updateCardView(run({ type: 'status', status: st({ phase: 'downloading', progress: 42 }) }))
  assert.equal(v.busy, true)
  assert.equal(v.progress, 42)
  assert.equal(v.primary, null)
  assert.match(v.body, /42%/)
  const i = updateCardView(run({ type: 'status', status: st({ phase: 'installing' }) }))
  assert.equal(i.title, 'Instalando a versão 0.2.0…')
  assert.equal(i.busy, true)
})

test('refused signature: says nothing was installed and offers a retry', () => {
  const msg = 'A assinatura da atualização não confere. Nada foi instalado.'
  const v = updateCardView(run({ type: 'status', status: st({ phase: 'failed', message: msg }) }))
  assert.equal(v.title, 'Atualização não instalada.')
  assert.equal(v.body, msg)
  assert.equal(v.tone, 'danger')
  assert.deepEqual(v.primary, { label: 'Tentar de novo', force: false })
  // a plain invoke error is shown the same way, "Depois" clears it
  const e = run({ type: 'status', status: st({}) }, { type: 'error', message: 'Nenhuma atualização disponível.' })
  assert.equal(updateCardView(e).body, 'Nenhuma atualização disponível.')
  assert.equal(updateCardView(updateReducer(e, { type: 'later' })).visible, false)
})

test('Sobre: version and commit, or a local build', () => {
  assert.equal(
    aboutLine({ version: '0.1.0', commit: '82b42c3ea', source_url: '', license: 'AGPL-3.0' }),
    'Versão 0.1.0 · commit 82b42c3ea',
  )
  assert.equal(aboutLine({ version: '0.1.0', commit: null, source_url: '', license: 'AGPL-3.0' }), 'Versão 0.1.0 · compilação local')
})
