<!--
  FileTrail — the file sends, inside the pill (spec §6.2 moment 5): one row per job hanging
  under the pill (name, percent, speed, ×), each with its own line of light along the bottom.
  A file the host already has turns its row into the question "Já existe. Substituir?"
  (Sim / Não / Sempre); only that job waits. Finished rows fade after 3 s; errors stay until
  dismissed. A refused send (permission off, no destination) is a one-line note.
  Owns the session's file events (`session:job_added`, `session:job`, `session:override`,
  `session:job_failed`, `session:jobs_dropped`, `session:jobs_cleared`, `session:permission`) and feeds `jobs` (session.ts → jobs.ts).
  Lives in the dock's flow, so the dead zone covers the rows (their buttons never reach
  the remote screen).
  Props:
    onreturn: () => void — give the keyboard back to the remote screen after a click
-->
<script lang="ts">
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import X from 'lucide-svelte/icons/x'
  import Icon from '../ui/Icon.svelte'
  import { EVENTS, answerOverride, cancelJob, dispatchJobs, jobs } from '../session'
  import { jobLine, percent, type Job, type JobAdded, type JobEvent, type OverrideEvent } from './jobs.ts'

  interface Props {
    onreturn: () => void
  }
  let { onreturn }: Props = $props()

  onMount(() => {
    const un: Promise<UnlistenFn>[] = [
      listen<JobAdded>(EVENTS.jobAdded, (e) => dispatchJobs({ type: 'added', ev: e.payload })),
      listen<JobEvent>(EVENTS.job, (e) => dispatchJobs({ type: 'job', ev: e.payload, at: Date.now() })),
      listen<OverrideEvent>(EVENTS.override, (e) => dispatchJobs({ type: 'override', ev: e.payload })),
      listen<{ id: number; message: string }>(EVENTS.jobFailed, (e) =>
        dispatchJobs({ type: 'failed', id: e.payload.id, message: e.payload.message, at: Date.now() }),
      ),
      listen<{ ids: number[] }>(EVENTS.jobsDropped, (e) => dispatchJobs({ type: 'dropped', ids: e.payload.ids })),
      listen(EVENTS.jobsCleared, () => dispatchJobs({ type: 'cleared' })),
      listen<{ name: string; value: boolean }>(EVENTS.permission, (e) =>
        dispatchJobs({ type: 'permission', name: e.payload.name, value: e.payload.value }),
      ),
    ]
    const tick = setInterval(() => dispatchJobs({ type: 'tick', now: Date.now() }), 500)
    return () => {
      clearInterval(tick)
      un.forEach((p) => p.then((f) => f()).catch(() => {}))
    }
  })

  function dismiss(j: Job) {
    if (j.status === 'preparing' || j.status === 'sending' || j.status === 'ask') cancelJob(j.id).catch(() => {})
    dispatchJobs({ type: 'cancel', id: j.id })
    onreturn()
  }
  function answer(j: Job, overwrite: boolean, remember: boolean) {
    if (!j.ask) return
    answerOverride(j.id, j.ask.fileNum, overwrite, remember).catch(() => {})
    dispatchJobs({ type: 'answered', id: j.id, overwrite })
    onreturn()
  }
  const closeLabel = (j: Job) =>
    j.status === 'preparing' || j.status === 'sending' || j.status === 'ask' ? `Cancelar o envio de ${j.name}` : `Dispensar ${j.name}`
</script>

{#if $jobs.jobs.length > 0 || $jobs.note}
  <div class="trail">
    {#if $jobs.note}
      <p class="note" role="status">{$jobs.note}</p>
    {/if}
    {#if $jobs.jobs.length > 0}
      <ul aria-label="Envios">
        {#each $jobs.jobs as j (j.id)}
          {@const p = percent(j)}
          <li class="row {j.status}" style:--p={p === null ? 0 : p / 100}>
            <span class="name" title={j.name}>{j.name}</span>
            <span class="line t-num" role={j.status === 'ask' || j.status === 'error' ? 'status' : undefined}
              >{jobLine(j)}</span
            >
            {#if j.status === 'ask'}
              <span class="choices" role="group" aria-label="Substituir {j.name} na outra máquina?">
                <button class="act yes" onclick={() => answer(j, true, false)}>Sim</button>
                <button class="act" onclick={() => answer(j, false, false)}>Não</button>
                <button class="act" onclick={() => answer(j, true, true)}>Sempre</button>
              </span>
            {/if}
            <button class="close" aria-label={closeLabel(j)} onclick={() => dismiss(j)}>
              <Icon icon={X} size={16} />
            </button>
            <span class="track" aria-hidden="true"><span class="fill"></span></span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  /* Hangs under the pill, as wide as the dock: in the flow, so the dead zone covers it. */
  .trail {
    width: 0;
    min-width: 100%;
    margin-top: var(--space-2);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  ul {
    margin: 0;
    padding: var(--space-1);
    list-style: none;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-md);
    background: var(--surface);
    box-shadow: var(--shadow-float);
  }

  .row {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      'name close'
      'line close'
      'choices choices';
    column-gap: var(--space-2);
    padding: var(--space-2) var(--space-2) var(--space-3) var(--space-3);
    animation: arrive var(--dur-base) var(--ease-out);
  }
  .row + .row {
    border-top: 1px solid var(--line);
  }
  .name {
    grid-area: name;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-label);
    font-weight: 500;
    color: var(--text);
  }
  .line {
    grid-area: line;
    font-family: var(--font-code);
    font-size: var(--fs-label);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row.preparing .line,
  .row.ask .line {
    color: var(--text);
    font-family: var(--font-ui);
  }
  .row.error .line {
    color: var(--danger);
    font-family: var(--font-ui);
    white-space: normal;
  }
  .row.done .line {
    color: var(--accent);
    font-family: var(--font-ui);
  }

  /* each job's own line of light, the pill's signature at row scale */
  .track {
    position: absolute;
    left: var(--space-3);
    right: var(--space-3);
    bottom: 4px;
    height: 2px;
    border-radius: 1px;
    background: var(--accent-track);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
    transform-origin: left center;
    transform: scaleX(var(--p));
    transition: transform var(--dur-slow) var(--ease-out);
  }
  .row.ask .fill {
    background: var(--warn);
    box-shadow: none;
  }
  .row.error .track {
    background: var(--danger-soft);
  }
  .row.error .fill {
    background: var(--danger);
    box-shadow: none;
  }

  .close {
    grid-area: close;
    align-self: center;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .close:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .choices {
    grid-area: choices;
    display: flex;
    gap: var(--space-1);
    margin-top: var(--space-2);
  }
  .act {
    min-height: 32px;
    padding: 0 var(--space-3);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--fs-label);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .act:hover {
    background: var(--surface-2);
  }
  .act.yes {
    background: var(--surface-2);
  }
  .act:focus-visible,
  .close:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  /* done: stays readable, then fades over the last moments of its 3 s (jobs.ts DONE_FADE_MS) */
  .row.done {
    animation: leave 600ms var(--ease-out) 2400ms forwards;
  }

  .note {
    margin: 0;
    padding: var(--space-2) var(--space-4);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-size: var(--fs-label);
    text-align: center;
  }

  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  @keyframes leave {
    to {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .row,
    .row.done {
      animation: none;
    }
    .fill {
      transition: none;
    }
  }
</style>
