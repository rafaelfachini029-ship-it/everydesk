<!--
  ContactCards — paired machines as discreet cards: online dot, name, code. Clicking a card
  connects. "…" opens a small menu with how the contact may enter here (PolicySelect, only for
  peers that connect to this machine) and Esquecer (with an inline confirmation).
  Props:
    contacts: Contact[] — already filtered/ordered by the caller
    onconnect: (peerId: string) => void
    onpolicy: (c: Contact, key: string) => void
    onforget: (c: Contact) => void
  Export: focusFirst() — moves keyboard focus to the first card.
-->
<script lang="ts">
  import Ellipsis from 'lucide-svelte/icons/ellipsis'
  import Button from '../ui/Button.svelte'
  import Icon from '../ui/Icon.svelte'
  import PolicySelect from '../PolicySelect.svelte'
  import { policyKey, type Contact } from '../policy.ts'

  interface Props {
    contacts: Contact[]
    onconnect: (peerId: string) => void
    onpolicy: (c: Contact, key: string) => void
    onforget: (c: Contact) => void
  }
  let { contacts, onconnect, onpolicy, onforget }: Props = $props()

  let open = $state<string | null>(null)
  let confirming = $state(false)
  let listEl: HTMLElement | undefined = $state()

  export function focusFirst() {
    listEl?.querySelector<HTMLButtonElement>('.card-go')?.focus()
  }

  function toggle(id: string) {
    confirming = false
    open = open === id ? null : id
  }

  function close() {
    open = null
    confirming = false
  }

  /** Escape closes the "…" menu and puts the focus back on its "…" button. Handled here,
   *  before Home's Escape (which folds the cards away), and marked as used. */
  function escape(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !open || e.defaultPrevented) return
    e.preventDefault()
    const peer = open
    const hadFocus = !!listEl?.querySelector(`[data-peer="${CSS.escape(peer)}"]`)?.contains(document.activeElement)
    close()
    if (hadFocus || document.activeElement === document.body)
      listEl?.querySelector<HTMLButtonElement>(`[data-peer="${CSS.escape(peer)}"] .more`)?.focus()
  }

  function onwindowdown(e: PointerEvent) {
    if (open && !(e.target as Element).closest?.(`[data-peer="${CSS.escape(open)}"]`)) close()
  }

  /** Arrow keys walk the cards like a list. */
  function keynav(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
    const cards = [...(listEl?.querySelectorAll<HTMLButtonElement>('.card-go') ?? [])]
    const i = cards.indexOf(document.activeElement as HTMLButtonElement)
    if (i < 0) return
    e.preventDefault()
    cards[Math.max(0, Math.min(cards.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))]?.focus()
  }
</script>

<svelte:window onpointerdown={onwindowdown} onkeydown={escape} />

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<ul class="cards" bind:this={listEl} aria-label="Contatos" onkeydown={(e) => (escape(e), keynav(e))}>
  {#each contacts as c (c.peer_id)}
    <li class="card" class:raised={open === c.peer_id} data-peer={c.peer_id}>
      <button class="card-go" onclick={() => onconnect(c.peer_id)} aria-label="Conectar a {c.name || c.peer_id}{c.online ? ', online' : ''}">
        <span class="top">
          <span class="dot" class:online={c.online} aria-hidden="true"></span>
          <span class="name">{c.name || c.peer_id}</span>
        </span>
        {#if c.name}<span class="code">{c.peer_id}</span>{/if}
      </button>
      <button
        class="more"
        aria-label="Opções de {c.name || c.peer_id}"
        aria-expanded={open === c.peer_id}
        onclick={() => toggle(c.peer_id)}
      >
        <Icon icon={Ellipsis} size={18} />
      </button>
      {#if open === c.peer_id}
        <div class="menu" role="group" aria-label="Opções de {c.name || c.peer_id}">
          {#if confirming}
            <p class="ask">Esquecer {c.name || c.peer_id}? Para voltar, vai precisar do código e de um aceite.</p>
            <div class="row">
              <Button variant="danger" onclick={() => (onforget(c), close())}>Esquecer</Button>
              <Button onclick={() => (confirming = false)}>Cancelar</Button>
            </div>
          {:else}
            {#if c.policy}
              <div class="policy">
                <span class="cap" aria-hidden="true">Quando acessar esta máquina</span>
                <PolicySelect label="Quando acessar esta máquina" value={policyKey(c.policy)} onchange={(key) => onpolicy(c, key)} />
              </div>
            {/if}
            <Button variant="danger" onclick={() => (confirming = true)}>Esquecer</Button>
          {/if}
        </div>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: var(--space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .card {
    position: relative;
  }
  .card.raised {
    z-index: 2;
  }
  .card-go {
    display: grid;
    gap: var(--space-1);
    width: 100%;
    min-height: 72px;
    padding: var(--space-4) calc(var(--space-4) + 36px) var(--space-4) var(--space-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-md);
    background: var(--surface-glass);
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }
  .card-go:hover {
    border-color: var(--line-strong);
    background: var(--surface-2);
  }
  .card-go:active {
    transform: scale(0.98);
  }
  .card-go:focus-visible,
  .more:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 3px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .name {
    overflow: hidden;
    font-size: var(--fs-body);
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .code {
    overflow: hidden;
    color: var(--text-muted);
    font-family: var(--font-code);
    font-size: var(--fs-body);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
  }
  .dot.online {
    background: var(--accent);
    border-color: var(--accent);
  }
  .more {
    position: absolute;
    top: var(--space-2);
    right: var(--space-2);
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .more:hover,
  .more[aria-expanded='true'] {
    background: var(--surface-2);
    color: var(--text);
  }
  .menu {
    position: absolute;
    top: calc(var(--space-2) + 40px);
    right: 0;
    display: grid;
    gap: var(--space-3);
    width: max(100%, 260px);
    padding: var(--space-4);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-md);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    animation: drop var(--dur-fast) var(--ease-out);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .policy {
    display: grid;
    justify-items: start;
    gap: var(--space-1);
  }
  .cap {
    color: var(--text-muted);
    font-size: var(--fs-body);
  }
  .ask {
    margin: 0;
    font-size: var(--fs-body);
    line-height: var(--lh-body);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
</style>
