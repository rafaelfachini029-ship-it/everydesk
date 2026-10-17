<!--
  ConnectedBadge — accessed side: while someone is authorized on this machine, a small,
  persistent, quiet badge in the bottom-left corner says who, with "Encerrar" at hand.
  One badge per session, stacked upward from the corner (oldest nearest it). The accept
  cards own the bottom-right corner, so the two never cover each other.
  Files the controller sends (C1, host/files.ts): for NOTE_MS (6 s) the badge's verb becomes
  the transfer ("Mãe enviou foto.jpg (2,1 MB)"), then fades back to "está acessando"; a quiet
  counter keeps how many arrived this session.
  State: the `sessions` store (host/badge.ts), fed by IncomingCard's `cm:*` listeners and
  `cm_pending` hydration; its only own listener is `cm:file` (hydrated from `cm_pending().files`).
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import { fade } from 'svelte/transition'
  import FileDown from 'lucide-svelte/icons/file-down'
  import { DUR, prefersReducedMotion } from '../../design/motion'
  import Button from '../ui/Button.svelte'
  import Icon from '../ui/Icon.svelte'
  import HostChat from './HostChat.svelte'
  import VoiceCard from './VoiceCard.svelte'
  import RecordMark from './RecordMark.svelte'
  import { applyCm, badgeLabel, sessions } from './badge.ts'
  import {
    FILE_EVENT,
    NOTE_MS,
    NO_FILES,
    applyFile,
    currentNote,
    fileVerb,
    hydrateFiles,
    receivedLabel,
    type CmFile,
    type HostFiles,
  } from './files.ts'

  /** conn_id → its transfers (latest + count). */
  let files = $state<Record<number, HostFiles>>({})
  /** Re-read when a note may have faded. */
  let now = $state(Date.now())
  let fadeTimer: ReturnType<typeof setTimeout> | null = null

  function wakeAfter(ms: number) {
    if (fadeTimer) clearTimeout(fadeTimer)
    fadeTimer = setTimeout(() => (now = Date.now()), ms + 20)
  }

  onMount(() => {
    let heard = false
    const un = listen<CmFile>(FILE_EVENT, (e) => {
      heard = true
      const f = e.payload
      now = Date.now()
      files = { ...files, [f.conn_id]: applyFile(files[f.conn_id] ?? NO_FILES, f, now) }
      wakeAfter(NOTE_MS)
    })
    un.then(() => invoke<{ files?: CmFile[] }>('cm_pending'))
      .then((snap) => {
        if (heard) return // an event that came first is newer than the snapshot
        const t = Date.now()
        const byConn: Record<number, HostFiles> = {}
        for (const f of snap?.files ?? []) byConn[f.conn_id] ??= hydrateFiles(snap?.files, f.conn_id, t)
        files = byConn
        now = t
        wakeAfter(NOTE_MS)
      })
      .catch((e) => console.warn('everydesk: cm_pending (arquivos)', e))
    return () => {
      un.then((f) => f())
      if (fadeTimer) clearTimeout(fadeTimer)
    }
  })

  /** conn_id → why "Encerrar" failed; the badge stays (the access is still on). */
  let failed = $state<Record<number, string>>({})

  /** Close first; the badge goes only once the engine confirms, so a failed close never
   *  hides an access that is still on. */
  async function end(connId: number) {
    try {
      await invoke('cm_close', { connId })
    } catch {
      failed = { ...failed, [connId]: 'Não consegui encerrar. Tente de novo.' }
      return
    }
    if (connId in failed) {
      const rest = { ...failed }
      delete rest[connId]
      failed = rest
    }
    sessions.update((l) => applyCm(l, { type: 'removed', conn_id: connId }))
  }
</script>

<section class="badges" aria-label="Acessos a esta máquina" aria-live="polite">
  <!-- newest first in the DOM (top), oldest nearest the corner: Tab follows the eye -->
  {#each [...$sessions].reverse() as c (c.conn_id)}
    {@const named = !!c.name?.trim()}
    {@const got = files[c.conn_id] ?? NO_FILES}
    {@const note = currentNote(got, now)}
    <div class="badge" title={named ? c.peer_id : undefined}>
      <span class="live" aria-hidden="true"></span>
      <span class="who">
        <strong class:code={!named}>{badgeLabel(c)}</strong>
        {#key note ? `${note.id}-${note.state}` : ''}
          <span
            class="verb"
            class:file={!!note}
            class:bad={note?.state === 'error' || note?.state === 'cancelled'}
            in:fade={{ duration: prefersReducedMotion() ? 0 : DUR.base }}
            >{note ? fileVerb(note) : 'está acessando'}</span
          >
        {/key}
      </span>
      {#if got.received > 0}
        <span class="got t-num" title={receivedLabel(got.received)}>
          <Icon icon={FileDown} size={16} />{got.received}<span class="sr-only">{receivedLabel(got.received)}</span>
        </span>
      {/if}
      <RecordMark conn={c} />
      <HostChat conn={c} />
      <VoiceCard conn={c} />
      <Button variant="danger" onclick={() => end(c.conn_id)}>Encerrar</Button>
      {#if failed[c.conn_id]}<span class="err" role="alert">{failed[c.conn_id]}</span>{/if}
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
    flex-direction: column;
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
  .err {
    padding-right: var(--space-3);
    color: var(--danger);
    white-space: nowrap;
  }
  strong {
    font-weight: 600;
  }
  strong.code {
    font-family: var(--font-code);
    font-weight: 500;
  }
  /* small text stays in the text colour (no small grey text, spec §6.1) */
  .verb {
    color: var(--text);
  }
  .verb.bad {
    color: var(--danger);
  }
  /* what arrived this session: a quiet count, same weight as the rest of the badge */
  .got {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding-left: var(--space-3);
    border-left: 1px solid var(--line);
    color: var(--text);
    white-space: nowrap;
  }
  /* the ghost of a danger button: red text, fills only on hover */
  .badge :global(.btn.danger:not(:hover)) {
    background: transparent;
    border-color: transparent;
  }
</style>
