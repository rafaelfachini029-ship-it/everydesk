<script lang="ts">
  // Accessed side: accept/deny prompts (60 s, then the engine closes the connection with
  // "Pedido não respondido em 60 s") and a "Conectado" indicator while a session is authorized.
  // A prompt with a `cm:pair_offer` can also remember the controller under a policy; an
  // authorized session may get a `cm:reconfirm` ring (ReconfirmCard). `cm:identity` says
  // whether the claimed id/name is backed by a verified key; "lembrar" is never offered for
  // an unverified one, and a known id without its key gets a loud warning.
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import {
    CM_EVENTS,
    canRemember,
    identityNotice,
    remainingSeconds,
    rememberByDefault,
    type CmSnapshot,
    type Connected,
    type Identity,
    type Incoming,
    type PairOffer,
    type Reconfirm,
    type Removed,
  } from './cm'
  import { policyFromKey } from './policy'
  import PolicySelect from './PolicySelect.svelte'
  import ReconfirmCard from './ReconfirmCard.svelte'

  type Pending = Incoming & { start: number }
  type Ring = Reconfirm & { start: number }
  /** Per-prompt answers of the "lembrar" controls. */
  type Remember = { on: boolean; policy: string; replaceKey: boolean }

  let pending = $state<Pending[]>([])
  let connected = $state<Connected[]>([])
  let offers = $state<Record<number, PairOffer>>({})
  let identities = $state<Record<number, Identity>>({})
  /** Per-prompt error of a failed accept (the card stays up). */
  let errors = $state<Record<number, string>>({})
  let busy = $state<Record<number, boolean>>({})
  let remember = $state<Record<number, Remember>>({})
  let rings = $state<Ring[]>([])
  let now = $state(Date.now())

  function addOffer(o: PairOffer) {
    offers = { ...offers, [o.conn_id]: o }
    if (!remember[o.conn_id]) {
      // Spec 4.4: accepting a verified offer pairs by default, policy "Sempre perguntar".
      remember = {
        ...remember,
        [o.conn_id]: { on: rememberByDefault(o), policy: 'ask', replaceKey: false },
      }
    }
  }

  function forget(connId: number) {
    const { [connId]: _o, ...o } = offers
    const { [connId]: _r, ...r } = remember
    const { [connId]: _i, ...i } = identities
    const { [connId]: _e, ...e } = errors
    const { [connId]: _b, ...b } = busy
    offers = o
    remember = r
    identities = i
    errors = e
    busy = b
  }

  function drop(connId: number) {
    pending = pending.filter((p) => p.conn_id !== connId)
    connected = connected.filter((c) => c.conn_id !== connId)
    rings = rings.filter((r) => r.conn_id !== connId)
    forget(connId)
  }

  async function accept(p: Pending) {
    if (busy[p.conn_id]) return
    const r = canRemember(identities[p.conn_id], offers[p.conn_id]) ? remember[p.conn_id] : undefined
    busy = { ...busy, [p.conn_id]: true }
    errors = { ...errors, [p.conn_id]: '' }
    try {
      if (r?.on) {
        await invoke('cm_pair_ack', {
          connId: p.conn_id,
          accepted: true,
          policy: policyFromKey(r.policy),
          replaceKey: r.replaceKey,
        })
      } else {
        await invoke('cm_authorize', { connId: p.conn_id })
      }
    } catch (e) {
      // Keep the card: the owner can fix the choice (or untick "lembrar") and accept again.
      busy = { ...busy, [p.conn_id]: false }
      errors = { ...errors, [p.conn_id]: `Não foi possível aceitar: ${e}` }
      return
    }
    pending = pending.filter((x) => x.conn_id !== p.conn_id)
    forget(p.conn_id)
    // `authorize` does not echo `cm:connected`; a vanished connection still sends `cm:removed`.
    if (!connected.some((c) => c.conn_id === p.conn_id)) {
      connected = [...connected, { conn_id: p.conn_id, peer_id: p.peer_id, name: p.name }]
    }
  }

  async function close(connId: number) {
    drop(connId)
    await invoke('cm_close', { connId })
  }

  async function answerRing(connId: number, ok: boolean) {
    rings = rings.filter((r) => r.conn_id !== connId)
    await invoke('cm_reconfirm_result', { connId, ok })
  }

  function nameOf(connId: number): string {
    const c = connected.find((x) => x.conn_id === connId)
    return c ? c.name || c.peer_id : `Sessão ${connId}`
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
      listen<PairOffer>(CM_EVENTS.pairOffer, (e) => addOffer(e.payload)),
      listen<Identity>(CM_EVENTS.identity, (e) => {
        identities = { ...identities, [e.payload.conn_id]: e.payload }
      }),
      listen<Reconfirm>(CM_EVENTS.reconfirm, (e) => {
        const r = e.payload
        rings = [...rings.filter((x) => x.conn_id !== r.conn_id), { ...r, start: Date.now() }]
      }),
    ]
    // Events sent before the listeners existed are not replayed: hydrate from the backend.
    Promise.all(unlisten)
      .then(() => invoke<CmSnapshot>('cm_pending'))
      .then((snap) => {
        const t = Date.now()
        for (const { age_ms, ...p } of snap.pending) {
          if (!pending.some((x) => x.conn_id === p.conn_id)) {
            pending = [...pending, { ...p, start: t - age_ms }]
          }
        }
        for (const c of snap.connected) {
          if (!connected.some((x) => x.conn_id === c.conn_id)) connected = [...connected, c]
        }
        for (const o of snap.offers) if (!offers[o.conn_id]) addOffer(o)
        for (const i of snap.identities) {
          if (!identities[i.conn_id]) identities = { ...identities, [i.conn_id]: i }
        }
        for (const { age_ms, ...r } of snap.reconfirms) {
          if (!rings.some((x) => x.conn_id === r.conn_id)) rings = [...rings, { ...r, start: t - age_ms }]
        }
      })
      .catch((e) => console.error('cm_pending', e))
    const timer = setInterval(() => {
      now = Date.now()
      // The engine closes an unanswered prompt itself; just drop the card at 0.
      pending = pending.filter((p) => remainingSeconds(p.start, now) > 0)
      rings = rings.filter((r) => remainingSeconds(r.start, now, r.ring_secs) > 0)
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
  {#each rings as r (r.conn_id)}
    <ReconfirmCard
      name={nameOf(r.conn_id)}
      start={r.start}
      ringSecs={r.ring_secs}
      {now}
      onanswer={(ok) => answerRing(r.conn_id, ok)}
    />
  {/each}
  {#each pending as p (p.conn_id)}
    {@const notice = identityNotice(identities[p.conn_id], p.name || p.peer_id)}
    <div class="card">
      <strong>Pedido de acesso{p.is_file_transfer ? ' (arquivos)' : ''}</strong>
      <div>ID: {p.peer_id}</div>
      <div>Nome: {p.name}</div>
      {#if notice}
        <p class={notice.level === 'loud' ? 'warn loud' : 'unverified'}>{notice.text}</p>
      {/if}
      {#if canRemember(identities[p.conn_id], offers[p.conn_id]) && remember[p.conn_id]}
        {@const o = offers[p.conn_id]}
        {#if o.key_changed}
          <p class="warn">A chave desta máquina mudou. Só marque "lembrar" se tiver certeza.</p>
        {/if}
        <label>
          <input type="checkbox" bind:checked={remember[p.conn_id].on} />
          Lembrar este contato
        </label>
        {#if remember[p.conn_id].on}
          <PolicySelect
            value={remember[p.conn_id].policy}
            onchange={(key) => (remember[p.conn_id].policy = key)}
          />
          {#if o.key_changed}
            <label>
              <input type="checkbox" bind:checked={remember[p.conn_id].replaceKey} />
              Substituir chave guardada
            </label>
          {/if}
        {/if}
      {/if}
      {#if errors[p.conn_id]}<p class="warn">{errors[p.conn_id]}</p>{/if}
      <div class="actions">
        <button onclick={() => accept(p)} disabled={busy[p.conn_id]}>Aceitar</button>
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
  label {
    display: block;
    margin-top: 4px;
  }
  .warn {
    color: #b00;
    margin: 4px 0;
  }
  .loud {
    font-weight: bold;
    border: 2px solid #b00;
    padding: 4px;
  }
  .unverified {
    color: #8a5a00;
    margin: 4px 0;
  }
</style>
