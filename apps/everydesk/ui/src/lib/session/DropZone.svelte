<!--
  DropZone — files dragged over the session (spec §6.2 moment 5).
  While files are dragged over the window the remote screen dims a little and a viewfinder
  marks the whole screen as the destination ("Solte para enviar para Mãe"). A drop starts the
  "em breve" trail inside the pill (FileTrail); Stage 3 reads and sends nothing.
  The layer is `pointer-events: none` at all times: drag events still reach the window
  through the canvas, and idle pointer input (canvas, pill dead zone) is never touched.
  Listens to both sources of drags: the DOM (browser, or Tauri with dragDropEnabled off)
  and Tauri's webview drag-drop events (the default in the app, which swallows the DOM ones).
  Props:
    peerId: string
    onactivity?: () => void — keep the pill shown while the trail runs (peek)
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onDestroy, onMount } from 'svelte'
  import ArrowDownToLine from 'lucide-svelte/icons/arrow-down-to-line'
  import Icon from '../ui/Icon.svelte'
  import type { Contact } from '../policy.ts'
  import {
    TRAIL_FILL_MS,
    TRAIL_NOTE_MS,
    dragDepth,
    dropTarget,
    hasFiles,
    overlayActive,
    trail,
    type DragStep,
  } from './dropzone.ts'

  interface Props {
    peerId: string
    onactivity?: () => void
  }
  let { peerId, onactivity }: Props = $props()

  let name = $state<string | null>(null)
  let depth = $state(0)
  let native = $state(false)
  const on = $derived(overlayActive(depth) || native)
  const label = $derived(dropTarget(name, peerId))

  let timers: ReturnType<typeof setTimeout>[] = []
  let lastDrop = 0

  function step(s: DragStep) {
    depth = dragDepth(depth, s)
  }

  /** Visual only: the trail fills, the note shows, then everything fades. No file is read. */
  function dropped() {
    const t = Date.now()
    if (t - lastDrop < 300) return // the same drop seen by both sources
    lastDrop = t
    timers.forEach(clearTimeout)
    trail.set('fill')
    onactivity?.()
    timers = [
      setTimeout(() => trail.set('note'), TRAIL_FILL_MS),
      // the pill hides 2 s after the last peek: keep it up until the note is read
      ...[1000, 2000, 3000].map((ms) => setTimeout(() => onactivity?.(), ms)),
      setTimeout(() => trail.set('idle'), TRAIL_FILL_MS + TRAIL_NOTE_MS),
    ]
  }

  function ondragenter(e: DragEvent) {
    if (!hasFiles(e.dataTransfer)) return
    e.preventDefault()
    step('enter')
  }
  function ondragover(e: DragEvent) {
    if (!hasFiles(e.dataTransfer)) return
    // required for the drop to land here instead of the webview opening the file
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
  }
  function ondragleave(e: DragEvent) {
    if (hasFiles(e.dataTransfer)) step('leave')
  }
  function ondrop(e: DragEvent) {
    if (!hasFiles(e.dataTransfer)) return
    e.preventDefault()
    step('drop')
    dropped()
  }
  const reset = () => step('reset')

  onMount(() => {
    invoke<Contact[]>('contacts_list')
      .then((list) => (name = list?.find((c) => c.peer_id === peerId)?.name ?? null))
      .catch(() => {})
    window.addEventListener('dragenter', ondragenter)
    window.addEventListener('dragover', ondragover)
    window.addEventListener('dragleave', ondragleave)
    window.addEventListener('drop', ondrop)
    window.addEventListener('dragend', reset)
    window.addEventListener('blur', reset)

    let unlistenNative: (() => void) | null = null
    let gone = false
    import('@tauri-apps/api/webview')
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload
          if (p.type === 'enter') native = p.paths.length > 0
          else if (p.type === 'leave') native = false
          else if (p.type === 'drop') {
            native = false
            if (p.paths.length > 0) dropped()
          }
        }),
      )
      .then((u) => (gone ? u() : (unlistenNative = u)))
      .catch(() => {}) // plain browser (dev mock): DOM events only

    return () => {
      gone = true
      unlistenNative?.()
      window.removeEventListener('dragenter', ondragenter)
      window.removeEventListener('dragover', ondragover)
      window.removeEventListener('dragleave', ondragleave)
      window.removeEventListener('drop', ondrop)
      window.removeEventListener('dragend', reset)
      window.removeEventListener('blur', reset)
    }
  })
  onDestroy(() => {
    timers.forEach(clearTimeout)
    trail.set('idle')
  })
</script>

<div class="drop" class:on aria-hidden="true">
  <span class="corner tl"></span>
  <span class="corner tr"></span>
  <span class="corner bl"></span>
  <span class="corner br"></span>
  <p class="target">
    <Icon icon={ArrowDownToLine} size={22} />
    <span>{label}</span>
  </p>
</div>
<p class="sr" role="status">{on ? label : ''}</p>

<style>
  /* Never a target: drags reach the window through the canvas, pointers never stop here. */
  .drop {
    --ink: #eef2f8; /* the veil is dark in both themes */
    --arm: 36px;
    --inset: var(--space-6);
    position: absolute;
    inset: 0;
    z-index: 1;
    pointer-events: none;
    background: rgb(4 6 12 / 0.42);
    opacity: 0;
    visibility: hidden;
    transition:
      opacity var(--dur-base) var(--ease-out),
      visibility 0s linear var(--dur-base);
  }
  .drop.on {
    opacity: 1;
    visibility: visible;
    transition:
      opacity var(--dur-base) var(--ease-out),
      visibility 0s linear 0s;
  }

  /* A viewfinder: the whole remote screen is the destination. The corners close in from
     just outside their resting place, saying "here". */
  .corner {
    position: absolute;
    width: var(--arm);
    height: var(--arm);
    border: 0 solid var(--ink);
    transition: transform var(--dur-slow) var(--ease-out);
  }
  .tl {
    top: var(--inset);
    left: var(--inset);
    border-width: 2px 0 0 2px;
    border-top-left-radius: var(--radius-sm);
    transform: translate(-10px, -10px);
  }
  .tr {
    top: var(--inset);
    right: var(--inset);
    border-width: 2px 2px 0 0;
    border-top-right-radius: var(--radius-sm);
    transform: translate(10px, -10px);
  }
  .bl {
    bottom: var(--inset);
    left: var(--inset);
    border-width: 0 0 2px 2px;
    border-bottom-left-radius: var(--radius-sm);
    transform: translate(-10px, 10px);
  }
  .br {
    bottom: var(--inset);
    right: var(--inset);
    border-width: 0 2px 2px 0;
    border-bottom-right-radius: var(--radius-sm);
    transform: translate(10px, 10px);
  }
  .on .corner {
    transform: none;
  }

  .target {
    position: absolute;
    top: 50%;
    left: 50%;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    max-width: calc(100% - 2 * var(--space-8));
    margin: 0;
    padding: var(--space-3) var(--space-5) var(--space-3) var(--space-4);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-size: var(--fs-lead);
    font-weight: 500;
    letter-spacing: var(--track-title);
    white-space: nowrap;
    transform: translate(-50%, calc(-50% + 8px));
    transition: transform var(--dur-slow) var(--ease-out);
  }
  .target span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .on .target {
    transform: translate(-50%, -50%);
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    margin: 0;
  }
</style>
