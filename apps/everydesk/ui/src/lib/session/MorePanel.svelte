<!--
  MorePanel — the pill's "Mais" tool and the panel it opens (spec §5.2/§5.3): switches in plain
  words (Só olhar, block input, sound, clipboard, remote cursor), the quality choice and the
  remote actions (Ctrl+Alt+Del, lock, restart with an inline confirm). State lives in
  session.ts `extras` (more.ts reducer), hydrated from `session_get_toggles` on open and kept
  by `session:block_input` / `session:permission`; a switch the host takes away reads "a outra
  máquina não permite". The panel hangs under the pill (positioned against the pill's wrapper,
  outside the tool strip's clip) and is part of the pill's DOM, so the pill's dead zone, key
  gate and hide timer cover it.
  Props:
    active: boolean — the pill is expanded; the panel closes when it collapses
    onreturn: () => void — give the keyboard back to the remote screen
-->
<script lang="ts">
  import { onMount, tick } from 'svelte'
  import Ellipsis from 'lucide-svelte/icons/ellipsis'
  import Eye from 'lucide-svelte/icons/eye'
  import Icon from '../ui/Icon.svelte'
  import {
    ctrlAltDel,
    dispatchExtras,
    extras,
    hydrateExtras,
    lockRemote,
    restartRemote,
    setQuality,
    toggleOption,
  } from '../session'
  import {
    QUALITY_LEVELS,
    SWITCHES,
    DENIED_NOTE,
    VIEW_ONLY_NOTE,
    actionDenied,
    optionAfter,
    switchView,
    toggleCommand,
    viewOnly,
    type QualityLevel,
    type SwitchDef,
  } from './more.ts'

  interface Props {
    active: boolean
    onreturn: () => void
  }
  let { active, onreturn }: Props = $props()

  const uid = $props.id()
  let open = $state(false)
  let confirming = $state(false)
  let error = $state<string | null>(null)
  let root: HTMLSpanElement
  let panel: HTMLDivElement | undefined = $state()

  const looking = $derived(viewOnly($extras))

  $effect(() => {
    if (!active) close()
  })

  // block_input / permission events are kept by session.ts (listenExtras) from before the
  // first frame; the panel only reads the shared store.
  onMount(() => {
    const outside = (e: PointerEvent) => {
      if (open && !root.contains(e.target as Node | null)) close()
    }
    document.addEventListener('pointerdown', outside, true)
    return () => {
      document.removeEventListener('pointerdown', outside, true)
    }
  })

  function close() {
    open = false
    confirming = false
    error = null
  }

  async function toggle() {
    if (open) return close()
    open = true
    error = null
    await hydrateExtras()
    await tick()
    panel?.querySelector<HTMLElement>('button:not([disabled])')?.focus()
  }

  async function flip(def: SwitchDef, on: boolean) {
    error = null
    const before = $extras.options[def.option] ?? false
    // Optimistic first: turning Só olhar on closes the input gate (and releases what the peer
    // holds) before the option reaches the engine.
    dispatchExtras({ type: 'set', option: def.option, value: optionAfter(def, on) })
    try {
      const value = await toggleOption(toggleCommand(def, on))
      dispatchExtras({ type: 'set', option: def.option, value })
    } catch (e) {
      dispatchExtras({ type: 'set', option: def.option, value: before })
      error = String(e)
    }
  }

  async function pickQuality(level: QualityLevel) {
    if (level === $extras.quality) return
    const before = $extras.quality
    dispatchExtras({ type: 'quality', level })
    try {
      await setQuality(level)
    } catch (e) {
      dispatchExtras({ type: 'quality', level: before })
      error = String(e)
    }
  }

  async function act(run: () => Promise<unknown>) {
    error = null
    try {
      await run()
      close()
      onreturn()
    } catch (e) {
      error = String(e)
    }
  }
</script>

