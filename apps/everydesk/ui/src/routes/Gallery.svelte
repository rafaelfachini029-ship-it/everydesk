<!--
  Hidden review page: /#gallery (both themes), /#gallery/escuro, /#gallery/claro.
  Shows every base component so Tasks 2–6 build on something already reviewed.
-->
<script lang="ts">
  import '../design/tokens.css'
  import '../design/typography.css'
  import { onDestroy } from 'svelte'
  import ArrowRight from 'lucide-svelte/icons/arrow-right'
  import Folder from 'lucide-svelte/icons/folder'
  import CircleDot from 'lucide-svelte/icons/circle-dot'
  import Monitor from 'lucide-svelte/icons/monitor'
  import MessagesSquare from 'lucide-svelte/icons/messages-square'
  import Gauge from 'lucide-svelte/icons/gauge'
  import PhoneOff from 'lucide-svelte/icons/phone-off'
  import Mic from 'lucide-svelte/icons/mic'
  import Settings from 'lucide-svelte/icons/settings'
  import Button from '../lib/ui/Button.svelte'
  import Field from '../lib/ui/Field.svelte'
  import Icon from '../lib/ui/Icon.svelte'
  import Ring from '../lib/ui/Ring.svelte'
  import Pill from '../lib/ui/Pill.svelte'
  import AccessCard from '../lib/cards/AccessCard.svelte'
  import ReconfirmCard from '../lib/ReconfirmCard.svelte'

  type View = 'ambos' | 'escuro' | 'claro'
  const views: { id: View; name: string }[] = [
    { id: 'ambos', name: 'Os dois' },
    { id: 'escuro', name: 'Escuro' },
    { id: 'claro', name: 'Claro' },
  ]
  const fromHash = (): View => {
    const v = location.hash.split('/')[1]
    return v === 'escuro' || v === 'claro' ? v : 'ambos'
  }
  let view = $state<View>(fromHash())
  function choose(v: View) {
    view = v
    history.replaceState(null, '', v === 'ambos' ? '#gallery' : `#gallery/${v}`)
  }
  const themes = $derived(
    view === 'ambos' ? (['dark', 'light'] as const) : view === 'escuro' ? (['dark'] as const) : (['light'] as const),
  )

  // live countdown for the ring
  let remaining = $state(60)
  const tick = setInterval(() => (remaining = remaining > 0 ? remaining - 1 : 60), 1000)
  onDestroy(() => clearInterval(tick))

  // moment 4: request cards (fixtures)
  let keep = $state({ on: true, policy: 'ask', replaceKey: false })
  const ringNow = 1_000_000

  let destination = $state('')
  let loading = $state(false)
  function fakeConnect() {
    loading = true
    setTimeout(() => (loading = false), 2400)
  }
</script>

