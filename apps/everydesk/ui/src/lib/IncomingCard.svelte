<script lang="ts">
  // Accessed side: accept/deny prompts (60 s, then the engine closes the connection with
  // "Pedido não respondido em 60 s"). The authorized sessions go to the `sessions` store
  // (host/badge.ts), which ConnectedBadge shows; these listeners are its only source.
  // A prompt with a `cm:pair_offer` can also remember the controller under a policy; an
  // authorized session may get a `cm:reconfirm` ring (ReconfirmCard). `cm:identity` says
  // whether the claimed id/name is backed by a verified key; "lembrar" is never offered for
  // an unverified one, and a known id without its key gets a loud warning.
  // Accessibility: every new card is said by an assertive live region (mounted before any
  // card exists) and takes the focus on its safe button (`data-autofocus`: Recusar/Encerrar),
  // unless the person is typing (cards.ts mayTakeFocus); F6 always brings the focus to the
  // newest card. When the focused card goes, the focus returns to where it was before.
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'
  import { DUR } from '../design/motion'
  import {
    CLICK_APPROVE_TIMEOUT_SECS,
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
  import {
    REMIND_AT_SECS,
    expire,
    mayTakeFocus,
    reminderAnnouncement,
    rememberDefault,
    requestAnnouncement,
    ringAnnouncement,
    stackOffset,
    stackOrder,
  } from './cards/cards'
  import { applyCm, sessions } from './host/badge'

  type Pending = Incoming & { start: number }
  type Ring = Reconfirm & { start: number }
  /** Per-prompt answers of the "lembrar" controls. `touched`: the owner flipped "lembrar",
   *  so a late `cm:identity` no longer changes its default. */
  type Remember = { on: boolean; policy: string; replaceKey: boolean; touched: boolean }

  let pending = $state<Pending[]>([])
  let offers = $state<Record<number, PairOffer>>({})
  let identities = $state<Record<number, Identity>>({})
  /** Per-prompt error of a failed accept (the card stays up). */
  let errors = $state<Record<number, string>>({})
  let busy = $state<Record<number, boolean>>({})
  let remember = $state<Record<number, Remember>>({})
  let rings = $state<Ring[]>([])
  let now = $state(Date.now())

  // ---- the stack: one list, oldest nearest the corner; the DOM lists it top to bottom ----
  type Item = { key: string; ring: Ring; req?: undefined } | { key: string; req: Pending; ring?: undefined }
  const items = $derived<Item[]>([
    ...rings.map((ring) => ({ key: `r${ring.conn_id}`, ring })),
    ...pending.map((req) => ({ key: `p${req.conn_id}`, req })),
  ])
  const ordered = $derived(stackOrder(items))

  // ---- announcements and focus ----
  let stackEl: HTMLElement | undefined = $state()
  let announcement = $state('')
  const announced = new Set<string>()
  const reminded = new Set<string>()
  /** Where the focus was before a card took it; it goes back there when the cards are gone. */
  let returnTo: HTMLElement | null = null
  /** The focus is in the stack (kept when it vanishes with a removed card). */
  let focusInside = false

  function say(text: string) {
    // Clear first so the same words are said again.
    announcement = ''
    tick().then(() => (announcement = text))
  }

  function nameOfItem(i: Item): string {
    return i.req ? i.req.name || i.req.peer_id : nameOf(i.ring.conn_id)
  }

  function focusCard(key: string | undefined, force: boolean) {
    if (!key || !stackEl) return
    const btn = stackEl.querySelector<HTMLElement>(`[data-card="${key}"] [data-autofocus]`)
    if (!btn) return
    const active = document.activeElement as HTMLElement | null
    if (!force && !mayTakeFocus(active)) return
    if (!focusInside && active && active !== document.body && !stackEl.contains(active)) returnTo = active
    btn.focus()
  }

  // A new card: say it, and take the focus to it when nobody is typing.
  $effect(() => {
    const keys = new Set(items.map((i) => i.key))
    for (const k of [...announced]) {
      if (!keys.has(k)) {
        announced.delete(k)
        reminded.delete(k)
      }
    }
    const fresh = items.filter((i) => !announced.has(i.key))
    if (!fresh.length) return
    for (const i of fresh) announced.add(i.key)
    const last = fresh[fresh.length - 1]
    const t = Date.now()
    // Several at once (hydration after a restart): say them all, the newest last.
    say(
      fresh
        .map((i) =>
          i.req
            ? requestAnnouncement(nameOfItem(i), i.req.is_file_transfer, remainingSeconds(i.req.start, t))
            : ringAnnouncement(nameOfItem(i), remainingSeconds(i.ring.start, t, i.ring.ring_secs)),
        )
        .join(' '),
    )
    tick().then(() => focusCard(last.key, false))
  })

  // A card left: if the focus went with it, hand it to the next card or back where it was.
  let count = 0
  $effect(() => {
    const n = items.length
    const shrank = n < count
    count = n
    if (!shrank || !focusInside) return
    setTimeout(() => {
      const a = document.activeElement
      if (a && a !== document.body && a.isConnected) return
      if (ordered.length) focusCard(ordered[0].item.key, true)
      else {
        focusInside = false
        if (returnTo?.isConnected) returnTo.focus()
        returnTo = null
      }
    }, DUR.fast + 50)
  })

  function onfocusin() {
    focusInside = true
  }
  function onfocusout(e: FocusEvent) {
    const to = e.relatedTarget as Node | null
    // null: the window lost focus or the focused card was removed; keep the claim.
    if (to && !stackEl?.contains(to)) {
      focusInside = false
      returnTo = null
    }
  }

  function remind(t: number) {
    for (const i of items) {
      if (reminded.has(i.key)) continue
      const total = i.req ? CLICK_APPROVE_TIMEOUT_SECS : i.ring.ring_secs
      const left = i.req ? remainingSeconds(i.req.start, t) : remainingSeconds(i.ring.start, t, i.ring.ring_secs)
      if (total > REMIND_AT_SECS && left > 0 && left <= REMIND_AT_SECS) {
        reminded.add(i.key)
        say(reminderAnnouncement(nameOfItem(i), left))
      }
    }
  }

  // F6: the keyboard way to the newest card, wherever the focus is (even the remote screen).
  function onkeydowncapture(e: KeyboardEvent) {
    if (e.key !== 'F6' || !ordered.length) return
    e.preventDefault()
    e.stopImmediatePropagation()
    focusCard(ordered[0].item.key, true)
  }

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
    sessions.update((l) => applyCm(l, { type: 'removed', conn_id: connId }))
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
    if (!$sessions.some((c) => c.conn_id === p.conn_id)) {
      sessions.update((l) => applyCm(l, { type: 'connected', session: { conn_id: p.conn_id, peer_id: p.peer_id, name: p.name } }))
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
    const c = $sessions.find((x) => x.conn_id === connId)
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
        sessions.update((l) => applyCm(l, { type: 'connected', session: c }))
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
          if (!$sessions.some((x) => x.conn_id === c.conn_id)) sessions.update((l) => applyCm(l, { type: 'connected', session: c }))
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
      remind(now)
    }, 250)
    window.addEventListener('keydown', onkeydowncapture, true)
    return () => {
      window.removeEventListener('keydown', onkeydowncapture, true)
      clearInterval(timer)
      unlisten.forEach((u) => u.then((f) => f()))
    }
  })
