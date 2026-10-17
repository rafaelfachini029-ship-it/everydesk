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
    type CmSnapshot,
    type Connected,
    type Identity,
    type Incoming,
    type PairOffer,
    type Reconfirm,
    type Removed,
  } from './cm'
  import { policyFromKey } from './policy'
  import ReconfirmCard from './ReconfirmCard.svelte'
  import AccessCard from './cards/AccessCard.svelte'
  import { expire, rememberDefault, stackOffset } from './cards/cards'
  import Button from './ui/Button.svelte'

  type Pending = Incoming & { start: number }
  type Ring = Reconfirm & { start: number }
  /** Per-prompt answers of the "lembrar" controls. `touched`: the owner flipped "lembrar",
   *  so a late `cm:identity` no longer changes its default. */
  type Remember = { on: boolean; policy: string; replaceKey: boolean; touched: boolean }

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
      // Default ON only for a verified identity without a key change (cards.ts).
      remember = {
        ...remember,
        [o.conn_id]: {
          on: rememberDefault(identities[o.conn_id], o.key_changed),
          policy: 'ask',
          replaceKey: false,
          touched: false,
        },
      }
    }
  }

  function addIdentity(i: Identity) {
    identities = { ...identities, [i.conn_id]: i }
    // The identity may arrive after the offer: recompute the default until the owner answers.
    const o = offers[i.conn_id]
    const r = remember[i.conn_id]
    if (o && r && !r.touched) {
      remember = { ...remember, [i.conn_id]: { ...r, on: rememberDefault(i, o.key_changed) } }
    }
  }

  function setRemember(connId: number, patch: Partial<Remember>) {
    const r = remember[connId]
    if (r) remember = { ...remember, [connId]: { ...r, ...patch } }
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
          // Only over a key change, and only after its own explicit toggle.
          replaceKey: !!offers[p.conn_id]?.key_changed && r.replaceKey,
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
      listen<Identity>(CM_EVENTS.identity, (e) => addIdentity(e.payload)),
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
        for (const i of snap.identities) if (!identities[i.conn_id]) addIdentity(i)
        for (const o of snap.offers) if (!offers[o.conn_id]) addOffer(o)
        for (const { age_ms, ...r } of snap.reconfirms) {
          if (!rings.some((x) => x.conn_id === r.conn_id)) rings = [...rings, { ...r, start: t - age_ms }]
        }
      })
      .catch((e) => console.error('cm_pending', e))
    const timer = setInterval(() => {
      now = Date.now()
      // The engine closes an unanswered prompt itself; just drop the card at 0.
      pending = expire(pending, now)
      rings = expire(rings, now)
    }, 250)
    return () => {
      clearInterval(timer)
      unlisten.forEach((u) => u.then((f) => f()))
    }
  })
</script>

<!-- Bottom-right corner. Cards stack upward from the corner (oldest nearest it), each one
     step further in (stackOffset), so two requests never read as one. -->
<div class="cm">
  <div class="stack">
    {#each rings as r, i (`r${r.conn_id}`)}
      <ReconfirmCard
        name={nameOf(r.conn_id)}
        start={r.start}
        ringSecs={r.ring_secs}
        {now}
        offset={stackOffset(i)}
        onanswer={(ok) => answerRing(r.conn_id, ok)}
      />
    {/each}
    {#each pending as p, i (`p${p.conn_id}`)}
      {@const o = offers[p.conn_id]}
      <AccessCard
        name={p.name || p.peer_id}
        code={p.peer_id}
        isFileTransfer={p.is_file_transfer}
        remaining={remainingSeconds(p.start, now)}
        offset={stackOffset(rings.length + i)}
        identity={identities[p.conn_id]}
        rememberable={canRemember(identities[p.conn_id], o)}
        keyChanged={!!o?.key_changed}
        remember={remember[p.conn_id]}
        error={errors[p.conn_id]}
        busy={busy[p.conn_id]}
        onremember={(on) => setRemember(p.conn_id, { on, touched: true, replaceKey: false })}
        onpolicy={(policy) => setRemember(p.conn_id, { policy })}
        onreplacekey={(replaceKey) => setRemember(p.conn_id, { replaceKey })}
        onaccept={() => accept(p)}
        ondeny={() => close(p.conn_id)}
      />
    {/each}
  </div>
  {#each connected as c (c.conn_id)}
    <div class="connected t-label">
      <span>Conectado: <strong>{c.name || c.peer_id}</strong></span>
      <Button variant="danger" onclick={() => close(c.conn_id)}>Encerrar</Button>
    </div>
  {/each}
</div>

<style>
  .cm {
    position: fixed;
    right: var(--space-4);
    bottom: var(--space-4);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-3);
    max-height: calc(100vh - 2 * var(--space-4));
    overflow-y: auto;
    font-family: var(--font-ui);
    pointer-events: none;
  }
  .stack {
    display: flex;
    flex-direction: column-reverse;
    align-items: flex-end;
    gap: var(--space-3);
  }
  .stack > :global(*),
  .connected {
    pointer-events: auto;
  }
  .connected {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: var(--surface-glass);
    -webkit-backdrop-filter: blur(20px);
    backdrop-filter: blur(20px);
    box-shadow: var(--shadow-float);
    color: var(--text);
  }
  .connected :global(.btn.danger:not(:hover)) {
    background: transparent;
  }
</style>
