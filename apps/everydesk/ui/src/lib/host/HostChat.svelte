<!--
  HostChat — accessed side, one per ConnectedBadge: a "Conversa" button on the badge (with the
  unread count) and a small balloon over it with the last thing the controller wrote and a
  reply field (Enter sends through `cm_send_chat`, Shift+Enter breaks the line, Escape puts the
  balloon away). The balloon comes up by itself when a message arrives; what arrived counts as
  unread until the owner opens it from the badge, goes to its field, or puts it away.
  State: chat.ts `chatReducer` for this connection, fed by `cm:chat` and hydrated from
  `cm_pending().chats` (the last 50 each way are kept by the backend).
  Props:
    conn: Connected — the session this badge stands for
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'
  import MessageSquare from 'lucide-svelte/icons/message-square'
  import SendHorizontal from 'lucide-svelte/icons/send-horizontal'
  import X from 'lucide-svelte/icons/x'
  import Icon from '../ui/Icon.svelte'
  import type { Connected } from '../cm.ts'
  import { badgeLabel } from './badge.ts'
  import {
    CHAT_EMPTY,
    CHAT_EVENTS,
    chatKey,
    chatReducer,
    formatAt,
    lastReceived,
    outgoing,
    unreadLabel,
    type ChatAction,
  } from '../session/chat.ts'

  interface Props {
    conn: Connected
  }
  let { conn }: Props = $props()

  interface CmChat {
    conn_id: number
    text: string
    at: number
    mine: boolean
  }

  const uid = $props.id()
  let chat = $state(CHAT_EMPTY)
  let shown = $state(false)
  let draft = $state('')
  let error = $state<string | null>(null)
  let sending = $state(false)
  let root: HTMLSpanElement
  let field: HTMLTextAreaElement | undefined = $state()
  let balloon: HTMLDivElement | undefined = $state()
  let tail = $state(24)

  // The balloon sits over its own badge (the badge's entry transform makes it the balloon's
  // containing block) and pushes the badges above it up by its height, so it never covers
  // another session's "Encerrar". The tail points at this button.
  $effect(() => {
    const b = balloon
    const badge = root.parentElement
    if (!b || !badge) return
    const fit = () => {
      badge.style.marginTop = `${b.offsetHeight + 14}px`
      tail = root.getBoundingClientRect().left - badge.getBoundingClientRect().left + 12
    }
    const ro = new ResizeObserver(fit)
    ro.observe(b)
    fit()
    return () => {
      ro.disconnect()
      badge.style.marginTop = ''
    }
  })

  const dispatch = (a: ChatAction) => (chat = chatReducer(chat, a))
  const who = $derived(badgeLabel(conn))
  const last = $derived(lastReceived(chat))
  /** The owner's last reply, when it came after the controller's last message. */
  const reply = $derived.by(() => {
    const m = chat.messages.at(-1)
    return m?.mine ? m : null
  })

  onMount(() => {
    const un = listen<CmChat>(CHAT_EVENTS.cm, (e) => {
      if (e.payload.conn_id !== conn.conn_id || e.payload.mine) return
      dispatch({ type: 'received', text: e.payload.text, at: e.payload.at || Date.now() })
      shown = true
    })
    un.then(() => invoke<{ chats?: CmChat[] }>('cm_pending'))
      .then((snap) => {
        const mine = (snap?.chats ?? []).filter((m) => m.conn_id === conn.conn_id)
        if (!mine.length) return
        dispatch({ type: 'hydrate', messages: mine })
        if (chat.unread) shown = true
      })
      .catch((e) => console.error('cm_pending chats', e))
    return () => {
      un.then((f) => f())
    }
  })

  /** The owner is looking at it: what arrived is read; new ones count again once they leave. */
  function engage() {
    if (!chat.open) dispatch({ type: 'open' })
  }
  function onfocusout(e: FocusEvent) {
    if (!root.contains(e.relatedTarget as Node | null)) dispatch({ type: 'close' })
  }

  async function toggle() {
    if (shown) return putAway()
    shown = true
    engage()
    await tick()
    field?.focus()
  }

  function putAway() {
    shown = false
    error = null
    dispatch({ type: 'open' })
    dispatch({ type: 'close' })
  }

  async function send() {
    const text = outgoing(draft)
    if (text === null || sending) return
    sending = true
    error = null
    try {
      await invoke('cm_send_chat', { connId: conn.conn_id, text })
      dispatch({ type: 'sent', text, at: Date.now() })
      draft = ''
    } catch (e) {
      error = `Não foi enviada: ${String(e)}`
    } finally {
      sending = false
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const k = chatKey(e)
    if (k === 'send') {
      e.preventDefault()
      send()
    } else if (k === 'close') {
      e.preventDefault()
      putAway()
      root.querySelector<HTMLElement>('.open')?.focus()
    }
  }
</script>

<span class="hc" bind:this={root} {onfocusout}>
  <button
    class="open"
    class:on={shown}
    aria-label={chat.unread ? `Conversa, ${chat.unread} não lida${chat.unread > 1 ? 's' : ''}` : 'Conversa'}
    aria-expanded={shown}
    aria-controls="{uid}-balloon"
    onclick={toggle}
  >
    <Icon icon={MessageSquare} size={18} />
    {#if chat.unread}<span class="count t-num" aria-hidden="true">{unreadLabel(chat.unread)}</span>{/if}
  </button>

  {#if shown}
    <div bind:this={balloon} id="{uid}-balloon" class="balloon" style:--tail="{tail}px" role="group" aria-label="Conversa com {who}">
      <div class="top">
        {#if last}
          <span class="meta"><strong>{who}</strong> <span class="t-num">{formatAt(last.at)}</span></span>
        {:else}
          <span class="meta">Escreva para <strong>{who}</strong></span>
        {/if}
        <button class="x" aria-label="Guardar conversa" onclick={putAway}
          ><Icon icon={X} size={16} /></button
        >
      </div>
      {#if last}<p class="said" aria-live="polite">{last.text}</p>{/if}
      {#if reply}
        <p class="reply"><span class="you">você</span> {reply.text}</p>
      {/if}
      <div class="compose">
        <textarea
          bind:this={field}
          bind:value={draft}
          rows="1"
          placeholder="Responder"
          aria-label="Resposta para {who}"
          onfocus={engage}
          {onkeydown}
        ></textarea>
        <button class="send" aria-label="Enviar" disabled={outgoing(draft) === null || sending} onclick={send}>
          <Icon icon={SendHorizontal} size={16} />
        </button>
      </div>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
  {/if}
</span>

<style>
  /* Not positioned: the balloon anchors to the badge. */
  .hc {
    display: inline-flex;
    flex: none;
  }
  .open {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .open:hover,
  .open.on {
    background: var(--surface-2);
  }
  .open:focus-visible,
  .x:focus-visible,
  .send:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  /* Unread: a number in the text colour; the accent stays the badge's "live" dot. */
  .count {
    position: absolute;
    top: -3px;
    right: -3px;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    box-sizing: border-box;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    line-height: 16px;
    text-align: center;
  }

  /* A speech balloon over the badge; its tail points down at the button that brought it. */
  .balloon {
    position: absolute;
    bottom: calc(100% + 14px);
    left: 0;
    z-index: 1;
    box-sizing: border-box;
    display: grid;
    gap: var(--space-2);
    width: min(300px, calc(100vw - 2 * var(--space-4)));
    padding: var(--space-3);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-md);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-size: var(--fs-body);
    animation: rise var(--dur-base) var(--ease-out);
  }
  .balloon::after {
    content: '';
    position: absolute;
    top: 100%;
    left: var(--tail);
    width: 12px;
    height: 12px;
    margin-top: -6px;
    box-sizing: border-box;
    border-right: 1px solid var(--line-strong);
    border-bottom: 1px solid var(--line-strong);
    background: var(--surface);
    transform: rotate(45deg);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    margin: -4px -4px 0 0;
  }
  .meta {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    font-size: var(--fs-label);
    white-space: nowrap;
  }
  .meta .t-num {
    color: var(--text-muted);
  }
  strong {
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 600;
  }
  .x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
  .x:hover {
    background: var(--surface-2);
  }
  .said {
    max-height: 9.8em;
    margin: 0;
    overflow-y: auto;
    line-height: 1.4;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .reply {
    display: -webkit-box;
    margin: 0;
    padding-left: var(--space-3);
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    border-left: 2px solid var(--line-strong);
    font-size: var(--fs-label);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .you {
    font-weight: 600;
  }
  .compose {
    display: flex;
    align-items: flex-end;
    gap: var(--space-2);
    padding: 4px 4px 4px var(--space-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
  }
  .compose:focus-within {
    border-color: var(--ring);
  }
  textarea {
    flex: 1;
    min-width: 0;
    min-height: 24px;
    max-height: 8em;
    padding: 4px 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--fs-label);
    line-height: 1.4;
    resize: none;
    field-sizing: content;
  }
  textarea:focus {
    outline: none;
  }
  textarea::placeholder {
    color: var(--text-muted);
  }
  .send {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }
  .send:disabled {
    border-style: dashed;
    background: transparent;
    color: var(--text-muted);
    cursor: default;
  }
  .error {
    margin: 0;
    color: var(--danger);
    font-size: var(--fs-label);
  }
</style>
