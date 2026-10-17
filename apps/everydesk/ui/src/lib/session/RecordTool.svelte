<!--
  RecordTool — the pill's "Gravar" tool (spec §5.3, gravação com um clique). One click starts
  the engine's recorder; while it records the tool turns into a stop button in the recording
  red (danger, never the accent: the accent means "connected", red means "on tape"). The
  running time is not repeated here: it sits on the lamp in the pill's summary (RecordLamp),
  which stays in sight while recording. A click stops
  it and "Abrir pasta" stays beside the tool for 8 s, opening the recordings folder; the pill
  stays down meanwhile (`onattention`).
  The state follows `session:record`, not the click: a host that does not allow recording
  makes the engine drop the start silently. That case is visible before the click: the tool
  reads "A outra máquina não permite gravar" and does nothing.
  State: record.ts `recording` store, fed by `session:record`; off on `session:closed`
  (Review Focus 4) and when the session view goes.
  Props:
    onreturn: () => void — give the keyboard back to the remote screen
    onhint: (text: string | null) => void — the pill's hover hint
    onattention: (on: boolean) => void — keep the pill down / let it go
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import CircleDot from 'lucide-svelte/icons/circle-dot'
  import FolderOpen from 'lucide-svelte/icons/folder-open'
  import Icon from '../ui/Icon.svelte'
  import { extras } from '../session'
  import {
    OFFER_MS,
    RECORD_EVENTS,
    RECORD_PERMISSION,
    RECORD_SAVED,
    dispatchRecord,
    recordHint,
    recording,
  } from './record.ts'

  interface Props {
    onreturn: () => void
    onhint: (text: string | null) => void
    onattention: (on: boolean) => void
  }
  let { onreturn, onhint, onattention }: Props = $props()

  const NOTICE_MS = 4000

  let root: HTMLSpanElement
  let pointed = $state(false)
  let held = false
  let offerTimer: ReturnType<typeof setTimeout> | null = null
  let noticeTimer: ReturnType<typeof setTimeout> | null = null

  const on = $derived($recording.on)
  const denied = $derived($extras.denied.includes(RECORD_PERMISSION))
  const hint = $derived(recordHint(on, denied))
  const off = $derived(denied && !on)

  $effect(() => {
    if (pointed) onhint(hint)
  })

  onMount(() => {
    dispatchRecord({ type: 'reset' })
    const uns = [
      listen<{ on: boolean }>(RECORD_EVENTS.session, (e) => dispatchRecord({ type: 'event', on: !!e.payload?.on, at: Date.now() })),
      listen(RECORD_EVENTS.closed, () => dispatchRecord({ type: 'closed' })),
    ]
    // A webview that mounts mid-recording (reload) asks once.
    invoke<boolean>('session_is_recording')
      .then((r) => r && dispatchRecord({ type: 'event', on: true, at: Date.now() }))
      .catch(() => {})
    return () => {
      uns.forEach((u) => u.then((f) => f()))
      clear()
      dispatchRecord({ type: 'reset' })
    }
  })

  // Saved: keep the pill down for "Abrir pasta", then let it go.
  $effect(() => {
    if (!$recording.saved) return
    hold()
    if (offerTimer) clearTimeout(offerTimer)
    offerTimer = setTimeout(() => {
      offerTimer = null
      dispatchRecord({ type: 'offered' })
    }, OFFER_MS)
  })
  // A note (a failed command): same, for a shorter while.
  $effect(() => {
    if (!$recording.notice) return
    hold()
    if (noticeTimer) clearTimeout(noticeTimer)
    noticeTimer = setTimeout(() => {
      noticeTimer = null
      dispatchRecord({ type: 'seen' })
    }, NOTICE_MS)
  })
  // Both gone: release the pill unless the person is on it.
  $effect(() => {
    if ($recording.saved || $recording.notice || !held) return
    held = false
    const dock = root?.closest('.dock')
    if (dock && (dock.matches(':hover') || dock.contains(document.activeElement))) return
    onattention(false)
  })

  function hold() {
    if (held) return
    held = true
    onattention(true)
  }

  function clear() {
    if (offerTimer) clearTimeout(offerTimer)
    if (noticeTimer) clearTimeout(noticeTimer)
    offerTimer = noticeTimer = null
  }

  async function click() {
    if (off) return onreturn()
    const start = !on
    try {
      await invoke('session_record', { start })
    } catch (e) {
      dispatchRecord({ type: 'failed', message: `Não deu para ${start ? 'gravar' : 'parar a gravação'}: ${e}` })
    }
  }

  async function openFolder() {
    dispatchRecord({ type: 'offered' })
    try {
      await invoke('open_recordings_dir')
    } catch (e) {
      dispatchRecord({ type: 'failed', message: String(e) })
    }
    onreturn()
  }
</script>

<span class="recordtool" bind:this={root}>
  <button
    class="tool"
    class:on
    aria-label={hint}
    aria-pressed={on}
    aria-disabled={off ? 'true' : undefined}
    onclick={click}
    onpointerenter={() => (pointed = true)}
    onfocus={() => (pointed = true)}
    onpointerleave={() => ((pointed = false), onhint(null))}
    onblur={() => (pointed = false)}
  >
    {#if on}
      <span class="stop" aria-hidden="true"></span>
    {:else}
      <Icon icon={CircleDot} />
    {/if}
  </button>
  {#if $recording.saved}
    <button class="open" onclick={openFolder} onpointerenter={() => onhint(RECORD_SAVED)} onpointerleave={() => onhint(null)}>
      <Icon icon={FolderOpen} size={16} />
      <span>Abrir pasta</span>
    </button>
  {/if}
  <span class="sr-only" role="status">{$recording.saved ? RECORD_SAVED : ''}{$recording.notice ?? ''}</span>
  {#if $recording.notice}
    <span class="note" aria-hidden="true">{$recording.notice}</span>
  {/if}
</span>

<style>
  .recordtool {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .tool,
  .open {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    height: 40px;
    border: 1px solid transparent;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }
  .tool {
    min-width: 40px;
    padding: 0;
  }
  .tool:hover:not([aria-disabled='true']),
  .open:hover {
    background: var(--surface-2);
  }
  .tool:focus-visible,
  .open:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .tool[aria-disabled='true'] {
    color: var(--text-muted);
    cursor: not-allowed;
  }

  /* Recording: a stop square in the recording red. */
  .tool.on {
    border-color: var(--danger);
    background: var(--danger-soft);
    color: var(--danger);
  }
  .tool.on:hover {
    background: var(--danger-soft);
    border-color: var(--danger);
  }
  .stop {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: currentColor;
  }

  .open {
    padding: 0 var(--space-3) 0 10px;
    border-color: var(--line-strong);
    font-size: var(--fs-label);
    white-space: nowrap;
    animation: in var(--dur-base) var(--ease-out);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateX(-4px);
    }
  }

  .note {
    position: absolute;
    top: calc(100% + var(--space-2));
    left: 50%;
    z-index: 3;
    transform: translateX(-50%);
    padding: var(--space-2) var(--space-4);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-size: var(--fs-body);
    white-space: nowrap;
    pointer-events: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .open {
      animation: none;
    }
  }
</style>
