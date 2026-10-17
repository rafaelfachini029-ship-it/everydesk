import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  CHAT_EMPTY,
  CHAT_KEEP,
  CHAT_MAX_CHARS,
  capIncoming,
  charsLeft,
  chatKey,
  chatReducer,
  formatAt,
  lastReceived,
  outgoing,
  unreadLabel,
  type ChatAction,
  type ChatState,
} from '../src/lib/session/chat.ts'

const run = (actions: ChatAction[], s: ChatState = CHAT_EMPTY) => actions.reduce(chatReducer, s)

test('messages stay in time order; same instant keeps arrival order', () => {
  const s = run([
    { type: 'received', text: 'b', at: 20 },
    { type: 'sent', text: 'a', at: 10 },
    { type: 'received', text: 'c', at: 20 },
    { type: 'sent', text: 'd', at: 30 },
  ])
  assert.deepEqual(
    s.messages.map((m) => [m.text, m.mine]),
    [
      ['a', true],
      ['b', false],
      ['c', false],
      ['d', true],
    ],
  )
  assert.equal(new Set(s.messages.map((m) => m.id)).size, 4)
})

test('unread counts only received messages while closed; opening clears it', () => {
  let s = run([
    { type: 'received', text: 'oi', at: 1 },
    { type: 'sent', text: 'olá', at: 2 },
    { type: 'received', text: 'tudo bem?', at: 3 },
  ])
  assert.equal(s.unread, 2)
  s = chatReducer(s, { type: 'open' })
  assert.equal(s.unread, 0)
  s = chatReducer(s, { type: 'received', text: 'aberto', at: 4 })
  assert.equal(s.unread, 0, 'nothing is unread while the conversation is open')
  s = run([{ type: 'close' }, { type: 'received', text: 'fechado', at: 5 }], s)
  assert.equal(s.unread, 1)
})

test('keeps the last 500', () => {
  const actions: ChatAction[] = []
  for (let i = 0; i < CHAT_KEEP + 7; i++) actions.push({ type: 'received', text: `m${i}`, at: i })
  const s = run(actions)
  assert.equal(s.messages.length, CHAT_KEEP)
  assert.equal(s.messages[0].text, 'm7')
  assert.equal(s.messages.at(-1)!.text, `m${CHAT_KEEP + 6}`)
})

test('hydrate merges by time, skips what is already there, and counts the unanswered tail', () => {
  let s = run([{ type: 'received', text: 'agora', at: 50 }])
  s = chatReducer(s, {
    type: 'hydrate',
    messages: [
      { text: 'oi', at: 10, mine: false },
      { text: 'oi, mãe', at: 20, mine: true },
      { text: 'pode olhar a impressora?', at: 30, mine: false },
      { text: 'agora', at: 50, mine: false },
    ],
  })
  assert.deepEqual(
    s.messages.map((m) => m.text),
    ['oi', 'oi, mãe', 'pode olhar a impressora?', 'agora'],
  )
  assert.equal(s.unread, 2)
  assert.equal(lastReceived(s)!.text, 'agora')
  assert.equal(lastReceived(CHAT_EMPTY), null)
})

test('Enter sends, Shift+Enter is a new line, Escape closes, IME keeps its Enter', () => {
  assert.equal(chatKey({ key: 'Enter', shiftKey: false }), 'send')
  assert.equal(chatKey({ key: 'Enter', shiftKey: true }), 'newline')
  assert.equal(chatKey({ key: 'Escape', shiftKey: false }), 'close')
  assert.equal(chatKey({ key: 'Enter', shiftKey: false, isComposing: true }), null)
  assert.equal(chatKey({ key: 'a', shiftKey: false }), null)
})

test('outgoing trims and refuses empty or too long text', () => {
  assert.equal(outgoing('  oi \n'), 'oi')
  assert.equal(outgoing(' \n\t'), null)
  assert.equal(outgoing('a\nb'), 'a\nb')
  assert.equal(outgoing('é'.repeat(CHAT_MAX_CHARS)), 'é'.repeat(CHAT_MAX_CHARS))
  assert.equal(outgoing('é'.repeat(CHAT_MAX_CHARS + 1)), null)
})

test('time reads 14:32; unread past nine reads 9+', () => {
  const tz = { hours: () => 9, minutes: () => 5 }
  assert.equal(formatAt(0, tz), '09:05')
  const d = new Date(2026, 9, 17, 14, 32)
  assert.equal(formatAt(d.getTime()), '14:32')
  assert.equal(unreadLabel(3), '3')
  assert.equal(unreadLabel(12), '9+')
})

test('nothing typed in the chat reaches the remote: the panel lives inside the pill', async () => {
  const { readFileSync } = await import('node:fs')
  const { shouldForwardKey } = await import('../src/lib/session/sessionPill.ts')
  const pill = readFileSync(new URL('../src/lib/session/SessionPill.svelte', import.meta.url), 'utf8')
  const inside = pill.slice(pill.indexOf('<Pill '), pill.indexOf('</Pill>'))
  assert.match(inside, /<ChatPanel /)
  // a key whose target is in the pill (the chat field) is never forwarded
  assert.equal(shouldForwardKey({ revealed: true, canvasFocused: false, pillFocused: true }), false)
})

test('a received message past 4000 characters is cut with "…"; replies show a counter', () => {
  const max = 'é'.repeat(CHAT_MAX_CHARS)
  assert.equal(capIncoming('oi'), 'oi')
  assert.equal(capIncoming(max), max)
  assert.equal(capIncoming(max + 'abc'), max + '…')
  const s = run([{ type: 'received', text: max + 'xyz', at: 1 }])
  assert.equal(s.messages[0].text, max + '…')
  const h = run([{ type: 'hydrate', messages: [{ text: max + 'x', at: 1, mine: false }] }])
  assert.equal(h.messages[0].text, max + '…')
  assert.equal(charsLeft('oi'), null)
  assert.deepEqual(charsLeft('a'.repeat(CHAT_MAX_CHARS - 10)), { left: 10, over: false, text: 'restam 10 caracteres' })
  assert.deepEqual(charsLeft('a'.repeat(CHAT_MAX_CHARS + 1)), { left: -1, over: true, text: '1 caractere a mais' })
})
