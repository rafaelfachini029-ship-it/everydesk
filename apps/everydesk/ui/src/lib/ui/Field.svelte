<!--
  Field — large single-line input with a label above and an optional hint below.
  Props:
    label: string (always visible; never placeholder-only)
    lead?: boolean — the label is the question of the screen ("Para onde você quer ir?"):
      lead size, centred, full text colour
    value?: string (bindable)
    hint?: string — helper text, announced with the input
    error?: string — replaces the hint, marks the input invalid (aria-invalid) and is said by a
      polite live region that is always mounted, so it is announced when it appears
    placeholder?: string
    id?: string (auto-generated when absent)
    type?: 'text' | 'search' | 'password' = 'text'
    autocomplete?: string = 'off'
    autofocus?: boolean
    onkeydown?: (e: KeyboardEvent) => void
-->
<script lang="ts">
  import type { FullAutoFill } from 'svelte/elements'

  interface Props {
    label: string
    lead?: boolean
    value?: string
    hint?: string
    error?: string
    placeholder?: string
    id?: string
    type?: 'text' | 'search' | 'password'
    autocomplete?: FullAutoFill
    autofocus?: boolean
    onkeydown?: (e: KeyboardEvent) => void
  }
  const uid = $props.id()
  let {
    label,
    lead = false,
    value = $bindable(''),
    hint,
    error,
    placeholder,
    id = `field-${uid}`,
    type = 'text',
    autocomplete = 'off',
    autofocus = false,
    onkeydown,
  }: Props = $props()

  const note = $derived(error || hint)
</script>

<div class="field" class:invalid={!!error} class:lead>
  <label class={lead ? 't-lead' : 't-label'} for={id}>{label}</label>
  <!-- svelte-ignore a11y_autofocus -->
  <input
    {id}
    {type}
    {placeholder}
    {autocomplete}
    {autofocus}
    {onkeydown}
    spellcheck="false"
    aria-invalid={error ? 'true' : undefined}
    aria-describedby={note ? `${id}-note` : undefined}
    bind:value
  />
  <p id="{id}-note" class="note" aria-live="polite">{note ?? ''}</p>
</div>

<style>
  .field {
    display: grid;
  }
  label {
    margin-bottom: var(--space-2);
    color: var(--text);
  }
  .lead label {
    justify-self: center;
    margin-bottom: var(--space-4);
    text-align: center;
  }
  input {
    min-height: 64px;
    padding: 0 var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-md);
    background: var(--surface);
    color: var(--text);
    font-family: var(--font-ui);
    font-size: var(--fs-lead);
    font-weight: 500;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out);
  }
  input::placeholder {
    color: var(--text-muted);
    opacity: 1;
  }
  input:hover {
    border-color: var(--text-muted);
  }
  input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 3px;
    border-color: var(--ring);
  }
  .note {
    margin: var(--space-2) 0 0;
    color: var(--text-muted);
    font-size: var(--fs-body);
    line-height: var(--lh-body);
  }
  /* empty but mounted: the live region exists before the first error */
  .note:empty {
    margin: 0;
  }
  .invalid input {
    border-color: var(--danger);
  }
  .invalid .note {
    color: var(--danger);
  }
</style>
