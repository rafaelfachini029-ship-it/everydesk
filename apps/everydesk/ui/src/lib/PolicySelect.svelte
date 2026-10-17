<script lang="ts">
  // Policy selector: presets, a stored custom period, and "Outro…" (a whole number of minutes,
  // RECONFIRM_MIN..RECONFIRM_MAX). `onchange` gets a policy key (see policy.ts) only once the
  // choice is complete; "Outro…" waits for a valid number and OK.
  // Looks like a phrase you can change ("Sempre perguntar ⌄"), not a form control.
  import ChevronDown from 'lucide-svelte/icons/chevron-down'
  import Icon from './ui/Icon.svelte'
  import {
    OTHER_KEY,
    RECONFIRM_MAX,
    RECONFIRM_MIN,
    isValidPeriod,
    policyFromKey,
    policyOptions,
  } from './policy'

  let {
    value,
    onchange,
    label = 'Da próxima vez',
  }: { value: string; onchange: (key: string) => void; label?: string } = $props()

  let other = $state(false)
  let minutes = $state<number | null>(null)

  function pick(key: string) {
    if (key === OTHER_KEY) {
      other = true
      minutes = null
      return
    }
    other = false
    onchange(key)
  }

  function confirmOther(e: Event) {
    e.preventDefault()
    if (minutes === null || !isValidPeriod(minutes)) return
    other = false
    onchange(`r${minutes}`)
  }
</script>

<span class="policy">
  <span class="choice">
    <select
      aria-label={label}
      value={other ? OTHER_KEY : value}
      onchange={(e) => pick(e.currentTarget.value)}
    >
      {#each policyOptions(policyFromKey(value)) as opt (opt.key)}
        <option value={opt.key}>{opt.label}</option>
      {/each}
    </select>
    <span class="chev"><Icon icon={ChevronDown} size={16} /></span>
  </span>
  {#if other}
    <span class="other">
      <span>a cada</span>
      <input
        type="number"
        aria-label="Minutos entre confirmações ({RECONFIRM_MIN} a {RECONFIRM_MAX})"
        min={RECONFIRM_MIN}
        max={RECONFIRM_MAX}
        step="1"
        bind:value={minutes}
        onkeydown={(e) => e.key === 'Enter' && confirmOther(e)}
      />
      <span>min</span>
      <button
        type="button"
        class="act"
        onclick={confirmOther}
        disabled={minutes === null || !isValidPeriod(minutes)}>OK</button
      >
      <button type="button" class="act quiet" onclick={() => (other = false)}>Cancelar</button>
    </span>
  {/if}
</span>

<style>
  .policy {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    font-family: var(--font-ui);
    font-size: var(--fs-label);
    color: var(--text);
  }
  .choice {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  select {
    appearance: none;
    /* hug the chosen phrase where supported (Chromium/WebView2) */
    field-sizing: content;
    min-height: 32px;
    padding: 0 calc(var(--space-2) + 18px) 0 var(--space-2);
    margin-left: calc(-1 * var(--space-2));
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    font: inherit;
    font-weight: 600;
    text-decoration: underline dotted var(--line-strong);
    text-underline-offset: 5px;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  select:hover {
    background: var(--surface-2);
  }
  select option {
    background: var(--surface);
    color: var(--text);
  }
  .chev {
    position: absolute;
    right: var(--space-1);
    pointer-events: none;
    color: var(--text-muted);
  }
  .other {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-muted);
  }
  input {
    width: 4.5em;
    min-height: 32px;
    padding: 0 var(--space-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-code);
    font-size: var(--fs-label);
  }
  .act {
    min-height: 32px;
    padding: 0 var(--space-3);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .act.quiet {
    border-color: transparent;
    color: var(--text-muted);
  }
  .act:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .act:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  select:focus-visible,
  input:focus-visible,
  .act:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
