<!--
  RecordLamp — the recording light in the collapsed pill (record.ts `recording` store): a
  tally lamp, like the red light on a camera that says "on tape". Only there while
  recording: a small red dot that breathes, and the running time beside it. Red is the
  danger colour, never the accent (the accent is the connection's own light). Under reduced
  motion the dot is steady. The host sees the same lamp on its badge (RecordMark).
-->
<script lang="ts">
  import { formatElapsed, lampLabel, recording } from './record.ts'

  let now = $state(Date.now())
  $effect(() => {
    if (!$recording.on) return
    now = Date.now()
    const t = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(t)
  })
  const ms = $derived(now - ($recording.since ?? now))
</script>

{#if $recording.on}
  <span class="rec" role="img" aria-label={lampLabel(ms)}>
    <span class="lamp" aria-hidden="true"></span>
    <span class="time t-num" aria-hidden="true">{formatElapsed(ms)}</span>
  </span>
{/if}

<style>
  .rec {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 6px;
    color: var(--danger);
    font-size: var(--fs-label);
    font-weight: 600;
  }
  .lamp {
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
