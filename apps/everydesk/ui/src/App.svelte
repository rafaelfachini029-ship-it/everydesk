<script lang="ts">
  import { onMount } from 'svelte'
  import IncomingCard from './lib/IncomingCard.svelte'
  import ConnectedBadge from './lib/host/ConnectedBadge.svelte'
  import RemoteCanvas from './lib/RemoteCanvas.svelte'
  import { humanizeMsgbox, phaseFor, type ConnEvent } from './lib/connecting/connecting.ts'
  import { close, login, type MsgBox } from './lib/session'
  import Connecting from './routes/Connecting.svelte'
  import Home from './routes/Home.svelte'

  // home → connecting → session. `remote` set means an attempt or a session is open; the
  // session view sits above the connecting screen, clipped to nothing until its first frame,
  // then opens from the centre and the connecting screen is dropped (`revealed`).
  let remote = $state<{ peerId: string; attempt: number; revealed: boolean } | null>(null)
  let events = $state<ConnEvent[]>([])
  const phase = $derived(phaseFor(events))
  let lastError = $state('')
  // Hidden design review page (/#gallery); loaded lazily so its styles never touch this screen.
  let gallery = $state(location.hash.startsWith('#gallery'))
  onMount(() => window.addEventListener('hashchange', () => (gallery = location.hash.startsWith('#gallery'))))

  /** Codes arrive already normalized (GoField → resolveTarget → normalizeCode). No password
   *  here: the host's policy or its card decides, and the connecting screen asks if needed. */
  function connect(id: string) {
    lastError = ''
    events = []
    remote = { peerId: id, attempt: 0, revealed: false }
  }

  function onevent(e: ConnEvent) {
    events = [...events, e]
    // A failed attempt is over: make sure the engine lets go of it too.
    if (phaseFor(events).name === 'erro') close()
  }

  function onclosed(last: MsgBox | null) {
    if (remote && !remote.revealed) {
      // Before the first frame the connecting screen explains what happened.
      onevent({ type: 'closed', box: last })
      return
    }
    remote = null
    if (last && last.kind.includes('error')) lastError = humanizeMsgbox(last.kind, last.text || last.title, { phase: 'live' }).title
  }

  function retry() {
    if (!remote) return
    events = []
    remote = { ...remote, attempt: remote.attempt + 1 }
  }

  function leave() {
    close()
    remote = null
  }

  function answerPassword(pw: string) {
    login(pw)
    events = [...events, { type: 'login' }]
  }
</script>

{#if gallery}
  {#await import('./routes/Gallery.svelte') then m}<m.default />{/await}
{:else if remote}
  {#if !remote.revealed}
    <Connecting
      target={remote.peerId}
      {phase}
      oncancel={leave}
      onretry={retry}
      onback={leave}
      onlogin={answerPassword}
    />
  {/if}
  {#if phase.name !== 'erro'}
    {#key remote.attempt}
      <RemoteCanvas
        peerId={remote.peerId}
        password=""
        {onclosed}
        {onevent}
        onrevealed={() => remote && (remote.revealed = true)}
      />
    {/key}
  {/if}
{:else}
  <Home onconnect={connect} bind:error={lastError} />
{/if}
<IncomingCard />
<ConnectedBadge />
