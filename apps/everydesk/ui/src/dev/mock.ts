// Dev-only fake backend so the UI renders in a plain browser (design screenshots).
// Loaded by main.ts only when import.meta.env.DEV and Tauri is absent; never in builds.
import { mockIPC } from '@tauri-apps/api/mocks'
import type { Contact } from '../lib/policy.ts'

const contacts: Contact[] = [
  { peer_id: 'agua-lua-7', name: 'Mãe', policy: { kind: 'StayConnected' }, online: true },
  { peer_id: 'verde-onca-13', name: 'Escritório', policy: null, online: true },
  { peer_id: 'cedro-falcao-58', name: 'Notebook da Ana', policy: { kind: 'AskAlways' }, online: false },
  { peer_id: 'rio-coruja-21', name: 'Recepção', policy: { kind: 'Reconfirm', every_min: 45 }, online: false },
]

mockIPC(
  (cmd) => {
    switch (cmd) {
      case 'get_my_id':
        return 'azul-tigre-42'
      case 'get_server_status':
        return 'registrado'
      case 'contacts_list':
        return contacts
      case 'contacts_presence':
        return contacts.map((c) => [c.peer_id, c.online])
      case 'cm_pending':
        return { pending: [], connected: [], offers: [], identities: [], reconfirms: [] }
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
