// The pairing book (Stage 2 logic, moved out of the old Contacts.svelte): list, live presence,
// per-contact policy and "Esquecer". Polls `contacts_list` every 5 s unless `busy()` says a
// control in the list is in use (an open select), and listens to `contacts:presence`.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { applyPresence, policyFromKey, type Contact, type Presence } from '../policy.ts'

export class ContactBook {
  list = $state<Contact[]>([])
  error = $state('')

  async load() {
    try {
      this.list = await invoke<Contact[]>('contacts_list')
    } catch (e) {
      this.error = `${e}`
    }
  }

  async setPolicy(c: Contact, key: string) {
    this.error = ''
    try {
      await invoke('contacts_set_policy', { peerId: c.peer_id, policy: policyFromKey(key) })
    } catch {
      this.error = 'Não deu para mudar como este contato entra. Tente de novo.'
    }
    await this.load()
  }

  async remove(c: Contact) {
    this.error = ''
    try {
      await invoke('contacts_remove', { peerId: c.peer_id })
    } catch {
      this.error = `Não deu para esquecer ${c.name || c.peer_id}. Tente de novo.`
    }
    await this.load()
  }

  /** Starts loading, polling and presence; returns the cleanup. */
  start(busy: () => boolean = () => false): () => void {
    this.load()
    // The book changes when a prompt is accepted with "lembrar" or a host remembers us.
    const t = setInterval(() => {
      if (!busy()) this.load()
    }, 5000)
    const un = listen<Presence[]>('contacts:presence', (e) => {
      this.list = applyPresence(this.list, e.payload)
    })
    invoke<[string, boolean][]>('contacts_presence')
      .then((s) => {
        this.list = applyPresence(
          this.list,
          s.map(([peer_id, online]) => ({ peer_id, online })),
        )
      })
      .catch((e) => console.error('contacts_presence', e))
    return () => {
      clearInterval(t)
      un.then((f) => f())
    }
  }
}
