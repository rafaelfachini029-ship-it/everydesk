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

  function focusout(e: FocusEvent) {
    if (!goEl?.contains(e.relatedTarget as Node | null)) focused = false
  }

  function wheel(e: WheelEvent) {
    if (e.deltaY > 0) scrolled = true
  }
</script>

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
            error={book.error}
          />
        </div>
      </div>
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
  .inner {
    min-height: 0;
    overflow: hidden;
  }
  .open .inner {
    overflow: visible;
    padding-top: var(--space-5);
  }
</style>
