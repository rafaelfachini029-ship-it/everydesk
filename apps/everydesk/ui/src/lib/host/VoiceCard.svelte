<!--
  VoiceCard — accessed side, one per ConnectedBadge (spec §5.3). Only the controller can start
  a voice call; when it asks, a card in the access cards' family says "Fulano quer conversar
  por voz" with Recusar / Aceitar. The disc ripples like the controller's "chamando…" dot: the
  same call, seen from both ends. While the call is on, the badge itself reads "voz ligada"
  with "Encerrar voz" beside it (this machine's microphone is going out: it must stay visible).
  The card does not live in the badge (whose entry transform would trap a fixed card): it is
  moved to <body> and sits in the top-right corner, apart from the access-card stack (bottom
  right) and the badges (bottom left). Focus, as for the access cards (Stage 3 rule): it goes
  to the safe button (Recusar) unless the owner is typing, and an assertive live region says
  the request either way; when the card goes, the focus goes back where it was.
  State: voice.ts `hostVoice`, fed by `cm:voice` for this connection and hydrated from
  `cm_pending().voices`.
  Props:
    conn: Connected — the session this badge stands for
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'
  import Mic from 'lucide-svelte/icons/mic'
  import '../../design/tokens.css'
  import '../../design/typography.css'
  import Button from '../ui/Button.svelte'
  import Icon from '../ui/Icon.svelte'
  import type { Connected } from '../cm.ts'
  import { initials, mayTakeFocus } from '../cards/cards.ts'
  import { badgeLabel } from './badge.ts'
  import {
    VOICE_BODY,
    VOICE_EVENTS,
    hostVoice,
    hydrateVoice,
    voiceTitle,
    type CmVoice,
    type HostVoice,
  } from '../session/voice.ts'

  interface Props {
    conn: Connected
  }
  let { conn }: Props = $props()

  const uid = $props.id()
  let call = $state<HostVoice>('none')
  let busy = $state<'accept' | 'refuse' | 'close' | ''>('')
  let error = $state('')
  let announcement = $state('')
  let card: HTMLDivElement | undefined = $state()
  let returnTo: HTMLElement | null = null

  const who = $derived(badgeLabel(conn))

  onMount(() => {
    const un = listen<CmVoice>(VOICE_EVENTS.cm, (e) => {
      if (e.payload.conn_id === conn.conn_id) set(hostVoice(e.payload))
    })
    un.then(() => invoke<{ voices?: CmVoice[] }>('cm_pending'))
      .then((snap) => {
        // an event that came first wins over the snapshot
        if (call === 'none') set(hydrateVoice(snap?.voices, conn.conn_id))
      })
      .catch((e) => console.error('cm_pending (voz)', e))
    return () => {
      un.then((f) => f())
      giveFocusBack()
    }
  })

  function set(next: HostVoice) {
    if (next === call) return
    const was = call
    call = next
    busy = ''
    error = ''
    if (next === 'incoming') arrive()
    else if (was === 'incoming') giveFocusBack()
    if (next === 'active') say(`Voz ligada com ${who}. O microfone desta máquina está ligado.`)
    else if (was === 'active') say(`Voz com ${who} encerrada.`)
  }

  function say(text: string) {
    announcement = ''
    tick().then(() => (announcement = text))
  }

  async function arrive() {
    say(`${voiceTitle(who)}. Aceitar ou recusar.`)
    await tick()
    const safe = card?.querySelector<HTMLElement>('[data-autofocus]')
    const active = document.activeElement as HTMLElement | null
    if (!safe || !mayTakeFocus(active)) return
    if (active && active !== document.body) returnTo = active
    safe.focus()
  }

  function giveFocusBack() {
    const to = returnTo
    returnTo = null
    if (to?.isConnected && (!document.activeElement || document.activeElement === document.body)) to.focus()
  }

  async function answer(accept: boolean) {
    if (busy) return
    busy = accept ? 'accept' : 'refuse'
    error = ''
    try {
      await invoke('cm_voice_answer', { connId: conn.conn_id, accept })
    } catch (e) {
      busy = ''
      error = `Não deu para responder: ${e}`
      return
    }
    // A refusal is final here; an accept waits for `cm:voice` (on) from the engine.
    if (!accept) set('none')
  }

  async function hangUp() {
    if (busy) return
    busy = 'close'
    try {
      await invoke('cm_voice_close', { connId: conn.conn_id })
      set('none')
    } catch {
      busy = ''
      error = 'Não consegui encerrar a voz. Tente de novo.'
    }
  }

  /** Moves the card to <body>, out of the badge's transformed box. */
  function toBody(node: HTMLElement) {
    document.body.appendChild(node)
    return { destroy: () => node.remove() }
  }
</script>

<div class="sr-only" aria-live="assertive" aria-atomic="true">{announcement}</div>

{#if call === 'active'}
  <span class="on" role="status">
    <span class="light" aria-hidden="true"></span>
    <span class="label">voz ligada</span>
  </span>
  <span class="hang"><Button variant="ghost" loading={busy === 'close'} onclick={hangUp}>Encerrar voz</Button></span>
  {#if error}<span class="err" role="alert">{error}</span>{/if}
{/if}

{#if call === 'incoming'}
  <div
    use:toBody
    bind:this={card}
    class="card"
    role="alertdialog"
    aria-modal="false"
    aria-labelledby="t-{uid}"
    aria-describedby="b-{uid}"
  >
    <header>
      <span class="disc" aria-hidden="true">
        <span class="letters">{initials(who)}</span>
        <span class="badge-mic"><Icon icon={Mic} size={14} /></span>
      </span>
      <h2 id="t-{uid}" class="title">{voiceTitle(who)}</h2>
    </header>
    <p id="b-{uid}" class="body t-body">{VOICE_BODY}</p>
    {#if error}<p class="error t-label" role="alert">{error}</p>{/if}
    <div class="actions">
      <Button variant="ghost" data-autofocus loading={busy === 'refuse'} onclick={() => answer(false)}>Recusar</Button>
      <Button variant="primary" icon={Mic} loading={busy === 'accept'} onclick={() => answer(true)}>Aceitar</Button>
    </div>
  </div>
{/if}

<style>
  /* In the badge: the voice is a status, so its light may carry the accent. */
  .on {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-1) 0 var(--space-3);
    border-left: 1px solid var(--line);
    white-space: nowrap;
  }
  .light {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-track);
  }
  .label {
    color: var(--text);
    font-weight: 600;
  }
  /* a quiet ghost in the badge, like its Encerrar: a frame only on hover */
  .hang {
    display: contents;
  }
  .hang :global(.btn:not(:hover)) {
    background: transparent;
    border-color: transparent;
  }
  .err {
    padding-right: var(--space-3);
    color: var(--danger);
    white-space: nowrap;
  }

  .card {
    position: fixed;
    top: var(--space-4);
    right: var(--space-4);
    z-index: 1000;
    width: min(400px, calc(100vw - 2 * var(--space-4)));
    box-sizing: border-box;
    display: grid;
    gap: var(--space-4);
    padding: var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-glass);
    -webkit-backdrop-filter: blur(20px) saturate(1.2);
    backdrop-filter: blur(20px) saturate(1.2);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-family: var(--font-ui);
    animation: arrive var(--dur-base) var(--ease-out) both;
  }
  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(-16px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
  header {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: var(--space-4);
  }
  /* The access cards' initials disc; it rings while the call waits for an answer. */
  .disc {
    position: relative;
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .disc::before,
  .disc::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1.5px solid var(--accent);
    opacity: 0;
    animation: ring 2.4s var(--ease-out) infinite;
  }
  .disc::after {
    animation-delay: 1.2s;
  }
  @keyframes ring {
    from {
      opacity: 0.8;
      transform: scale(1);
    }
    to {
      opacity: 0;
      transform: scale(1.45);
    }
  }
  .badge-mic {
    position: absolute;
    right: -2px;
    bottom: -2px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
    background: var(--surface);
    color: var(--text);
  }
  .title {
    margin: 0;
    font-size: var(--fs-lead);
    font-weight: 600;
    line-height: var(--lh-tight);
    letter-spacing: var(--track-title);
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
  }
  .body {
    color: var(--text);
    line-height: var(--lh-snug);
  }
  .error {
    color: var(--danger);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  @media (prefers-reduced-motion: reduce) {
    .disc::before {
      animation: none;
      opacity: 0.6;
    }
    .disc::after {
      animation: none;
    }
  }
</style>
