import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import {
  DENIED_NOTE,
  EXTRAS_EMPTY,
  SWITCHES,
  VIEW_ONLY_NOTE,
  actionDenied,
  extrasReducer,
  inputAllowed,
  optionAfter,
  qualityValue,
  switchView,
  toggleCommand,
  type ExtrasAction,
  type ExtrasState,
} from '../src/lib/session/more.ts'

const run = (actions: ExtrasAction[], s: ExtrasState = EXTRAS_EMPTY) => actions.reduce(extrasReducer, s)
const sw = (key: string) => SWITCHES.find((d) => d.key === key)!

test('labels are plain words, in this order', () => {
  assert.deepEqual(
    SWITCHES.map((d) => d.label),
    ['Só olhar', 'Bloquear teclado e mouse de lá', 'Som da outra máquina', 'Área de transferência', 'Mostrar o cursor de lá'],
  )
})

test('every option the panel sends is on the Rust allow-list', () => {
  const rs = readFileSync(new URL('../../src/commands.rs', import.meta.url), 'utf8')
  const list = rs.slice(rs.indexOf('pub const TOGGLES'), rs.indexOf('];', rs.indexOf('pub const TOGGLES')))
  for (const d of SWITCHES)
    for (const on of [true, false]) assert.ok(list.includes(`"${toggleCommand(d, on)}"`), toggleCommand(d, on))
})

test('hydrate keeps known options and a valid quality only', () => {
  const s = run([{ type: 'hydrate', options: { 'view-only': false, 'disable-audio': true, 'evil-option': true }, quality: 'sharp' }])
  assert.deepEqual(s.options, { 'view-only': false, 'disable-audio': true })
  assert.equal(s.quality, 'sharp')
  assert.equal(run([{ type: 'hydrate', options: {}, quality: 'custom' }], s).quality, 'sharp')
})

test('toggle reducer: inverted switches read the opposite of the option', () => {
  let s = run([{ type: 'hydrate', options: { 'disable-audio': false, 'disable-clipboard': true } }])
  assert.equal(switchView(s, sw('audio')).checked, true)
  assert.equal(switchView(s, sw('clipboard')).checked, false)
  // flipping "Som" off sets disable-audio
  assert.equal(optionAfter(sw('audio'), false), true)
  s = run([{ type: 'set', option: 'disable-audio', value: optionAfter(sw('audio'), false) }], s)
  assert.equal(switchView(s, sw('audio')).checked, false)
  assert.equal(toggleCommand(sw('audio'), false), 'disable-audio')
})

test('block input is two commands and follows session:block_input', () => {
  assert.equal(toggleCommand(sw('blockInput'), true), 'block-input')
  assert.equal(toggleCommand(sw('blockInput'), false), 'unblock-input')
  const s = run([{ type: 'blockInput', on: true }])
  assert.equal(switchView(s, sw('blockInput')).checked, true)
  assert.equal(switchView(run([{ type: 'blockInput', on: false }], s), sw('blockInput')).checked, false)
})

test('a permission taken away disables the related switch, and comes back', () => {
  let s = run([{ type: 'permission', name: 'clipboard', value: false }])
  assert.deepEqual(switchView(s, sw('clipboard')), { checked: true, disabled: true, note: DENIED_NOTE })
  assert.equal(switchView(s, sw('audio')).disabled, false)
  s = run([{ type: 'permission', name: 'clipboard', value: true }], s)
  assert.equal(switchView(s, sw('clipboard')).disabled, false)
  s = run([{ type: 'permission', name: 'audio', value: false }, { type: 'permission', name: 'audio', value: false }])
  assert.deepEqual(s.denied, ['audio'])
  assert.ok(actionDenied(run([{ type: 'permission', name: 'restart', value: false }]), 'restart'))
  assert.ok(actionDenied(run([{ type: 'permission', name: 'keyboard', value: false }]), 'ctrlAltDel'))
  assert.ok(!actionDenied(EXTRAS_EMPTY, 'lock'))
})

test('Só olhar turns off clipboard and block input and the keyboard actions', () => {
  const s = run([{ type: 'set', option: 'view-only', value: true }])
  assert.deepEqual(switchView(s, sw('clipboard')), { checked: false, disabled: true, note: VIEW_ONLY_NOTE })
  assert.equal(switchView(s, sw('blockInput')).disabled, true)
  assert.equal(switchView(s, sw('audio')).disabled, false)
  assert.ok(actionDenied(s, 'ctrlAltDel'))
  assert.ok(actionDenied(s, 'lock'))
  assert.ok(!actionDenied(s, 'restart'))
})

test('quality levels map to the engine names', () => {
  assert.equal(qualityValue('auto'), 'balanced')
  assert.equal(qualityValue('sharp'), 'best')
  assert.equal(qualityValue('smooth'), 'low')
  assert.equal(run([{ type: 'quality', level: 'smooth' }]).quality, 'smooth')
})

test('Review Focus 5: no input leaves while Só olhar is on', () => {
  assert.equal(inputAllowed({ viewOnly: true }), false)
  assert.equal(inputAllowed({ viewOnly: false }), true)
  // and RemoteCanvas gates every sender on it
  const src = readFileSync(new URL('../src/lib/RemoteCanvas.svelte', import.meta.url), 'utf8')
  for (const fn of ['onPointerMove', 'onPointerDown', 'onPointerUp', 'onWheel', 'onKey']) {
    const body = src.slice(src.indexOf(`function ${fn}(`), src.indexOf('\n  }\n', src.indexOf(`function ${fn}(`)))
    assert.ok(body.includes('allowed()'), fn)
  }
})
