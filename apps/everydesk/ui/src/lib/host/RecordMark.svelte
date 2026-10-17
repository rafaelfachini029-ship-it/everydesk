<!--
  RecordMark — accessed side, one per ConnectedBadge (spec §5.3: the recording is visible on
  both sides). While the controller records this session the badge reads "está gravando" with
  the same red lamp the controller sees in its pill: the same tape, seen from both ends. It
  goes when the controller stops, and with the badge when the session drops (Review Focus 4:
  `cm:removed` takes the whole row). Said once to screen readers on each change.
  State: `cm:record {conn_id, on}` for this connection, hydrated from `cm_pending().records`.
  Props:
    conn: Connected — the session this badge stands for
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'
  import type { Connected } from '../cm.ts'
  import { badgeLabel } from './badge.ts'
  import { RECORD_EVENTS, hostRecordSay, hydrateRecord, type CmRecord } from '../session/record.ts'

  interface Props {
    conn: Connected
  }
  let { conn }: Props = $props()

  let on = $state(false)
  let heard = false
  let announcement = $state('')

  onMount(() => {
    const un = listen<CmRecord>(RECORD_EVENTS.cm, (e) => {
      if (e.payload.conn_id !== conn.conn_id) return
      heard = true
      set(e.payload.on)
    })
    un.then(() => invoke<{ records?: CmRecord[] }>('cm_pending'))
      .then((snap) => {
        // an event that came first wins over the snapshot
        if (!heard) set(hydrateRecord(snap?.records, conn.conn_id))
      })
      .catch((e) => console.error('cm_pending (gravação)', e))
    return () => un.then((f) => f())
  })

  function set(next: boolean) {
    if (next === on) return
    on = next
    announcement = ''
    tick().then(() => (announcement = hostRecordSay(badgeLabel(conn), next)))
  }
</script>

<span class="sr-only" aria-live="polite" aria-atomic="true">{announcement}</span>
{#if on}
  <span class="rec">
    <span class="lamp" aria-hidden="true"></span>
    <span class="label">está gravando</span>
  </span>
{/if}

<style>
  .rec {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-1) 0 var(--space-3);
    border-left: 1px solid var(--line);
    color: var(--danger);
    font-weight: 600;
    white-space: nowrap;
  }
  .lamp {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-soft);
    animation: breathe 1.6s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.35;
      box-shadow: 0 0 0 1px var(--danger-soft);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .lamp {
      animation: none;
    }
  }
</style>
