<script lang="ts">
  import { onMount } from 'svelte'
  import IncomingCard from './lib/IncomingCard.svelte'
  import RemoteCanvas from './lib/RemoteCanvas.svelte'
  import { humanError } from './lib/home/go.ts'
  import type { MsgBox } from './lib/session'
  import Home from './routes/Home.svelte'

  // home ⇄ session: `remote` set means a session is open.
  let remote = $state<{ peerId: string; password: string } | null>(null)
  let lastError = $state('')
  // Hidden design review page (/#gallery); loaded lazily so its styles never touch this screen.
  let gallery = $state(location.hash.startsWith('#gallery'))
  onMount(() => window.addEventListener('hashchange', () => (gallery = location.hash.startsWith('#gallery'))))

  /** Codes arrive already normalized (GoField → resolveTarget → normalizeCode). No password
   *  here: the host's policy or its card decides, and RemoteCanvas asks if one is needed. */
  function connect(id: string) {
    lastError = ''
    remote = { peerId: id, password: '' }
  }

  function onclosed(last: MsgBox | null) {
    remote = null
    if (last && last.kind.includes('error')) lastError = humanError(`${last.title} ${last.text}`)
  }
</script>

{#if gallery}
  {#await import('./routes/Gallery.svelte') then m}<m.default />{/await}
{:else if remote}
  <RemoteCanvas peerId={remote.peerId} password={remote.password} {onclosed} />
{:else}
  <Home onconnect={connect} bind:error={lastError} />
{/if}
<IncomingCard />
