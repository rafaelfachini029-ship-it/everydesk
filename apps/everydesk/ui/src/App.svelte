<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import Contacts from './lib/Contacts.svelte'
  import IncomingCard from './lib/IncomingCard.svelte'
  import RemoteCanvas from './lib/RemoteCanvas.svelte'
  import { normalizeCode, type MsgBox } from './lib/session'

  let myId = $state('...')
  let status = $state('...')
  let peerId = $state('')
  let password = $state('')
  let remote = $state<{ peerId: string; password: string } | null>(null)
  let lastError = $state('')

  async function refresh() {
    try {
      myId = await invoke<string>('get_my_id')
      status = await invoke<string>('get_server_status')
    } catch (e) {
      status = `erro: ${e}`
    }
  }

  onMount(() => {
    refresh()
    const t = setInterval(refresh, 2000)
    return () => clearInterval(t)
  })

  function connect(e: SubmitEvent) {
    e.preventDefault()
    const id = normalizeCode(peerId)
    if (!id) return
    lastError = ''
    remote = { peerId: id, password }
  }

  /** A contact connects without a password: the host's policy (or its card) decides. */
  function connectContact(id: string) {
    lastError = ''
    remote = { peerId: id, password: '' }
  }

  function onclosed(last: MsgBox | null) {
    remote = null
    if (last && last.kind.includes('error')) lastError = `${last.title}: ${last.text}`
  }
</script>

{#if remote}
  <RemoteCanvas peerId={remote.peerId} password={remote.password} {onclosed} />
{:else}
  <main>
    <p>Seu ID</p>
    <h1 id="my-id">{myId}</h1>
    <p>Servidor: <span id="server-status">{status}</span></p>

    <form onsubmit={connect}>
      <input placeholder="ID remoto" bind:value={peerId} />
      <input type="password" placeholder="Senha" bind:value={password} />
      <button type="submit">Conectar</button>
    </form>
    {#if lastError}<p class="error">{lastError}</p>{/if}

    <Contacts onconnect={connectContact} />
  </main>
{/if}
<IncomingCard />

<style>
  .error {
    color: #b00;
  }
</style>
