<!--
  BigCode — this machine's code set like a plate, with a slow neutral glow behind it
  (deliberately not the accent: the accent is for online/connect/pulse/ring only), and the
  server status as one quiet line underneath.
  Props:
    code: string — "" while loading
    status: string — raw `get_server_status` text
-->
<script lang="ts">
  import { CODE_GLYPH_EM, CODE_GUTTERS_PX, CODE_HEIGHT_SHARE, CODE_MAX_PX, CODE_MIN_PX, codeParts, serverStatus } from './bigcode.ts'

  let { code, status }: { code: string; status: string } = $props()

  const parts = $derived(codeParts(code))
  const st = $derived(serverStatus(status))
  // Same formula as codeFontSize(); CSS does the live resizing.
  const size = $derived(
    `clamp(${CODE_MIN_PX}px, min((100vw - ${CODE_GUTTERS_PX}px) / ${Math.max(code.length || 13, 1) * CODE_GLYPH_EM}, ${CODE_HEIGHT_SHARE * 100}vh), ${CODE_MAX_PX}px)`,
  )
</script>

<section class="plate" aria-labelledby="my-code-label">
  <p id="my-code-label" class="eyebrow t-body">Seu código</p>
  <div class="lit">
    <span class="glow" aria-hidden="true"></span>
    {#if parts.length}
      <p id="my-id" class="code" style:font-size={size}>
        {#each parts as p, i (i)}{#if i > 0}<span class="sep">-</span>{/if}<span class="word">{p}</span>{/each}
      </p>
    {:else}
      <p class="code loading" style:font-size={size} aria-label="carregando seu código">···</p>
    {/if}
  </div>
  <p class="status t-body" id="server-status" aria-live="polite">
    <span class="dot" class:on={st.online} aria-hidden="true"></span>{st.label}
  </p>
</section>

<style>
  .plate {
    display: grid;
    justify-items: center;
    gap: var(--space-3);
    min-width: 0;
    text-align: center;
  }
  .eyebrow {
    margin: 0;
    color: var(--text-muted);
  }
  .lit {
    position: relative;
    display: grid;
    place-items: center;
    max-width: 100%;
  }
  /* the plate's light: a soft neutral halo that breathes slowly */
  .glow {
    --halo: rgb(238 242 248 / 0.1);
    position: absolute;
    inset: -45% -12%;
    border-radius: 50%;
    background: radial-gradient(closest-side, var(--halo), transparent);
    pointer-events: none;
    animation: breathe 7s ease-in-out infinite alternate;
  }
  :global([data-theme='light']) .glow {
    --halo: rgb(255 255 255 / 0.95);
  }
  @keyframes breathe {
    from {
      opacity: 0.45;
      transform: scale(0.97);
    }
    to {
      opacity: 1;
      transform: scale(1.03);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .glow {
      animation: none;
      opacity: 0.8;
    }
  }
  .code {
    position: relative;
    margin: 0;
    font-family: var(--font-code);
    font-weight: 500;
    line-height: var(--lh-tight);
    letter-spacing: var(--track-display);
    font-variant-ligatures: none;
    white-space: nowrap;
    color: var(--text);
    user-select: all;
  }
  /* the dashes recede so the words read like a plate */
  .sep {
    color: var(--text-muted);
    opacity: 0.55;
    font-weight: 300;
  }
  .loading {
    color: var(--text-muted);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    color: var(--text-muted);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
    transition:
      background-color var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-out);
  }
  .dot.on {
    background: var(--accent);
    border-color: var(--accent);
  }
</style>
