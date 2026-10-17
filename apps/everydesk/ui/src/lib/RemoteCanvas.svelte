<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import { DUR, EASE, animate } from '../design/motion.ts'
  import type { ConnEvent } from './connecting/connecting.ts'
  import SessionPill from './session/SessionPill.svelte'
  import DropZone from './session/DropZone.svelte'
  import {
    ALT_IDLE,
    altTapReducer,
    isDeadZone,
    nearTopEdge,
    shouldForwardKey,
    type AltEvent,
  } from './session/sessionPill.ts'
  import Button from './ui/Button.svelte'
  import Field from './ui/Field.svelte'
  import { tick } from 'svelte'
  import {
    EVENTS,
    FRAME_HEADER_LEN,
    MouseButton,
    MouseEvt,
    clientToDisplay,
    close,
    connect,
    domButton,
    frameAck,
    keyName,
    login,
    mouseMask,
    parseFrameHeader,
    sendKey,
    sendMouse,
    accumulateWheel,
    currentDisplay,
    displayList,
    mergeQuality,
    quality,
    resetSessionState,
    type DisplayRect,
    type Modifiers,
    type MsgBox,
    type Quality,
  } from './session'

  interface Props {
    peerId: string
    password: string
    onclosed: (lastMessage: MsgBox | null) => void
    /** Session events before the first frame, for the connecting screen (moment 2). */
    onevent?: (e: ConnEvent) => void
    /** The reveal finished: the connecting screen underneath can go. */
    onrevealed?: () => void
  }
  let { peerId, password, onclosed, onevent, onrevealed }: Props = $props()

  let canvas: HTMLCanvasElement
  let ctx: CanvasRenderingContext2D | null = null
  // Plain (non-reactive) copies of the shared stores, read on every pointer event.
  let displays: DisplayRect[] = []
  let current = 0
  let connected = $state(false)
  let msg = $state<MsgBox | null>(null)
  let pill = $state<SessionPill>()
  let retryPassword = $state('')
  // Hidden (clipped to nothing) until the first frame; until then the connecting screen
  // shows the progress and any message.
  let revealed = $state(false)
  let root: HTMLElement
  let dialog: HTMLElement | undefined = $state()
  const uid = $props.id()
  // Fallback ImageData (one per display) for packets whose stride has padding.
  const images = new Map<number, ImageData>()

  const needsPassword = $derived(
    msg !== null && (msg.kind === 'input-password' || msg.kind === 're-input-password'),
  )

  // Dev-only frame-time probe around putImageData (docs/design/latencia-etapa3.md).
  const probe = import.meta.env.DEV ? { n: 0, sum: 0, max: 0 } : null

  function onFrame(buf: ArrayBuffer) {
    const h = parseFrameHeader(buf)
    if (h && ctx) {
      if (current !== h.display) currentDisplay.set((current = h.display))
      if (canvas.width !== h.width || canvas.height !== h.height) {
        canvas.width = h.width
        canvas.height = h.height
      }
      let img: ImageData
      if (h.stride === h.width * 4) {
        // Zero-copy: wrap the packet's pixels.
        img = new ImageData(
          new Uint8ClampedArray(buf, FRAME_HEADER_LEN, h.width * h.height * 4),
          h.width,
          h.height,
        )
      } else {
        let cached = images.get(h.display)
        if (!cached || cached.width !== h.width || cached.height !== h.height) {
          cached = new ImageData(h.width, h.height)
          images.set(h.display, cached)
        }
        const src = new Uint8Array(buf, FRAME_HEADER_LEN)
        for (let y = 0; y < h.height; y++) {
          cached.data.set(src.subarray(y * h.stride, y * h.stride + h.width * 4), y * h.width * 4)
        }
        img = cached
      }
      if (probe) {
        const t0 = performance.now()
        ctx.putImageData(img, 0, 0)
        const dt = performance.now() - t0
        probe.n++
        probe.sum += dt
        probe.max = Math.max(probe.max, dt)
        if (probe.n === 120) {
          console.debug(`[frame] putImageData avg ${(probe.sum / 120).toFixed(2)} ms, max ${probe.max.toFixed(2)} ms`)
          probe.n = probe.sum = probe.max = 0
        }
      } else {
        ctx.putImageData(img, 0, 0)
      }
      if (!revealed) reveal()
      if (!connected) {
        connected = true
        if (msg && !needsPassword && msg.kind !== 'error') msg = null
      }
    }
    frameAck()
  }

  let revealReported = false
  /** Tell App the session view owns the screen (once): after the animation, or at once if
   *  the session ends during it, so its message reaches the live path, not the gone one. */
  function revealDone() {
    if (revealReported) return
    revealReported = true
    onrevealed?.()
  }

  /** First frame: open the remote screen from the centre (instant under reduced motion). */
  function reveal() {
    revealed = true
    onevent?.({ type: 'frame' })
    canvas.focus()
    const a = animate(
      root,
      [{ clipPath: 'circle(0% at 50% 50%)' }, { clipPath: 'circle(150% at 50% 50%)' }],
      { duration: DUR.slow, easing: EASE.out },
    )
    a.finished.then(revealDone, revealDone)
    // A hidden or throttled webview may not advance animations: never leave the screen clipped.
    setTimeout(() => a.playState !== 'finished' && a.finish(), DUR.slow + 300)
  }

  function mods(e: MouseEvent | KeyboardEvent): Modifiers {
    return { alt: e.altKey, ctrl: e.ctrlKey, shift: e.shiftKey, command: e.metaKey }
  }

  function toRemote(e: MouseEvent) {
    return clientToDisplay(
      e.clientX,
      e.clientY,
      canvas.getBoundingClientRect(),
      canvas.width,
      canvas.height,
      displays[current],
    )
  }

  let lastPos = { x: 0, y: 0 }
  // What the peer currently believes is pressed, so it can be released if this window
  // loses focus mid-press (otherwise the key/button sticks on the remote side).
  const heldButtons = new Set<number>()
  const heldKeys = new Set<string>()
  const wheelAcc = { x: 0, y: 0 }
  const NO_MODS: Modifiers = { alt: false, ctrl: false, shift: false, command: false }

  /** Pointer over the visible pill: this event must not reach the remote (Review Focus 5). */
  function inDeadZone(e: MouseEvent): boolean {
    return isDeadZone(e.clientX, e.clientY, pill?.hitRect() ?? null)
  }

  /** Release what the peer believes is pressed (only buttons it was told about). */
  function releaseButtons() {
    for (const b of heldButtons) {
      sendMouse(mouseMask(b, MouseEvt.up), lastPos.x, lastPos.y, NO_MODS)
    }
    heldButtons.clear()
  }
  function releaseKeys() {
    for (const name of heldKeys) sendKey(name, false, false, NO_MODS)
    heldKeys.clear()
  }

  function sendButton(b: number, down: boolean, e: MouseEvent) {
    if (b === MouseButton.none) return
    if (down) heldButtons.add(b)
    else if (!heldButtons.delete(b)) return // never told the peer it was down
    const p = toRemote(e) ?? lastPos
    sendMouse(mouseMask(b, down ? MouseEvt.down : MouseEvt.up), p.x, p.y, mods(e))
  }

  function onPointerMove(e: PointerEvent) {
    if (!revealed) return
    if (inDeadZone(e)) {
      // Only reachable with pointer capture (a drag that started on the screen): let go of it
      // there, so nothing over the pill is a drag on the remote.
      releaseButtons()
      if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId)
      return
    }
    if (nearTopEdge(e.clientY)) pill?.peek()
    // A second button pressed/released while another is held arrives as a pointermove.
    if (e.button >= 0) {
      const bit = e.button === 1 ? 4 : e.button === 2 ? 2 : 1 << e.button
      sendButton(domButton(e.button), (e.buttons & bit) !== 0, e)
    }
    const p = toRemote(e)
    if (!p) return
    lastPos = p
    sendMouse(mouseMask(MouseButton.none, MouseEvt.move), p.x, p.y, mods(e))
  }
  function onPointerDown(e: PointerEvent) {
    e.preventDefault()
    if (!revealed || inDeadZone(e)) return
    alt({ type: 'interrupt' })
    canvas.focus()
    // Capture so the matching pointerup arrives even if released outside the window.
    canvas.setPointerCapture(e.pointerId)
    sendButton(domButton(e.button), true, e)
  }
  function onPointerUp(e: PointerEvent) {
    e.preventDefault()
    if (!revealed || inDeadZone(e)) return
    sendButton(domButton(e.button), false, e)
    if (e.buttons === 0 && canvas.hasPointerCapture(e.pointerId)) {
      canvas.releasePointerCapture(e.pointerId)
    }
  }
  function onWheel(e: WheelEvent) {
    e.preventDefault()
    if (!revealed || inDeadZone(e)) return
    alt({ type: 'interrupt' })
    const s = accumulateWheel(wheelAcc, e.deltaX, e.deltaY, e.deltaMode)
    const mask = mouseMask(MouseButton.none, MouseEvt.wheel)
    for (let i = 0; i < Math.abs(s.x); i++) sendMouse(mask, Math.sign(s.x), 0, mods(e))
    for (let i = 0; i < Math.abs(s.y); i++) sendMouse(mask, 0, Math.sign(s.y), mods(e))
  }

  // Bare Alt taps summon the pill and never reach the remote (sessionPill.ts altTapReducer).
  let altState = ALT_IDLE
  /** Feeds the Alt machine; sends any held-back Alt; returns whether the event goes on. */
  function alt(ev: AltEvent): boolean {
    const r = altTapReducer(altState, ev)
    altState = r.state
    for (const k of r.send) {
      const name = keyName(k.code, 'Alt')
      if (!name) continue
      if (k.down) heldKeys.add(name)
      else heldKeys.delete(name)
      sendKey(name, k.down, false, { ...NO_MODS, alt: k.down })
    }
    if (r.summon) queueMicrotask(() => pill?.summon())
    return r.forward
  }

  function onKey(e: KeyboardEvent, down: boolean) {
    const isAlt = e.key === 'Alt'
    if (!down) {
      if (!alt({ type: 'up', alt: isAlt, t: e.timeStamp })) {
        if (isAlt) e.preventDefault()
        return
      }
      const name = keyName(e.code, e.key)
      // A release always goes out for a key the peer holds, wherever focus went meanwhile.
      if (!name || !heldKeys.delete(name)) return
      e.preventDefault()
      sendKey(name, false, false, mods(e))
      return
    }
    const target = e.target as Node | null
    const ok = shouldForwardKey({
      revealed,
      canvasFocused: target === canvas || target === document.body,
      pillFocused: !!pill?.owns(target),
      overlayOpen: msg !== null,
    })
    // Typing in the pill or a password field: never a tap, never forwarded.
    if (!ok) {
      alt({ type: 'reset' })
      return
    }
    if (!alt({ type: 'down', alt: isAlt, code: e.code, repeat: e.repeat, t: e.timeStamp })) {
      e.preventDefault()
      return
    }
    const name = keyName(e.code, e.key)
    if (!name) return
    e.preventDefault()
    heldKeys.add(name)
    sendKey(name, true, false, mods(e))
  }

  /** Window lost focus: send "up" for everything the peer still holds. */
  function releaseAll() {
    releaseKeys()
    releaseButtons()
    alt({ type: 'reset' })
    wheelAcc.x = 0
    wheelAcc.y = 0
  }

  function disconnect() {
    close()
    onclosed(null)
  }

  function submitPassword(e: SubmitEvent) {
    e.preventDefault()
    if (!retryPassword) return
    login(retryPassword)
    retryPassword = ''
    closeMessage()
  }

  /** The message over the session goes; the keyboard returns to the remote screen. */
  function closeMessage() {
    msg = null
    tick().then(() => canvas.focus())
  }

  // A message over a live session is a modal dialog: it takes the focus (the password field,
  // or OK) as it opens, releases what the peer still holds, and nothing typed reaches the
  // remote until it closes (shouldForwardKey overlayOpen).
  $effect(() => {
    if (!revealed || !msg || !dialog) return
    releaseKeys()
    alt({ type: 'reset' })
    const d = dialog
    tick().then(() => d.querySelector<HTMLElement>('input, button')?.focus())
  })

  function dialogKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      // A notice closes; a password prompt cancels (that ends the attempt, as "Cancelar").
      if (needsPassword) disconnect()
      else closeMessage()
      return
    }
    if (e.key !== 'Tab' || !dialog) return
    // Keep Tab inside the dialog.
    const stops = [...dialog.querySelectorAll<HTMLElement>('input, button:not([disabled])')]
    if (!stops.length) return
    const first = stops[0]
    const last = stops[stops.length - 1]
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault()
      first.focus()
    }
  }

  onMount(() => {
    ctx = canvas.getContext('2d')
    resetSessionState()
    let lastMsg: MsgBox | null = null
    const unlisten: Promise<UnlistenFn>[] = [
      listen<DisplayRect[]>(EVENTS.displays, (e) => {
        displays = e.payload
        displayList.set(e.payload)
        if (!revealed) onevent?.({ type: 'displays' })
      }),
      listen(EVENTS.connected, () => {
        connected = true
        if (!revealed) onevent?.({ type: 'connected' })
        if (msg && !needsPassword && msg.kind !== 'error') msg = null
      }),
      listen<MsgBox>(EVENTS.msgbox, (e) => {
        lastMsg = e.payload
        if (revealed) msg = e.payload
        else onevent?.({ type: 'msgbox', box: e.payload })
      }),
      listen<Quality>(EVENTS.quality, (e) => {
        quality.update((prev) => mergeQuality(prev, e.payload))
      }),
      listen(EVENTS.closed, () => {
        if (revealed) revealDone()
        onclosed(lastMsg)
      }),
    ]
    const down = (e: KeyboardEvent) => onKey(e, true)
    const up = (e: KeyboardEvent) => onKey(e, false)
    const hidden = () => document.hidden && releaseAll()
    window.addEventListener('keydown', down)
    window.addEventListener('keyup', up)
    window.addEventListener('blur', releaseAll)
    document.addEventListener('visibilitychange', hidden)
    // Listeners are registered before connecting so no early event is lost.
    Promise.all(unlisten)
      .then(() => connect(peerId, password, onFrame))
      .catch((err) => {
        lastMsg = { kind: 'error', title: 'Erro', text: String(err) }
        onclosed(lastMsg)
      })
    return () => {
      window.removeEventListener('keydown', down)
      window.removeEventListener('keyup', up)
      window.removeEventListener('blur', releaseAll)
      document.removeEventListener('visibilitychange', hidden)
      unlisten.forEach((p) => p.then((f) => f()))
    }
  })
