<!--
  FileTrail — the transfer progress inside the pill (spec §6.2 moment 5): a line of light
  drawn along the pill's lower edge. Stage 3 has no transfer yet, so after a drop it fills,
  says "Transferência de arquivos chega em breve" and fades. Reads the `trail` store that
  DropZone drives. Lives inside SessionPill's dock; never takes pointer input, and sits
  outside the flow so the dock's rect (the dead zone) stays the pill alone.
-->
<script lang="ts">
  import { TRAIL_FILL_MS, TRAIL_NOTE, trail } from './dropzone.ts'
</script>

<span
  class="trail"
  class:fill={$trail === 'fill'}
  class:note={$trail === 'note'}
  style:--fill-ms="{TRAIL_FILL_MS}ms"
  aria-hidden="true"><span class="bar"></span></span
>
<p class="note-text" class:on={$trail === 'note'} role="status">{$trail === 'note' ? TRAIL_NOTE : ''}</p>

<style>
  .trail {
    position: absolute;
    left: var(--space-5);
    right: var(--space-5);
    bottom: 0;
    height: 2px;
    border-radius: 1px;
    background: var(--accent-track);
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--dur-slow) var(--ease-out);
  }
  .trail.fill,
  .trail.note {
    opacity: 1;
  }
  .bar {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
    transform-origin: left center;
    transform: scaleX(0);
  }
  .fill .bar {
    animation: fill var(--fill-ms) cubic-bezier(0.4, 0, 0.2, 1) forwards;
  }
  .note .bar {
    transform: scaleX(1);
  }
  @keyframes fill {
    to {
      transform: scaleX(1);
    }
  }

  /* same place and voice as the pill's caption line */
  .note-text {
    position: absolute;
    top: 100%;
    margin: var(--space-2) 0 0;
    padding: 2px var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text);
    font-size: var(--fs-label);
    white-space: nowrap;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--dur-slow) var(--ease-out);
  }
  .note-text.on {
    opacity: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    .fill .bar {
      animation: none;
      transform: scaleX(1);
    }
  }
</style>
