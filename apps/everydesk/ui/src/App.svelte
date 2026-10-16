<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'

  let myId = $state('...')
  let status = $state('...')
  let peerId = $state('')
  let password = $state('')

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
    // Etapa 1: ainda sem efeito.
  }
</script>

<main>
  <p>Seu ID</p>
  <h1 id="my-id">{myId}</h1>
  <p>Servidor: <span id="server-status">{status}</span></p>

  <form onsubmit={connect}>
    <input placeholder="ID remoto" bind:value={peerId} />
    <input type="password" placeholder="Senha" bind:value={password} />
    <button type="submit">Conectar</button>
  </form>
</main>
