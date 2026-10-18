<!--
  UpdateCard — the discreet "Nova versão x.y.z. Atualizar agora?" card at the bottom of the home
  screen (Etapa 5, Task 7). Shows only when the app found a signed update (`update_status`,
  kept current by `update:status`); never on macOS/dev builds or without the updater key.
  No accent here (the accent belongs to "Conectar"): the version in the code face, a neutral
  main button, a ghost "Depois". With a session open, the first click asks again
  ("Encerrar e atualizar"). While downloading, one hairline along the bottom edge shows the
  progress. It never takes the focus by itself; the text is announced politely.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import Button from '../ui/Button.svelte'
  import {
    initialUpdateCard,
    SESSION_ACTIVE,
    UPDATE_EVENT,
    updateCardView,
    updateReducer,
    type UpdateAction,
    type UpdateStatus,
  } from './update.ts'

  let card = $state(initialUpdateCard)
  const view = $derived(updateCardView(card))
  let sending = $state(false)
  const dispatch = (a: UpdateAction) => (card = updateReducer(card, a))

  $effect(() => {
    invoke<UpdateStatus>('update_status')
      .then((status) => dispatch({ type: 'status', status }))
      .catch((e) => console.warn('everydesk: update_status', e))
    const off = listen<UpdateStatus>(UPDATE_EVENT, (e) => dispatch({ type: 'status', status: e.payload }))
    return () => {
      off.then((f) => f())
    }
  })

  async function primary() {
    if (!view.primary || sending) return
    sending = true
    try {
      // On success the app closes (setup or exe swap) and opens again updated.
      await invoke('update_install', { force: view.primary.force })
    } catch (e) {
      const msg = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
      dispatch(msg === SESSION_ACTIVE ? { type: 'session-active' } : { type: 'error', message: msg })
    } finally {
      sending = false
    }
  }

  function secondary() {
    if (card.confirming) {
      dispatch({ type: 'cancel' })
      return
    }
    invoke('update_dismiss').catch(() => {})
    dispatch({ type: 'later' })
  }
</script>

{#if view.visible}
  <section class="update" data-tone={view.tone} aria-labelledby="update-title" aria-busy={view.busy || undefined}>
    <div class="copy" aria-live="polite">
      <p id="update-title" class="title">
        {#if card.status?.phase === 'available' && !card.confirming && !card.error}
          Nova versão <span class="v">{card.status.version}</span>.
        {:else}
          {view.title}
        {/if}
      </p>
      <p class="body">{view.body}</p>
    </div>
    {#if view.primary || view.secondary}
      <div class="actions">
        {#if view.secondary}<Button variant="ghost" onclick={secondary}>{view.secondary}</Button>{/if}
        {#if view.primary}
          <Button variant={view.tone === 'warn' ? 'danger' : 'neutral'} loading={sending} onclick={primary}
            >{view.primary.label}</Button
          >
        {/if}
      </div>
    {/if}
    {#if view.busy}
      <span
        class="track"
        role="progressbar"
        aria-label="Progresso da atualização"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={view.progress ?? undefined}
        style:--p={view.progress == null ? null : `${view.progress}%`}
      ></span>
    {/if}
  </section>
{/if}

<style>
  .update {
    position: fixed;
    left: 50%;
    bottom: var(--space-4);
    z-index: 5;
    display: grid;
    gap: var(--space-3);
    width: min(440px, calc(100vw - 2 * var(--space-4)));
    box-sizing: border-box;
    overflow: hidden;
    padding: var(--space-4) var(--space-4) var(--space-4) var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-md);
    background: var(--surface-glass);
    backdrop-filter: blur(18px);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-family: var(--font-ui);
    transform: translateX(-50%);
    animation: rise var(--dur-base) var(--ease-out) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translate(-50%, 8px);
    }
  }
  .copy {
    display: grid;
    gap: var(--space-1);
  }
  p {
    margin: 0;
  }
  .title {
    font-size: var(--fs-body);
    font-weight: 600;
    line-height: var(--lh-snug);
    letter-spacing: var(--track-title);
  }
  .v {
    font-family: var(--font-code);
    font-weight: 500;
    letter-spacing: 0;
  }
  .body {
    font-size: var(--fs-label);
    line-height: var(--lh-body);
    color: var(--text-muted);
  }
  [data-tone='warn'] .title {
    color: var(--warn);
  }
  [data-tone='danger'] .title {
    color: var(--danger);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-2);
  }
  /* one hairline of progress along the bottom edge; without a size, a steady quarter */
  .track {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    background: var(--line);
  }
  .track::after {
    content: '';
    position: absolute;
    inset: 0 auto 0 0;
    width: var(--p, 25%);
    background: var(--text-muted);
    transition: width var(--dur-base) var(--ease-out);
  }
</style>
