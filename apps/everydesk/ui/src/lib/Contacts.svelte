<script lang="ts">
  // Home view contacts (pairing book): online dot, name, code, policy (editable only for
  // peers that connect here), Conectar and Esquecer. Utilitarian; the real design is Stage 3.
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import {
    applyPresence,
    policyFromKey,
    policyKey,
    policyLabel,
    policyOptions,
    type Contact,
    type Presence,
  } from './policy'

  let { onconnect }: { onconnect: (peerId: string) => void } = $props()

  let contacts = $state<Contact[]>([])
  let error = $state('')
  let listEl: HTMLElement | undefined = $state()

  /** Periodic reload: skipped while a control in the list has focus (an open select). */
  async function poll() {
    if (listEl && listEl.contains(document.activeElement)) return
    await load()
  }

  async function load() {
    try {
      contacts = await invoke<Contact[]>('contacts_list')
    } catch (e) {
      error = `${e}`
    }
  }

  async function setPolicy(c: Contact, key: string) {
    error = ''
    try {
      await invoke('contacts_set_policy', { peerId: c.peer_id, policy: policyFromKey(key) })
    } catch (e) {
      error = `${e}`
    }
    await load()
  }

  async function remove(c: Contact) {
    if (!confirm(`Esquecer ${c.name || c.peer_id}? Ele vai precisar do código e de um aceite de novo.`)) return
    error = ''
    try {
      await invoke('contacts_remove', { peerId: c.peer_id })
    } catch (e) {
      error = `${e}`
    }
    await load()
  }

  onMount(() => {
    load()
    // The book changes when a prompt is accepted with "lembrar" or a host remembers us.
    const t = setInterval(poll, 5000)
    const un = listen<Presence[]>('contacts:presence', (e) => {
      contacts = applyPresence(contacts, e.payload)
    })
    invoke<[string, boolean][]>('contacts_presence')
      .then((s) => {
        contacts = applyPresence(
          contacts,
          s.map(([peer_id, online]) => ({ peer_id, online })),
        )
      })
      .catch((e) => console.error('contacts_presence', e))
    return () => {
      clearInterval(t)
      un.then((f) => f())
    }
  })
</script>

<section class="contacts" bind:this={listEl}>
  <h2>Contatos</h2>
  {#if contacts.length === 0}
    <p>Nenhum contato ainda.</p>
  {:else}
    <ul>
      {#each contacts as c (c.peer_id)}
        <li>
          <span class="dot" class:online={c.online} title={c.online ? 'online' : 'offline'}></span>
          <span class="name">{c.name || c.peer_id}</span>
          <span class="code">{c.peer_id}</span>
          {#if c.policy}
            <select value={policyKey(c.policy)} onchange={(e) => setPolicy(c, e.currentTarget.value)}>
              {#each policyOptions() as opt (opt.key)}
                <option value={opt.key}>{opt.label}</option>
              {/each}
            </select>
          {:else}
            <span>{policyLabel(null)}</span>
          {/if}
          <button onclick={() => onconnect(c.peer_id)}>Conectar</button>
          <button onclick={() => remove(c)}>Esquecer</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
</section>

<style>
  ul {
    list-style: none;
    padding: 0;
  }
  li {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 4px 0;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #999;
  }
  .dot.online {
    background: #2a2;
  }
  .code {
    color: #666;
    font-family: monospace;
  }
  .error {
    color: #b00;
  }
</style>
