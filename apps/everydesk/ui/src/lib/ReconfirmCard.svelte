<script lang="ts">
  // Periodic "still connected?" ring (cm:reconfirm). The session keeps streaming; unanswered,
  // the engine ends it after `ringSecs` and the parent drops this card at 0.
  import { remainingSeconds } from './cm'

  let {
    name,
    start,
    ringSecs,
    now,
    onanswer,
  }: {
    name: string
    start: number
    ringSecs: number
    now: number
    onanswer: (ok: boolean) => void
  } = $props()
</script>

<div class="card">
  <strong>{name} continua conectado. Manter?</strong>
  <div class="actions">
    <button onclick={() => onanswer(true)}>Manter</button>
    <button onclick={() => onanswer(false)}>Encerrar</button>
    <span class="count">{remainingSeconds(start, now, ringSecs)} s</span>
  </div>
</div>

<style>
  .card {
    background: #fff;
    color: #000;
    border: 1px solid #888;
    padding: 8px 10px;
    font-size: 14px;
  }
  .actions {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 6px;
  }
  .count {
    margin-left: auto;
  }
</style>