<span class="more" bind:this={root}>
  <button
    class="tool"
    class:on={open}
    class:looking
    aria-label="Mais"
    aria-expanded={open}
    aria-controls="{uid}-panel"
    onclick={toggle}
  >
    <Icon icon={looking ? Eye : Ellipsis} />
  </button>

  {#if open}
    <div
      bind:this={panel}
      id="{uid}-panel"
      class="panel"
      role="dialog"
      aria-label="Mais opções da sessão"
      tabindex="-1"
    >
      <ul class="switches">
        {#each SWITCHES as def (def.key)}
          {@const v = switchView($extras, def)}
          <li class:lead={def.key === 'viewOnly'} class:off={v.disabled}>
            <button
              class="switch"
              role="switch"
              aria-checked={v.checked}
              disabled={v.disabled}
              aria-describedby={v.note || def.key === 'viewOnly' ? `${uid}-${def.key}` : undefined}
              onclick={() => flip(def, !v.checked)}
            >
              <span class="label">{def.label}</span>
              <span class="track" aria-hidden="true"><span class="knob"></span></span>
            </button>
            {#if def.key === 'viewOnly'}
              <span class="note" id="{uid}-{def.key}"
                >{v.checked ? 'Seu mouse e teclado não chegam lá' : 'Ver a tela sem mexer em nada'}</span
              >
            {:else if v.note}
              <span class="note" id="{uid}-{def.key}">{v.note}</span>
            {/if}
          </li>
        {/each}
      </ul>

      <div class="block">
        <span class="heading" id="{uid}-q">Qualidade</span>
        <div class="segmented" role="radiogroup" aria-labelledby="{uid}-q">
          {#each QUALITY_LEVELS as q (q.level)}
            <button
              role="radio"
              aria-checked={$extras.quality === q.level}
              onclick={() => pickQuality(q.level)}>{q.label}</button
            >
          {/each}
        </div>
      </div>

      <div class="block actions">
        <div class="pair">
          <button
            class="action t-num"
            disabled={actionDenied($extras, 'ctrlAltDel')}
            onclick={() => act(ctrlAltDel)}>Ctrl+Alt+Del</button
          >
          <button class="action" disabled={actionDenied($extras, 'lock')} onclick={() => act(lockRemote)}
            >Bloquear a tela de lá</button
          >
        </div>
        {#if looking}
          <span class="note">{VIEW_ONLY_NOTE}</span>
        {:else if actionDenied($extras, 'ctrlAltDel')}
          <span class="note">{DENIED_NOTE}</span>
        {/if}
        {#if confirming}
          <div class="confirm" role="group" aria-labelledby="{uid}-r">
            <span id="{uid}-r">A sessão cai e volta sozinha quando ela ligar.</span>
            <div class="pair">
              <button class="action danger" onclick={() => act(restartRemote)}>Reiniciar agora</button>
              <button class="action" onclick={() => (confirming = false)}>Cancelar</button>
            </div>
          </div>
        {:else}
          <button
            class="action wide"
            disabled={actionDenied($extras, 'restart')}
            onclick={async () => {
              confirming = true
              await tick()
              panel?.querySelector<HTMLElement>('.confirm .danger')?.focus()
            }}>Reiniciar a outra máquina</button
          >
          {#if actionDenied($extras, 'restart')}<span class="note">{DENIED_NOTE}</span>{/if}
        {/if}
      </div>

      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
  {/if}
</span>

<style>
  /* Not positioned: the panel anchors to the pill's wrapper, outside the tool strip's clip. */
  .more {
    display: inline-flex;
  }
  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .tool:hover,
  .tool.on {
    background: var(--surface-2);
  }
  /* Só olhar on: the tool itself says so, in the one accent */
  .tool.looking {
    color: var(--accent);
  }
  .tool:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .panel {
    position: absolute;
    top: calc(100% + var(--space-2));
    right: 0;
    z-index: 3;
    box-sizing: border-box;
    width: min(360px, calc(100vw - 2 * var(--space-4)));
    padding: var(--space-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    color: var(--text);
    cursor: default;
    animation: drop var(--dur-base) var(--ease-out);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .panel:focus {
    outline: none;
  }

  .switches {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .switches li {
    display: grid;
    padding: 0 var(--space-3);
    border-radius: var(--radius-md);
  }
  /* Só olhar leads, set apart: it changes what everything else here means */
  .switches li.lead {
    margin-bottom: var(--space-2);
    padding-block: var(--space-1) var(--space-2);
    background: var(--surface-2);
  }
  .switch {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: 44px;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: var(--fs-body);
    text-align: left;
    cursor: pointer;
  }
  .lead .switch {
    font-weight: 600;
  }
  .switch:disabled {
    cursor: not-allowed;
  }
  .off .label {
    color: var(--text-muted);
  }
  .switch:focus-visible {
    outline: none;
  }
  .switch:focus-visible .track {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .track {
    flex: none;
    position: relative;
    width: 40px;
    height: 24px;
    border: 1.5px solid var(--line-strong);
    border-radius: var(--radius-pill);
    transition:
      background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: var(--text-muted);
    transition:
      transform var(--dur-base) var(--ease-spring),
      background-color var(--dur-fast) var(--ease-out);
  }
  [aria-checked='true'] .track {
    border-color: var(--accent);
    background: var(--accent-track);
  }
  [aria-checked='true'] .knob {
    transform: translateX(16px);
    background: var(--accent);
  }
  .switch:disabled .track {
    border-style: dashed;
  }
  .note {
    margin: -6px 0 var(--space-2);
    color: var(--text-muted);
    font-size: var(--fs-label);
  }

  .block {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding: var(--space-3) var(--space-3) var(--space-2);
    border-top: 1px solid var(--line);
  }
  .heading {
    color: var(--text-muted);
    font-size: var(--fs-label);
  }
  .segmented {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    padding: 3px;
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
  }
  .segmented button {
    min-height: 34px;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--fs-body);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .segmented button:hover {
    color: var(--text);
  }
  .segmented [aria-checked='true'] {
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
  }
  .segmented button:focus-visible,
  .action:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .pair {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2);
  }
  .action {
    min-width: 0;
    min-height: 40px;
    padding: 0 var(--space-3);
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--fs-body);
    white-space: nowrap;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .action:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .action:disabled {
    color: var(--text-muted);
    border-style: dashed;
    cursor: not-allowed;
  }
  .action.wide {
    width: 100%;
  }
  .action.danger {
    border-color: var(--danger);
    background: var(--danger-soft);
    color: var(--danger);
  }
  .confirm {
    display: grid;
    gap: var(--space-2);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--danger-soft);
    font-size: var(--fs-body);
  }
  .confirm .pair {
    grid-template-columns: 1fr auto;
  }
  .error {
    margin: var(--space-2) var(--space-3) var(--space-1);
    color: var(--danger);
    font-size: var(--fs-label);
  }
</style>
