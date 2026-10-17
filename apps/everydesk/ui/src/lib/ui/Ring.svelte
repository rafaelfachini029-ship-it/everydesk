<!--
  Ring — countdown ring (spec §6.2 moment 4: "anel contando 60 segundos").
  Props:
    total: number — full duration (e.g. 60)
    remaining: number — time left, same unit; the accent arc shrinks with it
    size?: number = 56 (px)
    stroke?: number = 3 (px)
    name?: string — accessible name of the timer; defaults to "Tempo restante"
    label?: string — spoken value (aria-valuetext); defaults to "{remaining} segundos restantes"
  Exposed as a progressbar: aria-valuemin 0, aria-valuemax total, aria-valuenow the whole
  seconds left, so a screen reader can read the time on demand.
    children?: Snippet — centre content; defaults to the remaining number
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import { ringDashOffset, ringGeometry } from './ring'

  interface Props {
    total: number
    remaining: number
    size?: number
    stroke?: number
    name?: string
    label?: string
    children?: Snippet
  }
  let { total, remaining, size = 56, stroke = 3, name = 'Tempo restante', label, children }: Props = $props()

  const g = $derived(ringGeometry(size, stroke))
  const offset = $derived(ringDashOffset(total, remaining, g.c))
  const shown = $derived(Math.max(0, Math.ceil(remaining)))
</script>

<div
  class="ring"
  style:width="{size}px"
  style:height="{size}px"
  role="progressbar"
  aria-label={name}
  aria-valuemin={0}
  aria-valuemax={total}
  aria-valuenow={shown}
  aria-valuetext={label ?? `${shown} segundos restantes`}
>
  <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle class="track" cx={g.center} cy={g.center} r={g.r} stroke-width={stroke} />
    <circle
      class="arc"
      cx={g.center}
      cy={g.center}
      r={g.r}
      stroke-width={stroke}
      stroke-dasharray={g.c}
      stroke-dashoffset={offset}
      transform="rotate(-90 {g.center} {g.center})"
    />
  </svg>
  <span class="center t-num" aria-hidden="true">
    {#if children}{@render children()}{:else}{shown}{/if}
  </span>
</div>

<style>
  .ring {
    position: relative;
    display: inline-grid;
    place-items: center;
    flex: none;
  }
  svg {
    position: absolute;
    inset: 0;
  }
  circle {
    fill: none;
  }
  .track {
    stroke: var(--accent-track);
  }
  .arc {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset var(--dur-slow) var(--ease-out);
  }
  .center {
    position: relative;
    color: var(--text);
    font-size: var(--fs-label);
    font-weight: 600;
  }
</style>
