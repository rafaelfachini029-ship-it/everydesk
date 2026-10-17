<!--
  ConnectedBadge — accessed side: while someone is authorized on this machine, a small,
  persistent, quiet badge in the bottom-left corner says who, with "Encerrar" at hand.
  One badge per session, stacked upward from the corner (oldest nearest it). The accept
  cards own the bottom-right corner, so the two never cover each other.
  State: the `sessions` store (host/badge.ts), fed by IncomingCard's `cm:*` listeners and
  `cm_pending` hydration; this component adds no listeners of its own.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import Button from '../ui/Button.svelte'
  import { applyCm, badgeLabel, sessions } from './badge.ts'

  async function end(connId: number) {
    sessions.update((l) => applyCm(l, { type: 'removed', conn_id: connId }))
    await invoke('cm_close', { connId })
  }
</script>

<section class="badges" aria-label="Acessos a esta máquina" aria-live="polite">
  {#each $sessions as c (c.conn_id)}
    {@const named = !!c.name?.trim()}
    <div class="badge" title={named ? c.peer_id : undefined}>
      <span class="live" aria-hidden="true"></span>
      <span class="who">
        <strong class:code={!named}>{badgeLabel(c)}</strong>
        <span class="verb">está acessando</span>
      </span>
      <Button variant="danger" onclick={() => end(c.conn_id)}>Encerrar</Button>
    </div>
  {/each}
</section>

<style>
  .badges {
    position: fixed;
    left: var(--space-4);
    bottom: var(--space-4);
    z-index: 999;
    display: flex;
    flex-direction: column-reverse;
    align-items: flex-start;
    gap: var(--space-2);
    max-width: calc(100vw - 2 * var(--space-4));
    font-family: var(--font-ui);
    pointer-events: none;
  }
  .badge {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    max-width: 100%;
    padding: var(--space-1) var(--space-1) var(--space-1) var(--space-4);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-size: var(--fs-label);
    pointer-events: auto;
    animation: enter var(--dur-base) var(--ease-out) both;
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateX(-8px);
    }
  }
  /* status, so it may carry the accent; steady, not pulsing: quiet, not alarming */
  .live {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-track);
  }
  .who {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  strong {
    font-weight: 600;
  }
  strong.code {
    font-family: var(--font-code);
    font-weight: 500;
  }
  .verb {
    color: var(--text-muted);
  }
  /* the ghost of a danger button: red text, fills only on hover */
  .badge :global(.btn.danger:not(:hover)) {
    background: transparent;
    border-color: transparent;
  }
</style>
