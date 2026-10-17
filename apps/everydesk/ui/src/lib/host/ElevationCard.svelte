<!--
  ElevationCard — accessed side, Windows portable run only: the controller needs
  administrator rights (a UAC window is blocking input, or they asked for it). Asked once per
  episode by the backend (cm.rs `show_elevation`); a remembered "Permitir sempre" never shows
  it, and on macOS or an installed EveryDesk no `cm:elevation` ever arrives.
  Same family as the access cards (glass shell, disc, large title), but no countdown: nothing
  ends if the owner waits. It sits in the top-right corner, apart from the access-card stack
  (bottom right) and the session badges (bottom left), so it never covers either.
  It does not take the focus (the owner may be typing; a polite live region says it).
  State: applyElevation (host/elevation.ts), fed by `cm:elevation`, `cm:removed` and
  `cm_elevation_pending` on mount.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import ShieldUser from 'lucide-svelte/icons/shield-user'
  import { onMount } from 'svelte'
  import { fade } from 'svelte/transition'
  import '../../design/tokens.css'
  import '../../design/typography.css'
  import { DUR, prefersReducedMotion } from '../../design/motion'
  import { CM_EVENTS, type Removed } from '../cm'
  import Button from '../ui/Button.svelte'
  import { hostCard } from './HostStack.svelte'
  import Icon from '../ui/Icon.svelte'
  import {
    ELEVATION_BODY,
    ELEVATION_EVENT,
    ELEVATION_TITLE,
    applyElevation,
    dismissElevation,
    elevate,
    type ElevationCardState,
    type ElevationEvent,
    type ElevationStep,
  } from './elevation'

  let card = $state<ElevationCardState>(null)
  /** Which button is in flight. */
  let busy = $state<'always' | 'once' | ''>('')
  let error = $state('')
  let announcement = $state('')

  function step(s: ElevationStep) {
    const before = card
    card = applyElevation(card, s)
    if (card !== before) {
      busy = ''
      error = ''
      announcement = card === null ? '' : `${ELEVATION_TITLE}. ${ELEVATION_BODY}`
    }
  }

  async function allow(remember: boolean) {
    if (card === null || busy) return
    busy = remember ? 'always' : 'once'
    error = ''
    try {
      await elevate(card, remember)
    } catch (e) {
      busy = ''
      error = `Não foi possível pedir a permissão: ${e}`
      return
    }
    step({ type: 'answered' })
  }

  async function dismiss(rememberNo: boolean) {
    if (card === null || busy) return
    step({ type: 'answered' })
    try {
      await dismissElevation(rememberNo)
    } catch (e) {
      console.error('cm_elevation_dismiss', e)
    }
  }

  onMount(() => {
    const unlisten: Promise<UnlistenFn>[] = [
      listen<ElevationEvent>(ELEVATION_EVENT, (e) => step({ type: 'event', event: e.payload })),
      listen<Removed>(CM_EVENTS.removed, (e) => step({ type: 'removed', conn_id: e.payload.conn_id })),
    ]
    Promise.all(unlisten)
      .then(() => invoke<number | null>('cm_elevation_pending'))
      .then((conn_id) => step({ type: 'hydrate', conn_id: conn_id ?? null }))
      .catch((e) => console.error('cm_elevation_pending', e))
    return () => unlisten.forEach((u) => u.then((f) => f()))
  })

  const uid = $props.id()
</script>

<div class="sr-only" aria-live="polite" aria-atomic="true">{announcement}</div>
{#if card !== null}
  <div
    use:hostCard={'elevation'}
    class="card"
    role="dialog"
    aria-modal="false"
    aria-labelledby="t-{uid}"
    aria-describedby="b-{uid}"
    out:fade|global={{ duration: prefersReducedMotion() ? 0 : DUR.fast }}
  >
    <header>
      <span class="disc"><Icon icon={ShieldUser} size={28} /></span>
      <h2 id="t-{uid}" class="title">{ELEVATION_TITLE}</h2>
    </header>
    <p id="b-{uid}" class="body t-body">{ELEVATION_BODY}</p>

    {#if error}<p class="error t-label" role="alert">{error}</p>{/if}

    <!-- the two ways to allow, then the two ways to decline (this time / for good) -->
    <div class="actions">
      <Button variant="ghost" loading={busy === 'once'} onclick={() => allow(false)}>Só desta vez</Button>
      <Button variant="primary" loading={busy === 'always'} onclick={() => allow(true)}>Permitir sempre</Button>
    </div>
    <div class="decline">
      <Button variant="ghost" onclick={() => dismiss(false)}>Agora não</Button>
      <button type="button" class="never t-label" onclick={() => dismiss(true)}>não perguntar mais</button>
    </div>
  </div>
{/if}

<style>
  /* placed by HostStack (top right, shared with the voice card) */
  .card {
    position: relative;
    width: min(440px, calc(100vw - 2 * var(--space-4) - 2 * 14px));
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
  /* from the top edge, where it lives */
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
  /* the access cards' disc, holding a shield instead of initials; no ring: no clock here */
  .disc {
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--text);
  }
  .title {
    margin: 0;
    font-size: var(--fs-lead);
    font-weight: 600;
    line-height: var(--lh-tight);
    letter-spacing: var(--track-title);
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
  .decline {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--line);
  }
  /* a quiet text link, in the text colour (no small grey text) */
  .never {
    padding: var(--space-1) var(--space-2);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .never:hover {
    text-decoration-thickness: 2px;
  }
  .never:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
