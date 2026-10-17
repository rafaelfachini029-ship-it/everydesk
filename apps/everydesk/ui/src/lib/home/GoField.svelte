<!--
  GoField — "Para onde você quer ir?": one field and Conectar. Accepts a dictated code
  ("Azul Tigre 42"), a code, or a contact's name; Enter or Conectar goes to resolveTarget().
  Props:
    contacts: Contact[] — used to resolve a typed name
    query?: string (bindable)
    error?: string — from the last attempt; cleared as soon as the person types
    onconnect: (peerId: string) => void
    onarrowdown?: () => void — ArrowDown moves into the cards
-->
<script lang="ts">
  import ArrowRight from 'lucide-svelte/icons/arrow-right'
  import Button from '../ui/Button.svelte'
  import Field from '../ui/Field.svelte'
  import type { Contact } from '../policy.ts'
  import { resolveTarget } from './go.ts'

  interface Props {
    contacts: Contact[]
    query?: string
    error?: string
    onconnect: (peerId: string) => void
    onarrowdown?: () => void
  }
  let { contacts, query = $bindable(''), error = $bindable(''), onconnect, onarrowdown }: Props = $props()

  let typed = $state(query)
  $effect(() => {
    if (query !== typed) {
      typed = query
      error = ''
    }
  })

  function go() {
    const id = resolveTarget(contacts, query)
    if (!id) {
      error = 'Digite o código da outra máquina.'
      return
    }
    error = ''
    onconnect(id)
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      go()
    } else if (e.key === 'ArrowDown' && onarrowdown) {
      e.preventDefault()
      onarrowdown()
    }
  }
</script>

<div class="go">
  <Field
    label="Para onde você quer ir?"
    placeholder={contacts.length ? 'código ou contato' : 'código da outra máquina'}
    bind:value={query}
    {error}
    onkeydown={keydown}
  />
  <div class="action">
    <Button variant="primary" size="lg" icon={ArrowRight} onclick={go}>Conectar</Button>
  </div>
</div>

<style>
  .go {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    column-gap: var(--space-3);
    width: 100%;
  }
  /* sits level with the 64px input: below the label line, centred on the input */
  .action {
    padding-top: calc(var(--fs-label) * var(--lh-snug) + var(--space-2) + 2px);
  }
  .action :global(.btn) {
    flex-direction: row-reverse;
  }
  @media (max-width: 520px) {
    .go {
      grid-template-columns: 1fr;
      row-gap: var(--space-3);
    }
    .action {
      padding-top: 0;
    }
    .action :global(.btn) {
      width: 100%;
    }
  }
</style>
