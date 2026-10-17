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

let mockQuality = 'auto'
const toggles: Record<string, boolean> = {
  'view-only': false,
  'disable-clipboard': false,
  'disable-audio': false,
  'show-remote-cursor': false,
  'lock-after-session-end': false,
}

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
      case 'session_send_files':
        return simulateSend((args as { paths: string[] }).paths)
      case 'session_pick_and_send':
        return simulateSend(['Contrato assinado.pdf'])
      // The "Mais" panel (/#mais): sound on, clipboard on, a host that does not allow restart.
      case 'session_get_toggles':
        return { ...toggles }
      case 'session_get_quality':
        return mockQuality
      case 'session_quality':
        mockQuality = (args as { level: string }).level
        return null
      case 'session_toggle': {
        const name = (args as { name: string }).name
        if (name === 'block-input' || name === 'unblock-input') return name === 'block-input'
        toggles[name] = !toggles[name]
        return toggles[name]
      }
      // Conversa (/#conversa, /#conversa-host): sending always works here.
      case 'session_send_chat':
      case 'cm_send_chat':
        return null
      case 'session_connect':
        connecting((args as { frames: Channel<ArrayBuffer> }).frames)
        return null
      case 'cm_pending':
        // /#conectado: two authorized sessions on this machine (ConnectedBadge).
        // /#pedidos: two access requests (one verified with an offer, one whose key does not
        // match) plus a "still connected?" ring, for the card stack (moment 4).
        if (location.hash === '#pedidos')
          return {
            pending: [
              { conn_id: 11, peer_id: 'agua-lua-7', name: 'Mãe', is_file_transfer: false, age_ms: 4000 },
              { conn_id: 12, peer_id: 'verde-onca-13', name: 'Escritório', is_file_transfer: false, age_ms: 1000 },
            ],
            connected: [{ conn_id: 13, peer_id: 'cedro-falcao-58', name: 'Notebook da Ana' }],
            offers: [{ conn_id: 11, peer_id: 'agua-lua-7', name: 'Mãe', key_changed: false }],
            identities: [
              { conn_id: 11, state: 'verified', known: true },
              { conn_id: 12, state: 'key_changed', known: true },
            ],
            reconfirms: [{ conn_id: 13, ring_secs: 60, age_ms: 9000 }],
          }
        return {
          pending: [],
          connected:
            location.hash === '#conectado' || location.hash === '#conversa-host'
              ? [
                  { conn_id: 1, peer_id: 'agua-lua-7', name: 'Mãe' },
                  { conn_id: 2, peer_id: 'pedra-gaviao-90', name: '' },
                ]
              : [],
          offers: [],
          identities: [],
          reconfirms: [],
          // /#conversa-host: Mãe asked something, got an answer, and asked again.
          chats:
            location.hash === '#conversa-host'
              ? [
                  { conn_id: 1, text: 'Oi, filho! A impressora parou de novo.', at: Date.now() - 240e3, mine: false },
                  { conn_id: 1, text: 'Já estou olhando, mãe.', at: Date.now() - 180e3, mine: true },
                ]
              : [],
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
// /#aviso and /#senha-sessao: a session, then a notice / a password prompt over it (the
// overlay is a modal dialog that owns the keyboard).
const flow = location.hash.slice(1)
const sessionFlow = flow === 'conversa' || flow === 'mais' || flow === 'sessao' || flow === 'aviso' || flow === 'senha-sessao' || flow === 'arquivos'
function connecting(frames: Channel<ArrayBuffer>) {
  const at = (ms: number, f: () => void) => setTimeout(f, ms)
  const box = (kind: string, title: string, text: string) => emit('session:msgbox', { kind, title, text })
  if (flow === 'pedindo' || flow === 'revela') at(900, () => box('wait-remote-accept-nook', 'Prompt', ''))
  if (flow === 'erro')
    at(900, () => {
      box('error', 'Connection Error', 'Failed to connect via rendezvous server. Se o problema persistir, confira o relógio do computador.')
      emit('session:closed')
    })
  if (sessionFlow) {
    // Moment 3: a session with two monitors and live quality readings.
    at(300, () => emit('session:displays', [{ x: 0, y: 0, w: 1280, h: 800 }, { x: 1280, y: 0, w: 1280, h: 800 }]))
    at(400, () => emit('session:connected'))
    at(600, () => frames.onmessage(fakeFrame(1280, 800)))
    let n = 0
    setInterval(() => emit('session:quality', { fps: 30 - (n % 3), delay: 32 + (n++ % 5), speed: null }), 1000)
    if (flow === 'aviso') at(1500, () => box('info', 'A outra máquina vai reiniciar', 'A sessão continua quando ela voltar.'))
    // Moment 5: three sends in the pill's trail — one moving, one asking, one just done.
    if (flow === 'arquivos')
      at(1200, () => {
        simulateSend(['Relatório anual 2026.pdf'], 0.62)
        const ask = nextJob++
        emit('session:job_added', { id: ask, name: 'fotos-natal.zip', to: '', total: 48e6 })
        at(300, () => emit('session:override', { id: ask, file_num: 0, to: '', is_upload: true, is_identical: false }))
        const done = nextJob++
        emit('session:job_added', { id: done, name: 'planilha-contas.xlsx', to: '', total: 220e3 })
        at(500, () => emit('session:job', { id: done, file_num: 0, kind: 'done', speed: 0, finished: 0, err: null }))
      })
    if (flow === 'mais') {
      at(900, () => emit('session:permission', { name: 'restart', value: false }))
      at(2600, () => document.querySelector<HTMLElement>('.dock .pill')?.focus())
      at(2900, () => document.querySelector<HTMLButtonElement>('.more .tool')?.click())
    }
    // Moment 6 (controller): Mãe writes, the pill comes down with the preview; then the panel
    // opens and an answer goes out.
    if (flow === 'conversa') {
      const said = (ms: number, text: string) => at(ms, () => emit('session:chat', { text, at: Date.now() }))
      said(1200, 'Oi! Você já está vendo minha tela?')
      at(4200, () => document.querySelector<HTMLElement>('.dock .pill')?.focus())
      at(4400, () => document.querySelector<HTMLButtonElement>('.chat .tool')?.click())
      at(4900, () => {
        const f = document.querySelector<HTMLTextAreaElement>('.chat textarea')
        if (!f) return
        f.value = 'Estou sim. Vou abrir as configurações da impressora.'
        f.dispatchEvent(new Event('input', { bubbles: true }))
        f.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
      })
      said(5600, 'Tá bom! Ela fica no escritório,\nperto da janela.')
      at(6000, () => {
        const f = document.querySelector<HTMLTextAreaElement>('.chat textarea')
        if (!f) return
        f.value = 'Achei. Já'
        f.dispatchEvent(new Event('input', { bubbles: true }))
      })
    }
    if (flow === 'senha-sessao') at(1500, () => box('re-input-password', 'Digite a senha de novo', 'A senha mudou na outra máquina.'))
  }
  if (flow === 'revela') {
    at(2200, () => emit('session:displays', [{ x: 0, y: 0, w: 1280, h: 800 }]))
    at(2400, () => emit('session:connected'))
    at(3000, () => frames.onmessage(fakeFrame(1280, 800)))
  }
}

let nextJob = 1
/** A fake upload per name: progress at ~1,2 MB/s, then done (`hold` = stop at that fraction). */
function simulateSend(names: string[], hold?: number): number[] {
  return names.map((name) => {
    const id = nextJob++
    const total = 18e6
    emit('session:job_added', { id, name, to: '', total: 0, preparing: true })
    setTimeout(() => emit('session:job_added', { id, name, to: `C:\\Users\\mae\\Desktop\\${name}`, total }), 600)
    let sent = hold === undefined ? 0 : total * (hold - 0.08)
    const t = setInterval(() => {
      sent = Math.min(total, sent + 1.25e6 * 0.4)
      if (hold !== undefined && sent >= total * hold) {
        sent = total * hold
        clearInterval(t)
      }
      emit('session:job', { id, file_num: 0, kind: 'progress', speed: 1.2 * 1024 * 1024, finished: sent, err: null })
      if (sent >= total) {
        clearInterval(t)
        emit('session:job', { id, file_num: 0, kind: 'done', speed: 0, finished: 0, err: null })
      }
    }, 400)
    return id
  })
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

// Moment 6 (host): a new message arrives while the owner looks at the badge.
if (flow === 'conversa-host')
  setTimeout(() => emit('cm:chat', { conn_id: 1, text: 'Ela voltou a imprimir! Obrigada ❤', at: Date.now(), mine: false }), 2000)

if (['conversa', 'busca', 'pedindo', 'erro', 'revela', 'sessao', 'aviso', 'senha-sessao', 'arquivos', 'mais'].includes(flow))
  setTimeout(() => {
    const input = document.querySelector<HTMLInputElement>('.go input')
    if (!input) return
    input.value = 'mãe'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
  }, 400)
