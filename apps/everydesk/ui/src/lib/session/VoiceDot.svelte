<!--
  VoiceDot — the voice light in the collapsed pill (voice.ts `voice` store). A status light,
  so it may carry the accent: off is a hollow ring, "chamando…" a ring that rings (a slow
  ripple; a still ring under reduced motion), on a steady filled light.
-->
<script lang="ts">
  import { dotLabel, voice } from './voice.ts'
</script>

<span class="voice {$voice.state}" role="img" aria-label={dotLabel($voice.state)}></span>

<style>
  .voice {
    position: relative;
    flex: none;
    width: 8px;
    height: 8px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 1.5px solid var(--line-strong);
    transition:
      background-color var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out);
  }
  .voice.waiting {
    border-color: var(--accent);
  }
  .voice.waiting::after {
    content: '';
    position: absolute;
    inset: -1.5px;
    border-radius: 50%;
    border: 1.5px solid var(--accent);
    animation: ring 1.4s var(--ease-out) infinite;
  }
  @keyframes ring {
    from {
      opacity: 0.9;
      transform: scale(1);
    }
    to {
      opacity: 0;
      transform: scale(2.6);
    }
  }
  .voice.on {
    border-color: var(--accent);
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-track);
  }
  @media (prefers-reduced-motion: reduce) {
    .voice.waiting::after {
      animation: none;
      opacity: 0;
    }
  }
</style>
