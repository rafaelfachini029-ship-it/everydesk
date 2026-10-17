<!--
  HostStack — the one top-right stack every host card renders through (voice card, elevation
  card, later ones). Mounted once in App. A card joins with `use:hostCard={key}` (exported
  below): its node moves into this stack (out of any transformed parent, e.g. a badge), in
  arrival order, each one a `stackOffset` step further in than the one above it
  (host/hostStack.ts). Apart from the access-card stack (bottom right) and the session badges
  (bottom left), so nothing covers anything.
-->
<script module lang="ts">
  import { get, writable } from 'svelte/store'
  import { addCard, hostCards, placement, removeCard } from './hostStack.ts'

  const root = writable<HTMLElement | null>(null)

  /** Moves `node` into the stack under `key` until it is destroyed. */
  export function hostCard(node: HTMLElement, key: string) {
    hostCards.update((l) => addCard(l, key))
    const unRoot = root.subscribe((r) => {
      if (r && node.parentElement !== r) r.appendChild(node)
    })
    const unPlace = hostCards.subscribe((l) => {
      const p = placement(l, key)
      node.style.marginRight = p ? `${p.inset}px` : ''
    })
    if (!get(root)) document.body.appendChild(node)
    return {
      destroy() {
        unRoot()
        unPlace()
        hostCards.update((l) => removeCard(l, key))
        node.remove()
      },
    }
  }
</script>

<script lang="ts">
  import { onMount } from 'svelte'

  let el: HTMLDivElement
  onMount(() => {
    root.set(el)
    return () => root.set(null)
  })
</script>

<div class="host-stack" bind:this={el}></div>

<style>
  .host-stack {
    position: fixed;
    top: var(--space-4);
    right: var(--space-4);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-3);
    max-height: calc(100vh - 2 * var(--space-4));
    overflow: visible;
    pointer-events: none;
  }
  .host-stack:empty {
    display: none;
  }
  .host-stack > :global(*) {
    pointer-events: auto;
  }
</style>
