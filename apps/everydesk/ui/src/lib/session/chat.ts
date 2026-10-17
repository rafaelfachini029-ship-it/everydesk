/**
 * Text chat, both sides (spec §5, "Conversa"): the message list as pure state so `node --test`
 * can exercise it. The controller's ChatPanel and the host's HostChat (one state per
 * connection) both run on `chatReducer`. Tauri side: `session_send_chat` / `session:chat
 * {text, at}` (controller), `cm_send_chat` / `cm:chat {conn_id, text, at, mine}` and
 * `cm_pending().chats` (host). No DOM or Tauri dependency.
 */

/** Longest message, in characters (mirrors `commands.rs::CHAT_MAX_CHARS`). */
export const CHAT_MAX_CHARS = 4000
/** Messages kept on screen; the oldest go first. */
export const CHAT_KEEP = 500

export const CHAT_EVENTS = { session: 'session:chat', cm: 'cm:chat' } as const

export interface ChatMsg {
  id: number
  text: string
  /** Unix ms. */
  at: number
  /** Written here ("você"). */
  mine: boolean
}

export interface ChatState {
  messages: ChatMsg[]
  /** Received while the conversation was closed. */
  unread: number
  open: boolean
  nextId: number
}

export const CHAT_EMPTY: ChatState = { messages: [], unread: 0, open: false, nextId: 1 }

export type ChatAction =
  /** From the other side (`session:chat` / `cm:chat`). */
  | { type: 'received'; text: string; at: number }
  /** Sent from here, once the command accepted it. */
  | { type: 'sent'; text: string; at: number }
  /** Recent history (`cm_pending().chats` for one connection), merged by time. */
  | { type: 'hydrate'; messages: { text: string; at: number; mine: boolean }[] }
  | { type: 'open' }
  | { type: 'close' }

/** A received message as shown: past CHAT_MAX_CHARS characters it is cut and ends in "…" (the
 *  other side may run anything; mirrors `commands.rs::cap_incoming`). */
export function capIncoming(text: string): string {
  const chars = [...text]
  return chars.length > CHAT_MAX_CHARS ? chars.slice(0, CHAT_MAX_CHARS).join('') + '…' : text
}

/** "restam 120 caracteres" near the limit, "3 caracteres a mais" past it; null well below. */
export function charsLeft(draft: string): { left: number; over: boolean; text: string } | null {
  const left = CHAT_MAX_CHARS - [...draft.trim()].length
  if (left < 0) return { left, over: true, text: `${-left} ${left === -1 ? 'caractere' : 'caracteres'} a mais` }
  if (left < 200) return { left, over: false, text: `restam ${left} ${left === 1 ? 'caractere' : 'caracteres'}` }
  return null
}

/** Ordered by time; same instant keeps arrival order. */
function insert(list: ChatMsg[], m: ChatMsg): ChatMsg[] {
  let i = list.length
  while (i > 0 && list[i - 1].at > m.at) i--
  const next = list.slice()
  next.splice(i, 0, m)
  return next.length > CHAT_KEEP ? next.slice(next.length - CHAT_KEEP) : next
}

export function chatReducer(s: ChatState, a: ChatAction): ChatState {
  switch (a.type) {
    case 'received':
    case 'sent': {
      const mine = a.type === 'sent'
      const text = mine ? a.text : capIncoming(a.text)
      const messages = insert(s.messages, { id: s.nextId, text, at: a.at, mine })
      const unread = !mine && !s.open ? s.unread + 1 : s.unread
      return { ...s, messages, unread, nextId: s.nextId + 1 }
    }
    case 'hydrate': {
      // Only what is not already here (a `cm:chat` can arrive before `cm_pending` answers).
      let { messages, nextId } = s
      let unread = s.unread
      for (const raw of a.messages) {
        const m = raw.mine ? raw : { ...raw, text: capIncoming(raw.text) }
        if (messages.some((x) => x.at === m.at && x.mine === m.mine && x.text === m.text)) continue
        messages = insert(messages, { id: nextId++, text: m.text, at: m.at, mine: m.mine })
      }
      // A webview that comes back mid-conversation: what the controller said after the
      // owner's last reply is unread.
      if (!s.open) {
        unread = 0
        for (let i = messages.length - 1; i >= 0 && !messages[i].mine; i--) unread++
      }
      return { ...s, messages, nextId, unread }
    }
    case 'open':
      return { ...s, open: true, unread: 0 }
    case 'close':
      return { ...s, open: false }
  }
}

/** The last message from the other side, if any. */
export function lastReceived(s: ChatState): ChatMsg | null {
  for (let i = s.messages.length - 1; i >= 0; i--) if (!s.messages[i].mine) return s.messages[i]
  return null
}

/** "14:32", local time. */
export function formatAt(at: number, tz?: { hours: (d: Date) => number; minutes: (d: Date) => number }): string {
  const d = new Date(at)
  const h = tz ? tz.hours(d) : d.getHours()
  const m = tz ? tz.minutes(d) : d.getMinutes()
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

/** What a key does in the message field: Enter sends, Shift+Enter is a new line, Escape
 *  closes. An IME composition keeps its Enter. */
export function chatKey(e: { key: string; shiftKey: boolean; isComposing?: boolean }): 'send' | 'newline' | 'close' | null {
  if (e.isComposing) return null
  if (e.key === 'Escape') return 'close'
  if (e.key === 'Enter') return e.shiftKey ? 'newline' : 'send'
  return null
}

/** The text as it would go out, or null when there is nothing to send (or it is too long). */
export function outgoing(draft: string): string | null {
  const t = draft.trim()
  if (!t || [...t].length > CHAT_MAX_CHARS) return null
  return t
}

/** "9+" past nine, so the count never widens the tool. */
export function unreadLabel(n: number): string {
  return n > 9 ? '9+' : String(n)
}