<div class="gallery" data-theme={view === 'claro' ? 'light' : 'dark'}>
  <header>
    <h1 class="t-title">Galeria</h1>
    <div class="switch" role="group" aria-label="Tema">
      {#each views as v (v.id)}
        <button type="button" aria-pressed={view === v.id} onclick={() => choose(v.id)}>{v.name}</button>
      {/each}
    </div>
  </header>

  <div class="panels" class:single={themes.length === 1}>
    {#each themes as theme (theme)}
      <section class="panel" data-theme={theme} aria-label={theme === 'dark' ? 'Tema escuro' : 'Tema claro'}>
        <h2 class="t-label eyebrow">{theme === 'dark' ? 'Tema escuro' : 'Tema claro'}</h2>

        <div class="block plate">
          <p class="t-body muted">Seu código</p>
          <p class="t-display code">lua-azul-sete</p>
          <p class="status t-label"><span class="dot" aria-hidden="true"></span>Disponível</p>
        </div>

        <div class="block">
          <Field
            label="Para onde você quer ir?"
            placeholder="Código ou nome do contato"
            hint="O código aparece na tela da outra pessoa."
            bind:value={destination}
          />
          <Field
            label="Para onde você quer ir?"
            value="sol-verde"
            error="Não encontramos esse código. Confira com a outra pessoa."
          />
        </div>

        <div class="block row">
          <Button variant="primary" size="lg" icon={ArrowRight} {loading} onclick={fakeConnect}>
            {loading ? 'Conectando' : 'Conectar'}
          </Button>
          <Button>Cancelar</Button>
          <Button variant="danger" icon={PhoneOff}>Encerrar</Button>
          <Button icon={Settings} label="Configurações" />
          <Button disabled>Indisponível</Button>
        </div>

        <div class="block row">
          <Ring total={60} {remaining} size={72} />
          <Ring total={60} remaining={45} />
          <Ring total={60} remaining={8} />
          <p class="t-body muted">Pedido de acesso: o anel conta 60 segundos.</p>
        </div>

        <div class="block stage">
          {#snippet sessionPill(open: boolean)}
          <Pill label="Controles da sessão" expanded={open}>
            {#snippet summary()}
              <span class="t-num">24 ms</span>
              <span>Notebook da Ana</span>
              <Icon icon={Mic} label="Voz ligada" />
            {/snippet}
            <Button icon={Folder} label="Arquivos" />
            <Button icon={CircleDot} label="Gravar" />
            <Button icon={Monitor} label="Monitores" />
            <Button icon={MessagesSquare} label="Conversa" />
            <Button icon={Gauge} label="Qualidade" />
            <Button variant="danger" icon={PhoneOff} label="Encerrar" />
          </Pill>
          {/snippet}
          {@render sessionPill(false)}
          {@render sessionPill(true)}
          <p class="t-body muted">Passe o mouse ou use Tab para abrir. Fecha sozinha em 2 segundos.</p>
        </div>

        <div class="block cards">
          <p class="t-body muted">Momento 4: pedido de acesso e reconfirmação.</p>
          <AccessCard
            name="Marta Souza"
            code="lua-azul-sete"
            {remaining}
            identity={{ conn_id: 1, state: 'verified', known: true }}
            rememberable={true}
            keyChanged={false}
            remember={keep}
            onremember={(on) => (keep = { ...keep, on })}
            onpolicy={(policy) => (keep = { ...keep, policy })}
            onreplacekey={() => {}}
            onaccept={() => {}}
            ondeny={() => {}}
          />
          <AccessCard
            name="Notebook da Ana"
            code="sol-verde-dois"
            remaining={17}
            identity={{ conn_id: 2, state: 'unverified', known: true }}
            rememberable={false}
            keyChanged={false}
            onremember={() => {}}
            onpolicy={() => {}}
            onreplacekey={() => {}}
            onaccept={() => {}}
            ondeny={() => {}}
          />
          <ReconfirmCard
            name="Marta Souza"
            start={ringNow - 36_000}
            ringSecs={60}
            now={ringNow}
            onanswer={() => {}}
          />
        </div>
      </section>
    {/each}
  </div>
</div>

<style>
  .gallery {
    position: fixed;
    inset: 0;
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    font-family: var(--font-ui);
    font-size: var(--fs-body);
    -webkit-font-smoothing: antialiased;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    flex-wrap: wrap;
    padding: var(--space-6) var(--space-6) var(--space-5);
  }
  h1 {
    margin: 0;
  }
  .switch {
    display: inline-flex;
    padding: var(--space-1);
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
  }
  .switch button {
    min-height: 40px;
    padding: 0 var(--space-4);
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-weight: 500;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .switch button[aria-pressed='true'] {
    background: var(--surface-2);
    color: var(--text);
  }
  .switch button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .panels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 560px), 1fr));
    gap: var(--space-5);
    padding: 0 var(--space-6) var(--space-6);
  }
  .panels.single {
    grid-template-columns: minmax(0, 880px);
    justify-content: center;
  }
  .panel {
    display: grid;
    gap: var(--space-7);
    align-content: start;
    padding: var(--space-7) var(--space-6);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--bg);
    color: var(--text);
  }
  .eyebrow {
    margin: 0;
    color: var(--text-muted);
  }
  .block {
    display: grid;
    gap: var(--space-5);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
  }
  .muted {
    margin: 0;
    color: var(--text-muted);
  }

  .plate {
    gap: var(--space-3);
  }
  .code {
    margin: 0;
    overflow-wrap: anywhere;
    font-size: clamp(2.75rem, 6vw, 5.25rem);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    color: var(--text);
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-track);
  }

  .stage {
    justify-items: start;
    padding: var(--space-6) var(--space-5);
    border-radius: var(--radius-md);
    /* stands in for the remote screen under the pill */
    background:
      linear-gradient(135deg, var(--surface-2), var(--surface) 60%),
      var(--surface);
  }

  .cards {
    justify-items: start;
  }

  @media (max-width: 640px) {
    header,
    .panels {
      padding-inline: var(--space-4);
    }
    .panel {
      padding: var(--space-6) var(--space-4);
    }
  }
</style>
