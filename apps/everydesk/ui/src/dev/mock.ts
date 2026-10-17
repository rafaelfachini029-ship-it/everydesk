// Dev-only fake backend so the UI renders in a plain browser (design screenshots).
// Loaded by main.ts only when import.meta.env.DEV and Tauri is absent; never in builds.
import type { Channel } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { mockIPC } from '@tauri-apps/api/mocks'
import type { Contact } from '../lib/policy.ts'

const contacts: Contact[] = [
  { peer_id: 'agua-lua-7', name: 'Mãe', policy: { kind: 'StayConnected' }, online: true },
  { peer_id: 'verde-onca-13', name: 'Escritório', policy: null, online: true },
  { peer_id: 'cedro-falcao-58', name: 'Notebook da Ana', policy: { kind: 'AskAlways' }, online: false },
  { peer_id: 'rio-coruja-21', name: 'Recepção', policy: { kind: 'Reconfirm', every_min: 45 }, online: false },
]

mockIPC(
  (cmd, args) => {
    switch (cmd) {
      case 'get_my_id':
        return 'azul-tigre-42'
      case 'get_server_status':
        return 'registrado'
      case 'contacts_list':
        return contacts
      case 'contacts_presence':
        return contacts.map((c) => [c.peer_id, c.online])
      case 'session_connect':
        connecting((args as { frames: Channel<ArrayBuffer> }).frames)
        return null
      case 'cm_pending':
        // /#conectado: two authorized sessions on this machine (ConnectedBadge).
        return {
          pending: [],
          connected:
            location.hash === '#conectado'
              ? [
                  { conn_id: 1, peer_id: 'agua-lua-7', name: 'Mãe' },
                  { conn_id: 2, peer_id: 'pedra-gaviao-90', name: '' },
                ]
              : [],
          offers: [],
          identities: [],
          reconfirms: [],
        }
      default:
        return null
    }
  },
  { shouldMockEvents: true },
)

// /#foco: open with the field focused (the cards unfold), for the screenshot of that state.
// /#menu: the same, with the first card's "…" menu open.
if (location.hash === '#foco' || location.hash === '#menu') {
  setTimeout(() => document.querySelector<HTMLInputElement>('.go input')?.focus(), 300)
  if (location.hash === '#menu')
    setTimeout(() => document.querySelector<HTMLButtonElement>('.card .more')?.click(), 900)
}
// /#claro: light theme, with the settings panel open.
if (location.hash === '#claro') {
  document.documentElement.dataset.theme = 'light'
  setTimeout(() => document.querySelector<HTMLButtonElement>('.settings .btn')?.click(), 300)
}

// Moment 2 (connecting). /#busca: connect to "Mãe" and stay searching. /#pedindo: the host
// shows its accept card. /#erro: the server never answers (with the engine's clock hint,
// which the screen must not show). /#revela: a first frame arrives and the session opens.
// /#sessao (moment 3): straight into a session with two monitors and quality readings.
const flow = location.hash.slice(1)
function connecting(frames: Channel<ArrayBuffer>) {
  const at = (ms: number, f: () => void) => setTimeout(f, ms)
  const box = (kind: string, title: string, text: string) => emit('session:msgbox', { kind, title, text })
  if (flow === 'pedindo' || flow === 'revela') at(900, () => box('wait-remote-accept-nook', 'Prompt', ''))
  if (flow === 'erro')
    at(900, () => {
      box('error', 'Connection Error', 'Failed to connect via rendezvous server. Se o problema persistir, confira o relógio do computador.')
      emit('session:closed')
    })
  if (flow === 'sessao') {
    // Moment 3: a session with two monitors and live quality readings.
    at(300, () => emit('session:displays', [{ x: 0, y: 0, w: 1280, h: 800 }, { x: 1280, y: 0, w: 1280, h: 800 }]))
    at(400, () => emit('session:connected'))
    at(600, () => frames.onmessage(fakeFrame(1280, 800)))
    let n = 0
    setInterval(() => emit('session:quality', { fps: 30 - (n % 3), delay: 32 + (n++ % 5), speed: null }), 1000)
  }
  if (flow === 'revela') {
    at(2200, () => emit('session:displays', [{ x: 0, y: 0, w: 1280, h: 800 }]))
    at(2400, () => emit('session:connected'))
    at(3000, () => frames.onmessage(fakeFrame(1280, 800)))
  }
}

/** A frame packet (16-byte header + RGBA) with a soft diagonal gradient. */
function fakeFrame(w: number, h: number): ArrayBuffer {
  const buf = new ArrayBuffer(16 + w * h * 4)
  const v = new DataView(buf)
  v.setUint32(4, w, true)
  v.setUint32(8, h, true)
  v.setUint32(12, w * 4, true)
  const px = new Uint8ClampedArray(buf, 16)
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4
      px[i] = 40 + (x / w) * 60
      px[i + 1] = 70 + (y / h) * 80
      px[i + 2] = 120 + (x / w) * 100
      px[i + 3] = 255
    }
  return buf
}

if (['busca', 'pedindo', 'erro', 'revela', 'sessao'].includes(flow))
  setTimeout(() => {
    const input = document.querySelector<HTMLInputElement>('.go input')
    if (!input) return
    input.value = 'mãe'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
  }, 400)
