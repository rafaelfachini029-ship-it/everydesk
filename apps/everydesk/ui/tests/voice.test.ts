import assert from 'node:assert/strict'
import { test } from 'node:test'
import { get } from 'svelte/store'
import {
  SELF_CLOSE_REASON,
  VOICE_DENIED,
  VOICE_MUTED,
  VOICE_NOTICES,
  VOICE_OFF,
  clickIntent,
  closeNotice,
  dispatchVoice,
  dotLabel,
  hostVoice,
  hydrateVoice,
  toolLabel,
  voice,
  voiceHint,
  voiceReducer,
  voiceTitle,
  type Voice,
  type VoiceAction,
} from '../src/lib/session/voice.ts'

const run = (actions: VoiceAction[], s: Voice = VOICE_OFF) => actions.reduce(voiceReducer, s)
const ev = (state: 'waiting' | 'on' | 'off' | 'incoming', reason: string | null = null): VoiceAction => ({
  type: 'event',
  event: { state, reason },
})

test('one click: off -> waiting -> on -> off', () => {
  let s = run([{ type: 'start' }])
  assert.equal(s.state, 'waiting')
  s = run([ev('waiting')], s)
  assert.equal(s.state, 'waiting')
  s = run([ev('on')], s)
  assert.deepEqual(s, { state: 'on', notice: null })
  assert.deepEqual(clickIntent(s.state), { start: false })
  s = run([{ type: 'stop' }], s)
  assert.deepEqual(s, VOICE_OFF)
  // the engine's own "off" for our hang-up says nothing more
  s = run([ev('off', SELF_CLOSE_REASON)], s)
  assert.deepEqual(s, VOICE_OFF)
  assert.deepEqual(clickIntent(s.state), { start: true })
})

test('Review Focus 3: a refusal by the host is "sem voz" in the same step', () => {
  const s = run([{ type: 'start' }, ev('waiting'), ev('off', '')])
  assert.equal(s.state, 'off')
  assert.equal(s.notice, VOICE_NOTICES.recusada)
  assert.equal(s.notice, 'A outra máquina recusou a voz.')
})

test('Review Focus 3: a hang-up by the host is "sem voz" in the same step', () => {
  const s = run([{ type: 'start' }, ev('waiting'), ev('on'), ev('off', '')])
  assert.deepEqual(s, { state: 'off', notice: 'Voz encerrada.' })
})

test('off wins from any state', () => {
  for (const from of [VOICE_OFF, run([{ type: 'start' }]), run([ev('on')])]) {
    assert.equal(run([ev('off', '')], from).state, 'off')
    assert.equal(run([{ type: 'stop' }], from).state, 'off')
    assert.equal(run([{ type: 'failed', message: 'x' }], from).state, 'off')
  }
})

test('reasons map to pt-BR', () => {
  assert.equal(closeNotice('waiting', ''), VOICE_NOTICES.recusada)
  assert.equal(closeNotice('waiting', null), VOICE_NOTICES.recusada)
  assert.equal(closeNotice('on', ''), VOICE_NOTICES.encerrada)
  assert.equal(closeNotice('on', 'recusada'), VOICE_NOTICES.recusada)
  assert.equal(closeNotice('waiting', 'Refused'), VOICE_NOTICES.recusada)
  assert.equal(closeNotice('on', 'encerrada'), VOICE_NOTICES.encerrada)
  assert.equal(closeNotice('on', SELF_CLOSE_REASON), null)
  assert.equal(closeNotice('waiting', SELF_CLOSE_REASON), null)
  assert.equal(closeNotice('off', ''), null)
})

test('notices clear; start clears a stale notice; incoming is ignored on the controller', () => {
  let s = run([{ type: 'start' }, ev('off', '')])
  assert.ok(s.notice)
  s = run([{ type: 'seen' }], s)
  assert.deepEqual(s, VOICE_OFF)
  s = run([{ type: 'start' }, ev('off', ''), { type: 'start' }])
  assert.deepEqual(s, { state: 'waiting', notice: null })
  assert.deepEqual(run([ev('incoming')]), VOICE_OFF)
  // a second click while waiting is not a second request
  const w = run([{ type: 'start' }])
  assert.equal(run([{ type: 'start' }], w), w)
  assert.deepEqual(run([{ type: 'failed', message: 'Nenhuma sessão aberta' }]), { state: 'off', notice: 'Nenhuma sessão aberta' })
})

test('labels and hints', () => {
  assert.equal(toolLabel('off'), 'Ligar a voz')
  assert.equal(toolLabel('waiting'), 'Cancelar a chamada de voz')
  assert.equal(toolLabel('on'), 'Encerrar a voz')
  assert.equal(dotLabel('off'), 'Voz desligada')
  assert.equal(dotLabel('on'), 'Voz ligada')
  assert.equal(voiceHint('off', { denied: true, muted: false }), VOICE_DENIED)
  assert.equal(voiceHint('on', { denied: false, muted: true }), VOICE_MUTED)
  assert.equal(voiceHint('off', { denied: false, muted: true }), 'Ligar a voz')
})

test('shared store', () => {
  dispatchVoice({ type: 'reset' })
  dispatchVoice({ type: 'start' })
  assert.equal(get(voice).state, 'waiting')
  dispatchVoice({ type: 'event', event: { state: 'off', reason: '' } })
  assert.equal(get(voice).state, 'off')
  dispatchVoice({ type: 'reset' })
  assert.deepEqual(get(voice), VOICE_OFF)
})

test('host: cm:voice to card / indicator, hydrated from cm_pending', () => {
  assert.equal(hostVoice(undefined), 'none')
  assert.equal(hostVoice({ incoming: true, active: false }), 'incoming')
  assert.equal(hostVoice({ incoming: false, active: true }), 'active')
  assert.equal(hostVoice({ incoming: false, active: false }), 'none')
  const list = [
    { conn_id: 1, incoming: true, active: false },
    { conn_id: 2, incoming: false, active: true },
  ]
  assert.equal(hydrateVoice(list, 1), 'incoming')
  assert.equal(hydrateVoice(list, 2), 'active')
  assert.equal(hydrateVoice(list, 3), 'none')
  assert.equal(hydrateVoice(undefined, 1), 'none')
  assert.equal(voiceTitle('Mãe'), 'Mãe quer conversar por voz')
})
