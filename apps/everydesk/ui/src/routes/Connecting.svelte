<!--
  Connecting (spec 6.2, moment 2): your card, the other machine's card, and a trace of light
  between them. How far the light reaches says where things are (searching → it touched the
  other card → its dot lights up when the session opens); a spark travels the lit part
  instead of a spinner. Under "reduzir movimento" the reach jumps and the spark is gone; the
  label and the dot still change, so no state is lost.
  Props:
    target: string — the code being reached (already normalized)
    phase: Phase — from phaseFor(events)
    oncancel: () => void — while in progress (the caller closes the session)
    onretry: () => void, onback: () => void — error state
    onlogin: (password: string) => void — answer the host's password prompt
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount, tick } from 'svelte'
  import { animate, prefersReducedMotion } from '../design/motion.ts'
  import { REACH, SPARK_MS, arrived, cancellable, traceOptions, type Phase } from '../lib/connecting/connecting.ts'
  import type { Contact } from '../lib/policy.ts'
  import Button from '../lib/ui/Button.svelte'
  import Field from '../lib/ui/Field.svelte'

  interface Props {
    target: string
    phase: Phase
    oncancel: () => void
    onretry: () => void
    onback: () => void
    onlogin: (password: string) => void
  }
  let { target, phase, oncancel, onretry, onback, onlogin }: Props = $props()

  let myId = $state('')
  let name = $state('')
  let password = $state('')
  let w = $state(0)
  let h = $state(0)
  let lit: SVGLineElement | undefined = $state()
  let spark: SVGLineElement | undefined = $state()
  let actions: HTMLElement | undefined = $state()

  onMount(async () => {
    try {
      myId = await invoke<string>('get_my_id')
      const list = await invoke<Contact[]>('contacts_list')
      name = list.find((c) => c.peer_id === target)?.name ?? ''
    } catch {
      /* the cards still show the code being reached */
    }
  })

  // The trace runs along the longer side of its box: across on wide windows, down on narrow.
  const vertical = $derived(h > w)
  const len = $derived(Math.max(vertical ? h : w, 1))
  const end = $derived(vertical ? { x: 12, y: len } : { x: len, y: 12 })
  const failed = $derived(phase.name === 'erro')

  // How far the light reaches; frozen where it was when an attempt fails.
  let reach = $state(0)
  $effect(() => {
    if (phase.name !== 'erro') reach = REACH[phase.name]
  })

  // Lit part: animate from wherever it is to the new reach (instant under reduced motion).
  let shown = 0
  let reachAnim: Animation | undefined
  $effect(() => {
    if (!lit) return
    const to = reach
    const from = shown
    shown = to
    reachAnim?.cancel()
    reachAnim = animate(lit, [{ strokeDashoffset: 100 - from * 100 }, { strokeDashoffset: 100 - to * 100 }], traceOptions().reach)
  })

  // Spark: a short bright dash travelling the lit part, looping while waiting. Not drawn at
  // all under reduced motion (a looping cue would turn into a static dot with no meaning).
  const reduced = prefersReducedMotion()
  $effect(() => {
    if (!spark || reduced || failed) return
    const to = reach * 100
    const a = animate(
      spark,
      [
        { strokeDashoffset: 6, opacity: 0 },
        { opacity: 1, offset: 0.15 },
        { opacity: 1, offset: 0.8 },
        { strokeDashoffset: 6 - to, opacity: 0 },
      ],
      { ...traceOptions().spark, duration: SPARK_MS * Math.max(reach, 0.5) },
    )
    return () => a.cancel()
  })

  // Errors move focus to the first action; the password prompt to its field.
  $effect(() => {
    if (!failed) return
    tick().then(() => actions?.querySelector('button')?.focus())
  })

  const label = $derived(
    {
      procurando: 'Procurando a outra máquina',
      pedindo: 'Pedindo permissão',
      conectando: 'Conectando',
      pronto: 'Conectando',
      aviso: phase.message?.title ?? '',
      senha: phase.message?.title ?? '',
      erro: phase.message?.title ?? '',
    }[phase.name],
  )
  const sub = $derived(
    phase.name === 'pedindo'
      ? 'O pedido apareceu na outra tela. Assim que aceitarem, a conexão abre.'
      : (phase.message?.body ?? ''),
  )

  function submit(e: SubmitEvent) {
    e.preventDefault()
    if (!password) return
    onlogin(password)
    password = ''
  }

  function keydown(e: KeyboardEvent) {
    // Not during the reveal (pronto): the session that just opened must stay open.
    if (e.key === 'Escape' && cancellable(phase.name)) oncancel()
  }
