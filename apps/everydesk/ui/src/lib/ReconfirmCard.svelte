<script lang="ts">
  // Periodic "still connected?" ring (cm:reconfirm). The session keeps streaming; unanswered,
  // the engine ends it after `ringSecs` and the parent drops this card at 0.
  // Same card as the access request (cards/RequestCard), different words. "Encerrar" carries
  // `data-autofocus`: an unattended Enter must never keep a session alive.
  import { remainingSeconds } from './cm'
  import { RECONFIRM_LINE } from './cards/cards'
  import RequestCard from './cards/RequestCard.svelte'
  import Button from './ui/Button.svelte'

  let {
    name,
    start,
    ringSecs,
    now,
    offset = 0,
    onanswer,
  }: {
    name: string
    start: number
    ringSecs: number
    now: number
    offset?: number
    onanswer: (ok: boolean) => void
  } = $props()
</script>

<RequestCard
  {name}
  line={RECONFIRM_LINE}
  total={ringSecs}
  remaining={remainingSeconds(start, now, ringSecs)}
  {offset}
>
  {#snippet actions()}
    <Button variant="danger" data-autofocus onclick={() => onanswer(false)}>Encerrar</Button>
    <Button variant="primary" onclick={() => onanswer(true)}>Manter</Button>
  {/snippet}
</RequestCard>
