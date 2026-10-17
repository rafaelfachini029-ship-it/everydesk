<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import { DUR, EASE, animate } from '../design/motion.ts'
  import type { ConnEvent } from './connecting/connecting.ts'
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
    switchDisplay,
    accumulateWheel,
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
  let displays = $state<DisplayRect[]>([])
  let current = $state(0)
  let connected = $state(false)
  let msg = $state<MsgBox | null>(null)
  let quality = $state<Quality>({ fps: null, delay: null, speed: null })
  let retryPassword = $state('')
  // Hidden (clipped to nothing) until the first frame; until then the connecting screen
  // shows the progress and any message.
  let revealed = $state(false)
  let root: HTMLElement
  // Fallback ImageData (one per display) for packets whose stride has padding.
  const images = new Map<number, ImageData>()

  const needsPassword = $derived(
    msg !== null && (msg.kind === 'input-password' || msg.kind === 're-input-password'),
  )

  function onFrame(buf: ArrayBuffer) {
    const h = parseFrameHeader(buf)
    if (h && ctx) {
      current = h.display
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
      ctx.putImageData(img, 0, 0)
      if (!revealed) reveal()
      if (!connected) {
        connected = true
        if (msg && !needsPassword && msg.kind !== 'error') msg = null
      }
    }
    frameAck()
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
    a.finished.then(
      () => onrevealed?.(),
      () => onrevealed?.(),
    )
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

  function sendButton(b: number, down: boolean, e: MouseEvent) {
    if (b === MouseButton.none) return
    if (down) heldButtons.add(b)
    else if (!heldButtons.delete(b)) return // never told the peer it was down
    const p = toRemote(e) ?? lastPos
    sendMouse(mouseMask(b, down ? MouseEvt.down : MouseEvt.up), p.x, p.y, mods(e))
  }

  function onPointerMove(e: PointerEvent) {
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
    canvas.focus()
    // Capture so the matching pointerup arrives even if released outside the window.
    canvas.setPointerCapture(e.pointerId)
    sendButton(domButton(e.button), true, e)
  }
  function onPointerUp(e: PointerEvent) {
    e.preventDefault()
    sendButton(domButton(e.button), false, e)
    if (e.buttons === 0 && canvas.hasPointerCapture(e.pointerId)) {
      canvas.releasePointerCapture(e.pointerId)
    }
  }
  function onWheel(e: WheelEvent) {
    e.preventDefault()
    const s = accumulateWheel(wheelAcc, e.deltaX, e.deltaY, e.deltaMode)
    const mask = mouseMask(MouseButton.none, MouseEvt.wheel)
    for (let i = 0; i < Math.abs(s.x); i++) sendMouse(mask, Math.sign(s.x), 0, mods(e))
    for (let i = 0; i < Math.abs(s.y); i++) sendMouse(mask, 0, Math.sign(s.y), mods(e))
  }

  function onKey(e: KeyboardEvent, down: boolean) {
    if (!connected || e.target instanceof HTMLInputElement) return
    const name = keyName(e.code, e.key)
    if (!name) return
    e.preventDefault()
    if (down) heldKeys.add(name)
    else heldKeys.delete(name)
    sendKey(name, down, false, mods(e))
  }

  /** Window lost focus: send "up" for everything the peer still holds. */
  function releaseAll() {
    for (const name of heldKeys) sendKey(name, false, false, NO_MODS)
    heldKeys.clear()
    for (const b of heldButtons) {
      sendMouse(mouseMask(b, MouseEvt.up), lastPos.x, lastPos.y, NO_MODS)
    }
    heldButtons.clear()
    wheelAcc.x = 0
    wheelAcc.y = 0
  }

  function disconnect() {
    close()
    onclosed(null)
  }

  function submitPassword(e: SubmitEvent) {
    e.preventDefault()
    login(retryPassword)
    retryPassword = ''
    msg = null
  }

  onMount(() => {
    ctx = canvas.getContext('2d')
    let lastMsg: MsgBox | null = null
    const unlisten: Promise<UnlistenFn>[] = [
      listen<DisplayRect[]>(EVENTS.displays, (e) => {
        displays = e.payload
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
        const q = e.payload
        quality = {
          fps: q.fps ?? quality.fps,
          delay: q.delay ?? quality.delay,
          speed: q.speed ?? quality.speed,
        }
      }),
      listen(EVENTS.closed, () => onclosed(lastMsg)),
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
  <canvas
    bind:this={canvas}
    tabindex="0"
    onpointermove={onPointerMove}
    onpointerdown={onPointerDown}
    onpointerup={onPointerUp}
    onwheel={onWheel}
    oncontextmenu={(e) => e.preventDefault()}
  ></canvas>

  <div class="bar">
    <span>{peerId}</span>
    {#if displays.length > 1}
      {#each displays as _, i (i)}
        <button class:active={i === current} onclick={() => switchDisplay(i)}>Tela {i + 1}</button>
      {/each}
    {/if}
    {#if quality.fps !== null}<span>{quality.fps} fps</span>{/if}
    {#if quality.delay !== null}<span>{quality.delay} ms</span>{/if}
    <button onclick={disconnect}>Desconectar</button>
  </div>

  {#if revealed && msg}
    <div class="overlay">
      <p><strong>{msg.title}</strong></p>
      <p>{msg.text}</p>
      {#if needsPassword}
        <form onsubmit={submitPassword}>
          <input type="password" placeholder="Senha" bind:value={retryPassword} />
          <button type="submit">Entrar</button>
        </form>
      {:else if connected}
        <button onclick={() => (msg = null)}>OK</button>
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
  .bar {
    position: absolute;
    top: 0;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 0.5rem;
    align-items: center;
    padding: 0.25rem 0.75rem;
    background: rgba(0, 0, 0, 0.6);
    color: #eee;
    font-size: 0.85rem;
    border-radius: 0 0 6px 6px;
  }
  .bar button.active {
    font-weight: bold;
  }
  .overlay {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: #fff;
    color: #111;
    padding: 1rem 1.5rem;
    border-radius: 8px;
    max-width: 24rem;
    text-align: center;
  }
</style>