</script>

<svelte:window onkeydown={keydown} />

<main class="connecting" data-phase={phase.name}>
  <div class="route">
    <section class="card origin" aria-label="Este computador">
      <span class="who">Você</span>
      <span class="code">{myId}</span>
    </section>

    <div class="trace" class:failed bind:clientWidth={w} bind:clientHeight={h} aria-hidden="true">
      <svg width={vertical ? 24 : len} height={vertical ? len : 24} viewBox="0 0 {vertical ? 24 : len} {vertical ? len : 24}">
        <line class="track" x1={vertical ? 12 : 0} y1={vertical ? 0 : 12} x2={end.x} y2={end.y} />
        <line
          bind:this={lit}
          class="lit"
          class:failed
          pathLength="100"
          x1={vertical ? 12 : 0}
          y1={vertical ? 0 : 12}
          x2={end.x}
          y2={end.y}
        />
        {#if !reduced && !failed}
          <line bind:this={spark} class="spark" pathLength="100" x1={vertical ? 12 : 0} y1={vertical ? 0 : 12} x2={end.x} y2={end.y} />
        {/if}
      </svg>
    </div>

    <section class="card dest" class:found={reach === 1} class:arrived={arrived(phase.name)} aria-label="Máquina de destino">
      <span class="dot" aria-hidden="true"></span>
      {#if name}
        <span class="who">{name}</span>
        <span class="code">{target}</span>
      {:else}
        <span class="code solo">{target}</span>
      {/if}
    </section>
  </div>

  <div class="status">
    <div aria-live="polite" aria-atomic="true">
      <h1 class="label">{label}</h1>
      {#if sub}<p class="sub">{sub}</p>{/if}
    </div>

    {#if failed && phase.message?.detail}
      <details class="detail">
        <summary>Detalhes</summary>
        <code>{phase.message.detail}</code>
      </details>
    {/if}

    {#if phase.name === 'senha'}
      <form class="password" onsubmit={submit}>
        <Field label="Senha" type="password" autocomplete="current-password" autofocus bind:value={password} />
        <div class="row">
          <Button variant="primary" type="submit">Entrar</Button>
          <Button onclick={oncancel}>Cancelar</Button>
        </div>
      </form>
    {:else}
      <div class="row" bind:this={actions}>
        {#if failed}
          {#if phase.message?.retryable}<Button variant="primary" onclick={onretry}>Tentar de novo</Button>{/if}
          <Button onclick={onback}>Voltar</Button>
        {:else}
          <Button disabled={!cancellable(phase.name)} onclick={oncancel}>Cancelar</Button>
        {/if}
      </div>
    {/if}
  </div>
</main>

<style>
  .connecting {
    min-height: 100svh;
    box-sizing: border-box;
    display: grid;
    align-content: center;
    justify-items: center;
    gap: clamp(var(--space-6), 8vh, var(--space-8));
    padding: var(--space-7) var(--space-6);
  }

  /* ---- the route: card — trace — card ---- */
  .route {
    width: min(100%, 880px);
    display: grid;
    grid-template-columns: minmax(0, 15rem) minmax(4rem, 1fr) minmax(0, 15rem);
    align-items: center;
  }
  .card {
    position: relative;
    display: grid;
    gap: var(--space-1);
    padding: var(--space-5) var(--space-5);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    min-width: 0;
  }
  .who {
    font-size: var(--fs-lead);
    font-weight: 500;
    line-height: var(--lh-snug);
    overflow-wrap: anywhere;
  }
  .code {
    font-family: var(--font-code);
    font-size: var(--fs-label);
    color: var(--text-muted);
    letter-spacing: var(--track-label);
    overflow-wrap: anywhere;
    min-height: 1lh;
  }
  .code.solo {
    font-size: var(--fs-lead);
    color: var(--text);
    letter-spacing: 0;
  }
  /* Not found yet: the destination waits half-lit. Opacity, not colour, so it reads in
     both themes and without motion. */
  .dest {
    opacity: 0.55;
    transition:
      opacity var(--dur-slow) var(--ease-out),
      border-color var(--dur-slow) var(--ease-out);
  }
  .dest.found {
    opacity: 1;
    border-color: var(--line-strong);
  }
  /* The destination's dot: hollow until the session opens, then the pulse fills it. */
  .dot {
    position: absolute;
    top: 50%;
    left: -5px;
    width: 10px;
    height: 10px;
    margin-top: -5px;
    border-radius: 50%;
    box-sizing: border-box;
    background: var(--bg);
    border: 1.5px solid var(--line-strong);
    transition:
      background-color var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out);
  }
  .arrived .dot {
    background: var(--accent);
    border-color: var(--accent);
    box-shadow: 0 0 12px var(--accent-glow);
  }

  .trace {
    height: 24px;
    min-width: 0;
    overflow: visible;
  }
  /* The glow sits on the svg box: a filter on a straight line has a zero-height bounding
     box and would hide the line. */
  .trace svg {
    display: block;
    overflow: visible;
    filter: drop-shadow(0 0 3px var(--accent)) drop-shadow(0 0 10px var(--accent-glow));
  }
  .trace.failed svg {
    filter: none;
  }
  .trace line {
    fill: none;
    stroke-linecap: round;
  }
  .track {
    stroke: var(--line);
    stroke-width: 1;
  }
  .lit {
    stroke: var(--accent);
    stroke-width: 1.5;
    stroke-dasharray: 100 100;
    stroke-dashoffset: 100;
    transition: stroke var(--dur-base) var(--ease-out);
  }
  .lit.failed {
    stroke: var(--line-strong);
  }
  .spark {
    stroke: var(--accent);
    stroke-width: 3;
    stroke-dasharray: 6 200;
    stroke-dashoffset: 6;
    opacity: 0;
  }

  /* Narrow windows: cards stacked, the trace runs down between them. */
  @media (max-width: 640px) {
    .route {
      grid-template-columns: minmax(0, 20rem);
      grid-template-rows: auto 72px auto;
      justify-content: center;
    }
    .trace {
      height: 100%;
      width: 24px;
      justify-self: center;
    }
    .dot {
      top: -5px;
      left: 50%;
      margin: 0 0 0 -5px;
    }
  }

  /* ---- status and actions ---- */
  .status {
    display: grid;
    justify-items: center;
    gap: var(--space-5);
    width: min(100%, 30rem);
    text-align: center;
  }
  .label {
    margin: 0;
    font-size: var(--fs-lead);
    font-weight: 500;
    line-height: var(--lh-snug);
  }
  .sub {
    margin: var(--space-2) 0 0;
    color: var(--text-muted);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-3);
  }
  .password {
    display: grid;
    gap: var(--space-4);
    width: 100%;
    text-align: start;
  }
  .detail {
    color: var(--text-muted);
    font-size: var(--fs-label);
  }
  .detail summary {
    cursor: pointer;
    border-radius: var(--radius-sm);
  }
  .detail summary:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 3px;
  }
  .detail code {
    display: block;
    margin-top: var(--space-2);
    font-family: var(--font-code);
    overflow-wrap: anywhere;
  }
</style>
