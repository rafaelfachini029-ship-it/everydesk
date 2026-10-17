<!--
  VoiceTool — the pill's "Voz" tool (spec §5.3, voz com um clique). One click asks the other
  machine for a voice call (the tool reads "chamando…" until it answers), a click while
  calling cancels, a click while on hangs up. Unanswered for 60 s (voice.ts RING_TIMEOUT_MS)
  the call is cancelled and the note says "A outra máquina não atendeu." The voice dot in the collapsed pill (VoiceDot)
  follows the same state. A call that ends without the person asking (refused, hung up by
  the other side) is said once in a short note under the pill, which comes down for it
  (`onattention`), like the chat preview.
  No "who is speaking" light: the received audio plays inside the engine and never reaches
  the app (voice.ts).
  State: voice.ts `voice` store, fed by `session:voice`; reset when the session view goes.
  Props:
    onreturn: () => void — give the keyboard back to the remote screen
    onhint: (text: string | null) => void — the pill's hover hint
    onattention: (on: boolean) => void — keep the pill down / let it go
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import Mic from 'lucide-svelte/icons/mic'
  import Icon from '../ui/Icon.svelte'
  import { extras } from '../session'
  import { RING_TIMEOUT_MS, VOICE_EVENTS, clickIntent, dispatchVoice, voice, voiceHint, type VoiceEvent } from './voice.ts'

  interface Props {
    onreturn: () => void
    onhint: (text: string | null) => void
    onattention: (on: boolean) => void
  }
  let { onreturn, onhint, onattention }: Props = $props()

  /** How long a "recusou" / "encerrada" note stays under the pill. */
  const NOTICE_MS = 4000

  let root: HTMLSpanElement
  let timer: ReturnType<typeof setTimeout> | null = null
  /** Pointer or focus on the tool: its hint follows the state (chamando… → on). */
  let pointed = $state(false)

  const phase = $derived($voice.state)
  const denied = $derived($extras.denied.includes('audio'))
  const muted = $derived($extras.options['disable-audio'] === true)
  const hint = $derived(voiceHint(phase, { denied, muted }))
  const off = $derived(denied && phase === 'off')

  $effect(() => {
    if (pointed) onhint(hint)
  })

  onMount(() => {
    dispatchVoice({ type: 'reset' })
    const un = listen<VoiceEvent>(VOICE_EVENTS.session, (e) => dispatchVoice({ type: 'event', event: e.payload }))
    return () => {
      un.then((f) => f())
      endNotice(false)
      dispatchVoice({ type: 'reset' })
    }
  })

  // Ringing with no answer: give up after RING_TIMEOUT_MS ("A outra máquina não atendeu.").
  $effect(() => {
    if (phase !== 'waiting') return
    const t = setTimeout(() => {
      dispatchVoice({ type: 'timeout' })
      invoke('session_voice', { start: false }).catch((e) => console.warn('everydesk: voice cancel failed', e))
    }, RING_TIMEOUT_MS)
    return () => clearTimeout(t)
  })

  // A new note: bring the pill down for it, then let it go.
  $effect(() => {
    if (!$voice.notice) return
    if (timer) clearTimeout(timer)
    else onattention(true)
    timer = setTimeout(() => endNotice(true), NOTICE_MS)
  })

  function endNotice(release: boolean) {
    const had = timer !== null
    if (timer) clearTimeout(timer)
    timer = null
    if (!had) return
    dispatchVoice({ type: 'seen' })
    if (!release) return
    const dock = root.closest('.dock')
    if (dock && (dock.matches(':hover') || dock.contains(document.activeElement))) return
    onattention(false)
  }

  async function click() {
    if (off) return onreturn()
    const { start } = clickIntent(phase)
    dispatchVoice({ type: start ? 'start' : 'stop' })
    try {
      await invoke('session_voice', { start })
    } catch (e) {
      if (start) dispatchVoice({ type: 'failed', message: `Não deu para ligar a voz: ${e}` })
    }
  }
</script>

<span class="voicetool" bind:this={root}>
  <button
    class="tool {phase}"
    aria-label={hint}
    aria-pressed={phase !== 'off'}
    aria-disabled={off ? 'true' : undefined}
    onclick={click}
    onpointerenter={() => (pointed = true)}
    onfocus={() => (pointed = true)}
    onpointerleave={() => ((pointed = false), onhint(null))}
    onblur={() => (pointed = false)}
  >
    <Icon icon={Mic} />
    {#if phase === 'waiting'}<span class="calling" aria-hidden="true">chamando…</span>{/if}
  </button>
  <span class="sr-only" role="status">{$voice.notice ?? ''}</span>
  {#if $voice.notice}
    <span class="note" aria-hidden="true">{$voice.notice}</span>
  {/if}
</span>

<style>
  /* Not positioned: the note anchors to the pill's wrapper, outside the tool strip's clip. */
  .voicetool {
    display: inline-flex;
  }
  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    min-width: 40px;
    height: 40px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      padding var(--dur-base) var(--ease-out);
  }
  .tool:hover:not([aria-disabled='true']) {
    background: var(--surface-2);
  }
  .tool:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .tool[aria-disabled='true'] {
    color: var(--text-muted);
    cursor: not-allowed;
  }
  /* Calling: the tool says so in words, beside the microphone. */
  .tool.waiting {
    padding: 0 var(--space-3) 0 10px;
    border-color: var(--line-strong);
  }
  .calling {
    font-size: var(--fs-label);
    white-space: nowrap;
  }
  /* On: pressed, and the microphone carries the status light's colour. */
  .tool.on {
    border-color: var(--line-strong);
    background: var(--surface-2);
    color: var(--accent);
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
    animation: drop var(--dur-base) var(--ease-out);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translate(-50%, -6px);
    }
  }
</style>
