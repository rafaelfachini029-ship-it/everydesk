<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
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
    wheelSteps,
    type DisplayRect,
    type Modifiers,
    type MsgBox,
    type Quality,
  } from './session'

  interface Props {
    peerId: string
    password: string
    onclosed: (lastMessage: MsgBox | null) => void
  }
  let { peerId, password, onclosed }: Props = $props()

  let canvas: HTMLCanvasElement
  let ctx: CanvasRenderingContext2D | null = null
  let displays = $state<DisplayRect[]>([])
  let current = $state(0)
  let connected = $state(false)
  let msg = $state<MsgBox | null>(null)
  let quality = $state<Quality>({ fps: null, delay: null, speed: null })
  let retryPassword = $state('')
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
      if (!connected) {
        connected = true
        if (msg && !needsPassword && msg.kind !== 'error') msg = null
      }
    }
    frameAck()
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
  function onMouseMove(e: MouseEvent) {
    const p = toRemote(e)
    if (!p) return
    lastPos = p
    sendMouse(mouseMask(MouseButton.none, MouseEvt.move), p.x, p.y, mods(e))
  }
  function onMouseButton(e: MouseEvent, event: number) {
    e.preventDefault()
    canvas.focus()
    const b = domButton(e.button)
    if (b === MouseButton.none) return
    const p = toRemote(e) ?? lastPos
    sendMouse(mouseMask(b, event), p.x, p.y, mods(e))
  }
  function onWheel(e: WheelEvent) {
    e.preventDefault()
    const s = wheelSteps(e.deltaX, e.deltaY)
    if (s.x === 0 && s.y === 0) return
    sendMouse(mouseMask(MouseButton.none, MouseEvt.wheel), s.x, s.y, mods(e))
  }

  function onKey(e: KeyboardEvent, down: boolean) {
    if (!connected || e.target instanceof HTMLInputElement) return
    const name = keyName(e.code, e.key)
    if (!name) return
    e.preventDefault()
    sendKey(name, down, false, mods(e))
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
    canvas.focus()
    let lastMsg: MsgBox | null = null
    const unlisten: Promise<UnlistenFn>[] = [
      listen<DisplayRect[]>(EVENTS.displays, (e) => (displays = e.payload)),
      listen(EVENTS.connected, () => {
        connected = true
        if (msg && !needsPassword && msg.kind !== 'error') msg = null
      }),
      listen<MsgBox>(EVENTS.msgbox, (e) => {
        msg = e.payload
        lastMsg = e.payload
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
    window.addEventListener('keydown', down)
    window.addEventListener('keyup', up)
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
      unlisten.forEach((p) => p.then((f) => f()))
    }
  })
</script>

<div class="remote">
  <canvas
    bind:this={canvas}
    tabindex="0"
    onmousemove={onMouseMove}
    onmousedown={(e) => onMouseButton(e, MouseEvt.down)}
    onmouseup={(e) => onMouseButton(e, MouseEvt.up)}
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

  {#if !connected || msg}
    <div class="overlay">
      {#if msg}
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
      {:else}
        <p>Conectando a {peerId}...</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .remote {
    position: fixed;
    inset: 0;
    background: #111;
  }
  canvas {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    outline: none;
    cursor: default;
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
