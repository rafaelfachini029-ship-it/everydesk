<script lang="ts">
  // Accessed side: accept/deny prompts (60 s, then the engine closes the connection with
  // "Pedido não respondido em 60 s") and a "Conectado" indicator while a session is authorized.
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import {
    CM_EVENTS,
    remainingSeconds,
    type Connected,
    type Incoming,
    type Removed,
  } from './cm'

  type Pending = Incoming & { start: number }

  let pending = $state<Pending[]>([])
  let connected = $state<Connected[]>([])
  let now = $state(Date.now())

  function drop(connId: number) {
    pending = pending.filter((p) => p.conn_id !== connId)
    connected = connected.filter((c) => c.conn_id !== connId)
  }

  async function accept(p: Pending) {
    pending = pending.filter((x) => x.conn_id !== p.conn_id)
    await invoke('cm_authorize', { connId: p.conn_id })
    // `authorize` does not echo `cm:connected`; a vanished connection still sends `cm:removed`.
    if (!connected.some((c) => c.conn_id === p.conn_id)) {
      connected = [...connected, { conn_id: p.conn_id, peer_id: p.peer_id, name: p.name }]
    }
  }

  async function close(connId: number) {
    drop(connId)
    await invoke('cm_close', { connId })
  }

  onMount(() => {
    const unlisten: Promise<UnlistenFn>[] = [
      listen<Incoming>(CM_EVENTS.incoming, (e) => {
        const p = e.payload
        pending = [...pending.filter((x) => x.conn_id !== p.conn_id), { ...p, start: Date.now() }]
      }),
      listen<Connected>(CM_EVENTS.connected, (e) => {
        const c = e.payload
        pending = pending.filter((x) => x.conn_id !== c.conn_id)
        connected = [...connected.filter((x) => x.conn_id !== c.conn_id), c]
      }),
      listen<Removed>(CM_EVENTS.removed, (e) => drop(e.payload.conn_id)),
    ]
    const timer = setInterval(() => {
      now = Date.now()
      // The engine closes an unanswered prompt itself; just drop the card at 0.
      pending = pending.filter((p) => remainingSeconds(p.start, now) > 0)
    }, 250)
    return () => {
      clearInterval(timer)
      unlisten.forEach((u) => u.then((f) => f()))
    }
  })
</script>

<div class="cm">
  {#each connected as c (c.conn_id)}
    <div class="connected">
      <span>Conectado: {c.name || c.peer_id}</span>
      <button onclick={() => close(c.conn_id)}>Encerrar</button>
    </div>
  {/each}
  {#each pending as p (p.conn_id)}
    <div class="card">
      <strong>Pedido de acesso{p.is_file_transfer ? ' (arquivos)' : ''}</strong>
      <div>ID: {p.peer_id}</div>
      <div>Nome: {p.name}</div>
      <div class="actions">
        <button onclick={() => accept(p)}>Aceitar</button>
        <button onclick={() => close(p.conn_id)}>Recusar</button>
        <span class="count">{remainingSeconds(p.start, now)} s</span>
      </div>
    </div>
  {/each}
</div>

<style>
  .cm {
    position: fixed;
    right: 12px;
    bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 1000;
    max-width: 320px;
  }
  .card,
  .connected {
    background: #fff;
    color: #000;
    border: 1px solid #888;
    padding: 8px 10px;
    font-size: 14px;
  }
  .connected {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: space-between;
  }
  .actions {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 6px;
  }
  .count {
    margin-left: auto;
  }
</style>