</script>

<!-- Bottom-right corner. Cards stack upward from the corner (oldest nearest it), each one
     step further in (stackOffset), so two requests never read as one. The DOM lists them top
     to bottom (stackOrder), so Tab follows what the eye sees. -->
<div class="sr-only" aria-live="assertive" aria-atomic="true">{announcement}</div>
<div class="cm" bind:this={stackEl} {onfocusin} {onfocusout}>
  {#each ordered as { item, corner } (item.key)}
    <div class="slot" data-card={item.key}>
      {#if item.ring}
        {@const r = item.ring}
        <ReconfirmCard
          name={nameOf(r.conn_id)}
          start={r.start}
          ringSecs={r.ring_secs}
          {now}
          offset={stackOffset(corner)}
          onanswer={(ok) => answerRing(r.conn_id, ok)}
        />
      {:else if item.req}
        {@const p = item.req}
        {@const o = offers[p.conn_id]}
        <AccessCard
          name={p.name || p.peer_id}
          code={p.peer_id}
          isFileTransfer={p.is_file_transfer}
          remaining={remainingSeconds(p.start, now)}
          offset={stackOffset(corner)}
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
      {/if}
    </div>
  {/each}
</div>

<style>
  /* The scroll container takes the pointer (so its scrollbar works when many cards pile
     up); it is only as large as the cards, so it covers nothing else. */
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
    overscroll-behavior: contain;
    font-family: var(--font-ui);
  }
  .cm:empty {
    display: none;
  }
  .slot {
    display: flex;
    justify-content: flex-end;
  }
</style>
