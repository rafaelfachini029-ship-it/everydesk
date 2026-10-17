<!--
  ChatPanel — the pill's "Conversa" tool and the panel it opens: the session's messages as
  bubbles ("você" / the other side's name) and a field where Enter sends and Shift+Enter breaks
  the line. Escape closes and gives the keyboard back to the remote screen. Like MorePanel it
  is part of the pill's DOM (anchored to the pill's wrapper, outside the tool strip's clip), so
  the pill's key gate (shouldForwardKey: pillFocused) keeps everything typed here off the
  remote, and its dead zone and hide timer cover it.
  A message that arrives while the panel is closed counts as unread on the tool and asks the
  pill to come down expanded (`onattention(true)`) with a short preview under it; after
  PREVIEW_MS it lets the pill go again (`onattention(false)`), unless the pointer or the focus
  is on the pill (the pill's own leave handlers release it then).
  State: chat.ts `chatReducer`, local to this component (it lives as long as the session).
  Props:
    active: boolean — the pill is expanded; the panel closes when it collapses
    who: string — the other side's name (or code)
    onreturn: () => void — give the keyboard back to the remote screen
    onattention: (on: boolean) => void — keep the pill down and expanded / let it go
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'
  import MessageSquare from 'lucide-svelte/icons/message-square'
  import SendHorizontal from 'lucide-svelte/icons/send-horizontal'
  import X from 'lucide-svelte/icons/x'
  import Icon from '../ui/Icon.svelte'
  import {
    CHAT_EMPTY,
    CHAT_EVENTS,
    CHAT_MAX_CHARS,
    chatKey,
    chatReducer,
    formatAt,
    lastReceived,
    outgoing,
    unreadLabel,
    type ChatAction,
    type ChatMsg,
  } from './chat.ts'

  interface Props {
    active: boolean
    who: string
    onreturn: () => void
    onattention: (on: boolean) => void
  }
  let { active, who, onreturn, onattention }: Props = $props()

  /** How long an arrival preview stays under the pill. */
  const PREVIEW_MS = 6000
  /** Consecutive messages from one side closer than this share one name/time line. */
  const GROUP_MS = 120_000

  const uid = $props.id()
  let chat = $state(CHAT_EMPTY)
  let draft = $state('')
  let error = $state<string | null>(null)
  let sending = $state(false)
  let preview = $state<ChatMsg | null>(null)
  let root: HTMLSpanElement
  let field: HTMLTextAreaElement | undefined = $state()
  let list: HTMLOListElement | undefined = $state()
  let previewTimer: ReturnType<typeof setTimeout> | null = null

  const dispatch = (a: ChatAction) => (chat = chatReducer(chat, a))
  const left = $derived(CHAT_MAX_CHARS - [...draft.trim()].length)

  $effect(() => {
    if (!active && chat.open) close(false)
  })

  onMount(() => {
    const un = listen<{ text: string; at: number }>(CHAT_EVENTS.session, (e) => arrive(e.payload.text, e.payload.at))
    const outside = (e: PointerEvent) => {
      if (chat.open && !root.contains(e.target as Node | null)) close(false)
    }
    document.addEventListener('pointerdown', outside, true)
    return () => {
      document.removeEventListener('pointerdown', outside, true)
      un.then((f) => f())
      endPreview(false)
    }
  })

  function arrive(text: string, at: number) {
    dispatch({ type: 'received', text, at: at || Date.now() })
    if (chat.open) return scrollEnd()
    preview = lastReceived(chat)
    if (previewTimer) clearTimeout(previewTimer)
    else onattention(true)
    previewTimer = setTimeout(() => endPreview(true), PREVIEW_MS)
  }

  /** The preview goes; the pill may hide again unless someone is on it. */
  function endPreview(release: boolean) {
    if (previewTimer) clearTimeout(previewTimer)
    const had = previewTimer !== null
    previewTimer = null
    preview = null
    if (!release || !had) return
    // The dock's own pointerleave / focusout release the hide timer when the person leaves.
    const dock = root.closest('.dock')
    if (dock && (dock.matches(':hover') || dock.contains(document.activeElement))) return
    onattention(false)
  }

  async function open() {
    endPreview(false)
    dispatch({ type: 'open' })
    error = null
    await tick()
    scrollEnd()
    field?.focus()
  }

  function close(giveBack: boolean) {
    dispatch({ type: 'close' })
    error = null
    if (giveBack) onreturn()
  }

  function toggle() {
    if (chat.open) close(false)
    else open()
  }

  async function scrollEnd() {
    await tick()
    if (list) list.scrollTop = list.scrollHeight
  }

  async function send() {
    const text = outgoing(draft)
    if (text === null || sending) return
    sending = true
    error = null
    try {
      await invoke('session_send_chat', { text })
      dispatch({ type: 'sent', text, at: Date.now() })
      draft = ''
      scrollEnd()
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
      close(true)
    }
  }

  /** The name/time line shows at the start of each run from one side. */
  function heads(i: number): boolean {
    const m = chat.messages[i]
    const p = chat.messages[i - 1]
    return !p || p.mine !== m.mine || m.at - p.at > GROUP_MS
  }
</script>

<span class="chat" bind:this={root}>
  <button
    class="tool"
    class:on={chat.open}
    aria-label={chat.unread ? `Conversa, ${chat.unread} não lida${chat.unread > 1 ? 's' : ''}` : 'Conversa'}
    aria-expanded={chat.open}
    aria-controls="{uid}-panel"
    onclick={toggle}
  >
    <Icon icon={MessageSquare} />
    {#if chat.unread}<span class="count t-num" aria-hidden="true">{unreadLabel(chat.unread)}</span>{/if}
  </button>

  {#if preview && !chat.open}
    <button class="preview" onclick={open} aria-label="Abrir conversa: {who} disse {preview.text}">
      <span class="meta"><strong>{who}</strong> <span class="t-num">{formatAt(preview.at)}</span></span>
      <span class="ptext">{preview.text}</span>
    </button>
  {/if}

  {#if chat.open}
    <div id="{uid}-panel" class="panel" role="dialog" aria-label="Conversa com {who}">
      <header>
        <span class="title">Conversa com <strong>{who}</strong></span>
        <button class="x" aria-label="Fechar conversa" onclick={() => close(true)}><Icon icon={X} size={18} /></button>
      </header>

      {#if chat.messages.length}
        <ol class="log" bind:this={list} role="log" aria-live="polite" aria-label="Mensagens">
          {#each chat.messages as m, i (m.id)}
            <li class:mine={m.mine} class:head={heads(i)}>
              {#if heads(i)}
                <span class="meta"
                  ><strong>{m.mine ? 'você' : who}</strong> <span class="t-num">{formatAt(m.at)}</span></span
                >
              {/if}
              <span class="bubble">{m.text}</span>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="empty">Escreva para quem está na outra máquina. A mensagem aparece na tela de lá.</p>
      {/if}

      <div class="compose">
        <textarea
          bind:this={field}
          bind:value={draft}
          rows="1"
          maxlength={CHAT_MAX_CHARS + 200}
          placeholder="Escreva uma mensagem"
          aria-label="Mensagem para {who}"
          aria-describedby="{uid}-keys"
          {onkeydown}
        ></textarea>
        <button class="send" aria-label="Enviar" disabled={outgoing(draft) === null || sending} onclick={send}>
          <Icon icon={SendHorizontal} size={18} />
        </button>
      </div>
      <p class="keys" id="{uid}-keys">
        {#if left < 0}
          <span class="over">{-left} caracteres a mais</span>
        {:else if left < 200}
          <span class="t-num">restam {left} caracteres</span>
        {:else}
          Enter envia · Shift+Enter quebra a linha
        {/if}
      </p>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
  {/if}
</span>

<style>
  /* Not positioned: the panel anchors to the pill's wrapper, outside the tool strip's clip. */
  .chat {
    display: inline-flex;
  }
  .tool {
    position: relative;
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
  .tool:focus-visible,
  .x:focus-visible,
  .send:focus-visible,
  .preview:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  /* Unread: a number, in the text colour — information, not a status light. */
  .count {
    position: absolute;
    top: 1px;
    right: 0;
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

  .panel,
  .preview {
    position: absolute;
    top: calc(100% + var(--space-2));
    right: 0;
    z-index: 3;
    box-sizing: border-box;
    border: 1px solid var(--line-strong);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-family: var(--font-ui);
    animation: drop var(--dur-base) var(--ease-out);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }

  /* The arrival: one balloon, the newest words only. */
  .preview {
    display: grid;
    gap: 2px;
    width: min(320px, calc(100vw - 2 * var(--space-4)));
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md) var(--radius-sm) var(--radius-md) var(--radius-md);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .ptext {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    font-size: var(--fs-body);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .panel {
    display: grid;
    width: min(380px, calc(100vw - 2 * var(--space-4)));
    padding: var(--space-2);
    border-radius: var(--radius-lg);
    cursor: default;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-1) var(--space-1) var(--space-2) var(--space-3);
    font-size: var(--fs-label);
  }
  .title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  strong {
    font-weight: 600;
  }
  .x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
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

  .log {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: min(46vh, 360px);
    margin: 0;
    padding: var(--space-1) var(--space-2) var(--space-3);
    overflow-y: auto;
    overscroll-behavior: contain;
    list-style: none;
    border-top: 1px solid var(--line);
  }
  .log li {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    max-width: 100%;
  }
  .log li.head {
    margin-top: var(--space-3);
  }
  .log li.mine {
    align-items: flex-end;
  }
  .meta {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    padding: 0 var(--space-1);
    margin-bottom: 2px;
    font-size: var(--fs-label);
  }
  .meta .t-num {
    color: var(--text-muted);
  }
  /* Theirs: drawn in line, like words on paper. Yours: filled, as already sent. The corner
     nearest the speaker is the sharp one. */
  .bubble {
    max-width: 85%;
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-md);
    font-size: var(--fs-body);
    line-height: 1.4;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .head:not(.mine) .bubble {
    border-top-left-radius: 6px;
  }
  .mine .bubble {
    border-color: transparent;
    background: var(--surface-2);
  }
  .head.mine .bubble {
    border-top-right-radius: 6px;
  }
  .empty {
    margin: 0;
    padding: var(--space-4) var(--space-3);
    border-top: 1px solid var(--line);
    font-size: var(--fs-body);
    line-height: 1.45;
  }

  .compose {
    display: flex;
    align-items: flex-end;
    gap: var(--space-2);
    padding: var(--space-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-md);
  }
  .compose:focus-within {
    border-color: var(--ring);
  }
  textarea {
    flex: 1;
    min-width: 0;
    min-height: 24px;
    max-height: 30vh;
    padding: 4px var(--space-1);
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--fs-body);
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
    width: 36px;
    height: 36px;
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
  .keys {
    margin: var(--space-2) var(--space-2) var(--space-1);
    color: var(--text-muted);
    font-size: var(--fs-label);
  }
  .over,
  .error {
    color: var(--danger);
  }
  .error {
    margin: 0 var(--space-2) var(--space-1);
    font-size: var(--fs-label);
  }
</style>
