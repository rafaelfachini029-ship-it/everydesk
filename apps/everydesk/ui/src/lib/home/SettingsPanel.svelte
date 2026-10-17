<!--
  SettingsPanel — the single quiet corner button and what sits behind it. For now only the
  theme; the rest of the settings arrive in later tasks. A disclosure: the gear says whether
  it is open (aria-expanded/aria-controls); Escape or "Fechar" closes it and puts the focus
  back on the gear.
-->
<script lang="ts">
  import Settings from 'lucide-svelte/icons/settings'
  import X from 'lucide-svelte/icons/x'
  import Button from '../ui/Button.svelte'
  import { applyTheme, storedTheme, type Theme } from './theme.ts'

  let open = $state(false)
  let theme = $state<Theme>(document.documentElement.dataset.theme === 'light' ? 'light' : storedTheme())
  let root: HTMLElement | undefined = $state()
  const panelId = 'settings-panel'

  function choose(t: Theme) {
    theme = t
    applyTheme(t, true)
  }

  function onwindowdown(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false
  }

  /** Close and hand the keyboard back to the gear. */
  function closeToGear() {
    open = false
    root?.querySelector<HTMLButtonElement>('.gear')?.focus()
  }

  function onwindowkey(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !open) return
    e.preventDefault()
    closeToGear()
  }
</script>

<svelte:window onpointerdown={onwindowdown} onkeydown={onwindowkey} />

<div class="settings" bind:this={root}>
  <Button
    class="gear"
    icon={Settings}
    label="Configurações"
    aria-expanded={open}
    aria-controls={panelId}
    onclick={() => (open = !open)}
  />
  {#if open}
    <div class="panel" id={panelId} role="group" aria-labelledby="{panelId}-title">
      <header>
        <h2 id="{panelId}-title">Configurações</h2>
        <Button icon={X} label="Fechar" onclick={closeToGear} />
      </header>
      <div class="opt">
        <span id="theme-label">Aparência</span>
        <div class="switch" role="group" aria-labelledby="theme-label">
          <button aria-pressed={theme === 'dark'} onclick={() => choose('dark')}>Escuro</button>
          <button aria-pressed={theme === 'light'} onclick={() => choose('light')}>Claro</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .settings {
    position: fixed;
    top: var(--space-4);
    right: var(--space-4);
    z-index: 10;
    color: var(--text-muted);
  }
  .settings > :global(.btn) {
    color: var(--text-muted);
  }
  .settings > :global(.btn:hover) {
    color: var(--text);
  }
  .panel {
    position: absolute;
    top: calc(44px + var(--space-2));
    right: 0;
    display: grid;
    gap: var(--space-5);
    width: min(320px, calc(100vw - 2 * var(--space-4)));
    padding: var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-glass);
    backdrop-filter: blur(18px);
    box-shadow: var(--shadow-float);
    color: var(--text);
    transform-origin: top right;
    animation: open var(--dur-base) var(--ease-out);
  }
  @keyframes open {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: calc(-1 * var(--space-2)) calc(-1 * var(--space-3)) 0 0;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-lead);
    font-weight: 600;
    letter-spacing: var(--track-title);
  }
  .opt {
    display: grid;
    gap: var(--space-2);
    font-size: var(--fs-body);
    color: var(--text-muted);
  }
  .switch {
    display: grid;
    grid-template-columns: 1fr 1fr;
    padding: var(--space-1);
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    background: var(--surface-2);
  }
  .switch button {
    min-height: 40px;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--fs-body);
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .switch button[aria-pressed='true'] {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.2);
  }
  .switch button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