</script>

<div class="remote" class:revealed bind:this={root}>
  <!-- Not focusable until the reveal: hidden under the connecting screen it must not take
       Tab (reveal() focuses it programmatically). -->
  <canvas
    bind:this={canvas}
    tabindex={revealed ? 0 : -1}
    onpointermove={onPointerMove}
    onpointerdown={onPointerDown}
    onpointerup={onPointerUp}
    onwheel={onWheel}
    oncontextmenu={(e) => e.preventDefault()}
  ></canvas>

  {#if revealed}
    <SessionPill
      bind:this={pill}
      {peerId}
      onend={disconnect}
      onreturn={() => canvas.focus()}
      onenter={releaseKeys}
    />
    <DropZone {peerId} onactivity={() => pill?.peek()} />
  {/if}

  {#if revealed && msg}
    <div class="scrim" aria-hidden="true"></div>
    <div
      class="overlay"
      bind:this={dialog}
      role="dialog"
      aria-modal="true"
      aria-labelledby="{uid}-title"
      aria-describedby={msg.text ? `${uid}-text` : undefined}
      tabindex="-1"
      onkeydown={dialogKeydown}
    >
      <h2 id="{uid}-title" class="t-lead">{msg.title}</h2>
      {#if msg.text}<p id="{uid}-text" class="t-body muted">{msg.text}</p>{/if}
      {#if needsPassword}
        <form onsubmit={submitPassword}>
          <Field label="Senha" type="password" autocomplete="current-password" bind:value={retryPassword} />
          <div class="row">
            <Button variant="primary" type="submit">Entrar</Button>
            <Button onclick={disconnect}>Cancelar</Button>
          </div>
        </form>
      {:else}
        <div class="row">
          {#if connected}<Button onclick={closeMessage}>OK</Button>{:else}<Button onclick={disconnect}>Encerrar</Button>{/if}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .remote {
    position: fixed;
    inset: 0;
    background: #111;
    clip-path: circle(0% at 50% 50%);
  }
  .remote.revealed {
    clip-path: circle(150% at 50% 50%);
  }
  canvas {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    outline: none;
    cursor: default;
    touch-action: none;
  }
  .overlay {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 4;
    display: grid;
    gap: var(--space-3);
    justify-items: center;
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow-float);
    padding: var(--space-5) var(--space-6);
    border-radius: var(--radius-lg);
    max-width: 26rem;
    text-align: center;
  }
  .overlay h2,
  .overlay p {
    margin: 0;
  }
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 3;
    background: rgb(0 0 0 / 0.35);
  }
  .overlay:focus {
    outline: none;
  }
  .muted {
    color: var(--text-muted);
  }
  .overlay form {
    display: grid;
    gap: var(--space-4);
    width: 100%;
    text-align: start;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-3);
  }
</style>
