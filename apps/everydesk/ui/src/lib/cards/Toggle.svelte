<!--
  Toggle — a quiet switch row for the request card ("Lembrar este contato").
  Props:
    checked: boolean (bindable)
    tone?: 'default' | 'danger' — danger for "Substituir a chave guardada"
    onchange?: (checked: boolean) => void — after the user flips it
    children: the label
    hint?: Snippet — a second line under the label
  The "on" colour is the text colour: the accent is reserved (spec 6.1).
-->
<script lang="ts">
  import type { Snippet } from 'svelte'

  interface Props {
    checked: boolean
    tone?: 'default' | 'danger'
    onchange?: (checked: boolean) => void
    children: Snippet
    hint?: Snippet
  }
  let { checked = $bindable(), tone = 'default', onchange, children, hint }: Props = $props()
  const uid = $props.id()
</script>

<div class="row {tone}">
  <label for="t-{uid}" class="text">
    <span class="t-label name">{@render children()}</span>
  </label>
  <input
    id="t-{uid}"
    type="checkbox"
    role="switch"
    bind:checked
    onchange={() => onchange?.(checked)}
  />
  {#if hint}<div class="hint">{@render hint()}</div>{/if}
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    column-gap: var(--space-4);
    row-gap: var(--space-1);
  }
  .text {
    cursor: pointer;
    min-height: 32px;
    display: flex;
    align-items: center;
  }
  .name {
    color: var(--text);
  }
  .danger .name {
    color: var(--danger);
  }
  .hint {
    grid-column: 1 / -1;
  }

  input {
    appearance: none;
    position: relative;
    width: 40px;
    height: 24px;
    margin: 0;
    border-radius: var(--radius-pill);
    border: 1px solid var(--line-strong);
    background: transparent;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }
  input::after {
    content: '';
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text-muted);
    transition:
      transform var(--dur-base) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out);
  }
  input:checked {
    background: var(--text);
    border-color: var(--text);
  }
  input:checked::after {
    transform: translateX(16px);
    background: var(--bg);
  }
  .danger input:checked {
    background: var(--danger);
    border-color: var(--danger);
  }
  input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 3px;
  }
</style>
