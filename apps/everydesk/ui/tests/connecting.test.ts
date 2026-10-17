// Moment 2 (connecting): engine messages in plain Portuguese, the phase reducer, and the
// light trace under "reduzir movimento".
import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  REACH,
  arrived,
  humanizeMsgbox,
  phaseFor,
  traceOptions,
  type ConnEvent,
} from '../src/lib/connecting/connecting.ts'

const err = (text: string) => humanizeMsgbox('error', text)

test('host-side refusals and timeouts', () => {
  assert.equal(err('Pedido não respondido em 60 s').title, 'A outra máquina não respondeu.')
  assert.equal(err('Closed manually by the peer').title, 'A outra máquina recusou.')
  assert.equal(err('Conexão encerrada pelo dono da máquina').title, 'O dono da máquina encerrou a conexão.')
  assert.equal(err('Reconfirmação não respondida em 60 s').title, 'O dono da máquina não confirmou a tempo.')
  assert.equal(err('Aguardando o aplicativo EveryDesk').title, 'A outra máquina ainda está abrindo o EveryDesk.')
})

test('not found: rendezvous failure and timeouts, without the clock hint', () => {
  const nf = 'Não encontrei essa máquina. Confira o código.'
  for (const text of [
    'Failed to connect via rendezvous server',
    'Failed to connect via rendezvous server. Se o problema persistir, confira o relógio do computador.',
    'Timeout',
    'Timeout. Se o problema persistir, confira o relógio do computador.',
    'Connection timed out',
    'ID does not exist',
  ]) {
    const h = err(text)
    assert.equal(h.title, nf, text)
    assert.ok(!`${h.title} ${h.body}`.includes('relógio'), `no clock hint for: ${text}`)
    assert.equal(h.retryable, true)
  }
})

test('only the server clock rejection keeps the clock text', () => {
  const h = err('O relógio deste computador está errado. Acerte a data e a hora e tente de novo.')
  assert.equal(`${h.title} ${h.body}`, 'O relógio deste computador está errado. Acerte a data e a hora e tente de novo.')
  assert.ok(!h.title.includes('Não encontrei'))
})

test('password prompts reuse the re-entry path, not an error', () => {
  const ask = humanizeMsgbox('input-password', '')
  assert.equal(ask.title, 'Essa máquina pede uma senha.')
  assert.equal(ask.retryable, false)
  const again = humanizeMsgbox('re-input-password', 'Do you want to enter again?')
  assert.equal(again.title, 'Essa máquina pede uma senha.')
  assert.ok(again.body.length > 0 && !/password|wrong/i.test(again.body))
})

test('unknown text: plain sentence, raw text kept only as detail', () => {
  const h = err('Reset by the peer')
  assert.equal(h.title, 'Não deu certo. Tente de novo.')
  assert.equal(h.detail, 'Reset by the peer')
  assert.equal(h.retryable, true)
  // known mappings carry no raw detail
  assert.equal(err('Closed manually by the peer').detail, undefined)
})

test('the owner ending the session is not offered as a retry', () => {
  assert.equal(err('Conexão encerrada pelo dono da máquina').retryable, false)
})

const box = (kind: string, text = '', title = '') => ({ type: 'msgbox', box: { kind, title, text } }) as ConnEvent

test('phaseFor: searching → asking → connecting → ready', () => {
  assert.equal(phaseFor([]).name, 'procurando')
  assert.equal(phaseFor([box('wait-remote-accept-nook')]).name, 'pedindo')
  assert.equal(phaseFor([box('wait-remote-accept-nook'), { type: 'displays' }]).name, 'conectando')
  assert.equal(phaseFor([{ type: 'connected' }]).name, 'conectando')
  assert.equal(phaseFor([box('success', 'Connected, waiting for image...')]).name, 'conectando')
  assert.equal(phaseFor([{ type: 'connected' }, { type: 'frame' }]).name, 'pronto')
})

test('phaseFor: ordering does not matter once a frame arrived', () => {
  assert.equal(phaseFor([{ type: 'frame' }]).name, 'pronto')
  assert.equal(phaseFor([{ type: 'frame' }, { type: 'displays' }, { type: 'connected' }]).name, 'pronto')
  // a late asking message never moves a connected session back
  assert.equal(phaseFor([{ type: 'displays' }, box('wait-remote-accept-nook')]).name, 'conectando')
  // errors after the first frame belong to the session view
  assert.equal(phaseFor([{ type: 'frame' }, box('error', 'Timeout')]).name, 'pronto')
})

test('phaseFor: password prompt, then login resumes the trace', () => {
  const p = phaseFor([box('input-password')])
  assert.equal(p.name, 'senha')
  assert.equal(p.message?.title, 'Essa máquina pede uma senha.')
  assert.equal(phaseFor([box('input-password'), { type: 'login' }]).name, 'conectando')
  assert.equal(phaseFor([box('input-password'), { type: 'login' }, box('re-input-password')]).name, 'senha')
})

test('phaseFor: errors and silent closes before the first frame', () => {
  const e = phaseFor([box('wait-remote-accept-nook'), box('error', 'Pedido não respondido em 60 s')])
  assert.equal(e.name, 'erro')
  assert.equal(e.message?.title, 'A outra máquina não respondeu.')
  const c = phaseFor([{ type: 'closed', box: { kind: 'error', title: 'Erro', text: 'Timeout' } }])
  assert.equal(c.name, 'erro')
  assert.equal(c.message?.title, 'Não encontrei essa máquina. Confira o código.')
  const silent = phaseFor([{ type: 'closed', box: null }])
  assert.equal(silent.name, 'erro')
  assert.equal(silent.message?.title, 'Não deu certo. Tente de novo.')
  // the first error wins over the close that follows it
  assert.equal(
    phaseFor([box('error', 'Closed manually by the peer'), { type: 'closed', box: null }]).message?.title,
    'A outra máquina recusou.',
  )
})

test('the trace reaches the other card once it answers; its dot lights only when connecting', () => {
  assert.ok(REACH.procurando < REACH.pedindo)
  assert.equal(REACH.pedindo, 1)
  assert.equal(REACH.conectando, 1)
  assert.equal(arrived('pedindo'), false)
  assert.equal(arrived('senha'), false)
  assert.equal(arrived('conectando'), true)
})

test('reduced motion: the trace jumps (duration 0), no travelling spark loop', () => {
  const reduced = traceOptions(true)
  assert.equal(reduced.reach.duration, 0)
  assert.equal(reduced.spark.duration, 0)
  assert.equal(reduced.spark.iterations, 1)
  const full = traceOptions(false)
  assert.ok((full.reach.duration as number) > 0)
  assert.equal(full.spark.iterations, Infinity)
})
