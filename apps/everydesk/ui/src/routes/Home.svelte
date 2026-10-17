<!--
  Home (spec 6.2, moment 1): the machine's code like a plate, one field "Para onde você quer
  ir?", contacts as discreet cards that unfold when the field is focused or the page is
  scrolled, settings behind a single corner icon. Nothing else.
  Props:
    onconnect: (peerId: string) => void
    error?: string (bindable) — last connection error, already in plain words
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import BigCode from '../lib/home/BigCode.svelte'
  import ContactCards from '../lib/home/ContactCards.svelte'
  import GoField from '../lib/home/GoField.svelte'
  import SettingsPanel from '../lib/home/SettingsPanel.svelte'
  import { ContactBook } from '../lib/home/contacts.svelte.ts'
  import { suggest } from '../lib/home/go.ts'
  import { shouldCollapse } from '../lib/home/reveal.ts'

  let { onconnect, error = $bindable('') }: { onconnect: (peerId: string) => void; error?: string } = $props()

  let myId = $state('')
  let status = $state('conectando')
  let query = $state('')
  let focused = $state(false)
  let scrolled = $state(false)
  let goEl: HTMLElement | undefined = $state()
  let cards: ContactCards | undefined = $state()

  const book = new ContactBook()
  const shown = $derived(query.trim() ? suggest(book.list, query) : book.list)
  const revealed = $derived((focused || scrolled || !!query.trim()) && shown.length > 0)

  async function refresh() {
    try {
      myId = await invoke<string>('get_my_id')
      status = await invoke<string>('get_server_status')
    } catch {
      status = 'sem conexão'
    }
  }

  onMount(() => {
    refresh()
    const t = setInterval(refresh, 2000)
    // skip the periodic reload while a control in the cards is in use (an open select)
    const stop = book.start(() => !!goEl?.querySelector('.cards')?.contains(document.activeElement))
    return () => {
      clearInterval(t)
      stop()
    }
  })

  // Pointer pressed inside the field/cards area; cleared after the click has landed.
  let pointerInside = false

  function collapseUnlessHeld() {
    if (!goEl) return
    if (shouldCollapse({ focusWithin: goEl.matches(':focus-within'), pointerInside })) focused = false
  }

  function focusout() {
    // relatedTarget is unreliable (null on WKWebView button clicks): re-check next frame.
    requestAnimationFrame(collapseUnlessHeld)
  }

  function windowdown(e: PointerEvent) {
    pointerInside = !!goEl?.contains(e.target as Node)
    if (pointerInside) focused = true
    else requestAnimationFrame(collapseUnlessHeld)
  }

  function windowup() {
    if (pointerInside) setTimeout(() => (pointerInside = false))
  }

  function wheel(e: WheelEvent) {
    if (e.deltaY > 0) scrolled = true
  }
</script>

<svelte:window onpointerdown={windowdown} onpointerup={windowup} onpointercancel={windowup} />

<main class="home" onwheel={wheel}>
  <SettingsPanel />
  <div class="stage">
    <BigCode code={myId} {status} />
    <div class="go" bind:this={goEl} onfocusin={() => (focused = true)} onfocusout={focusout}>
      <GoField contacts={book.list} bind:query bind:error {onconnect} onarrowdown={() => cards?.focusFirst()} />
      <div class="unfold" class:open={revealed} inert={!revealed}>
        <div class="inner">
          <ContactCards
            bind:this={cards}
            contacts={shown}
            {onconnect}
            onpolicy={(c, key) => book.setPolicy(c, key)}
            onforget={(c) => book.remove(c)}
          />
        </div>
      </div>
      {#if book.error}<p class="book-error" role="alert">{book.error}</p>{/if}
    </div>
  </div>
</main>

<style>
  .home {
    min-height: 100svh;
  }
  .stage {
    display: grid;
    align-content: center;
    justify-items: center;
    gap: clamp(var(--space-5), 7vh, var(--space-9));
    min-height: 100svh;
    padding: clamp(var(--space-5), 8vh, var(--space-9)) var(--space-6);
    box-sizing: border-box;
  }
  .go {
    width: min(100%, 640px);
  }
  /* cards unfold under the field: height animates 0fr → 1fr */
  .unfold {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    transition:
      grid-template-rows var(--dur-slow) var(--ease-out),
      opacity var(--dur-base) var(--ease-out);
  }
  .unfold.open {
    grid-template-rows: 1fr;
    opacity: 1;
  }
  .book-error {
    margin: var(--space-4) 0 0;
    color: var(--danger);
    font-size: var(--fs-body);
  }
  .inner {
    min-height: 0;
    overflow: hidden;
  }
  .open .inner {
    overflow: visible;
    padding-top: var(--space-5);
  }
</style>
