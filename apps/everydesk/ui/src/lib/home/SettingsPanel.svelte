<!--
  SettingsPanel — the single quiet corner button and what sits behind it: the theme, and what
  someone who accesses this machine may do (four plain switches: files, sound, recording,
  clipboard; `cm_get_permissions` / `cm_set_permission`, which also applies to whoever is
  connected now), and permanent access (`service_status`, kept current by `service:status`;
  `install_permanent` / `uninstall_permanent`, one UAC prompt; Windows only), and "Sobre"
  (version, commit, source code and AGPL-3.0 links; `about_info` / `about_open`, fixed URLs
  chosen in Rust). A disclosure: the gear says whether
  it is open (aria-expanded/aria-controls); Escape or "Fechar" closes it and puts the focus
  back on the gear.
-->
<script lang="ts">
  import Settings from 'lucide-svelte/icons/settings'
  import X from 'lucide-svelte/icons/x'
  import { invoke } from '@tauri-apps/api/core'
  import Button from '../ui/Button.svelte'
  import Toggle from '../cards/Toggle.svelte'
  import { PERMISSION_ROWS, type PermissionName, type Permissions } from '../host/files.ts'
  import { applyTheme, storedTheme, type Theme } from './theme.ts'
  import { listen } from '@tauri-apps/api/event'
  import { SERVICE_EVENT, serviceError, serviceView, type ServiceStatus } from './service.ts'
  import { aboutLine, type About } from './update.ts'

  let about = $state<About | null>(null)
  let aboutError = $state('')

  async function loadAbout() {
    if (about) return
    try {
      about = await invoke<About>('about_info')
    } catch (e) {
      console.warn('everydesk: about_info', e)
    }
  }

  async function openLink(which: 'source' | 'license') {
    aboutError = ''
    try {
      await invoke('about_open', { which })
    } catch (e) {
      aboutError = typeof e === 'string' ? e : 'Não consegui abrir o navegador.'
    }
  }

  let service = $state<ServiceStatus | null>(null)
  let serviceBusy = $state(false)
  let serviceMsg = $state('')
  const view = $derived(serviceView(service))

  async function loadService() {
    try {
      service = await invoke<ServiceStatus>('service_status')
    } catch (e) {
      console.warn('everydesk: service_status', e)
    }
  }

  async function runService() {
    if (!view.action || view.disabled || serviceBusy) return
    serviceBusy = true
    serviceMsg = ''
    try {
      await invoke(view.action.command)
      serviceMsg = 'Pronto. O EveryDesk vai fechar e abrir de novo em alguns segundos.'
    } catch (e) {
      serviceMsg = serviceError(e)
    } finally {
      serviceBusy = false
    }
  }

  $effect(() => {
    const off = listen<ServiceStatus>(SERVICE_EVENT, (e) => (service = e.payload))
    return () => {
      off.then((f) => f())
    }
  })

  let perms = $state<Permissions | null>(null)
  let permError = $state('')

  async function loadPerms() {
    try {
      perms = await invoke<Permissions>('cm_get_permissions')
      permError = ''
    } catch (e) {
      console.warn('everydesk: cm_get_permissions', e)
      permError = 'Não consegui ler as permissões.'
    }
  }

  async function setPerm(name: PermissionName, on: boolean) {
    if (!perms) return
    permError = ''
    try {
      await invoke('cm_set_permission', { name, on })
    } catch (e) {
      perms = { ...perms, [name]: !on }
      permError = `Não consegui mudar: ${e}`
    }
  }

  $effect(() => {
    if (open) {
      loadPerms()
      loadService()
      loadAbout()
    }
  })

  let open = $state(false)
  let theme = $state<Theme>(document.documentElement.dataset.theme === 'light' ? 'light' : storedTheme())
  let root: HTMLElement | undefined = $state()
  const panelId = 'settings-panel'

  function choose(t: Theme) {
    theme = t
    applyTheme(t, true)
  }

  function onwindowdown(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false
  }

  /** Close and hand the keyboard back to the gear. */
  function closeToGear() {
    open = false
    root?.querySelector<HTMLButtonElement>('.gear')?.focus()
  }

  function onwindowkey(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !open) return
    e.preventDefault()
    closeToGear()
  }
</script>

<svelte:window onpointerdown={onwindowdown} onkeydown={onwindowkey} />

<div class="settings" bind:this={root}>
  <Button
    class="gear"
    icon={Settings}
    label="Configurações"
    aria-expanded={open}
    aria-controls={panelId}
    onclick={() => (open = !open)}
  />
  {#if open}
    <div class="panel" id={panelId} role="group" aria-labelledby="{panelId}-title">
      <header>
        <h2 id="{panelId}-title">Configurações</h2>
        <Button icon={X} label="Fechar" onclick={closeToGear} />
      </header>
      <div class="opt">
        <span id="theme-label">Aparência</span>
        <div class="switch" role="group" aria-labelledby="theme-label">
          <button aria-pressed={theme === 'dark'} onclick={() => choose('dark')}>Escuro</button>
          <button aria-pressed={theme === 'light'} onclick={() => choose('light')}>Claro</button>
        </div>
      </div>
      <div class="opt perms" role="group" aria-labelledby="perms-label" aria-describedby="perms-hint">
        <span id="perms-label">Quem acessar esta máquina pode</span>
        {#if perms}
          {#each PERMISSION_ROWS as row (row.name)}
            <Toggle bind:checked={perms[row.name]} onchange={(on) => setPerm(row.name, on)}>{row.label}</Toggle>
          {/each}
        {/if}
        <p id="perms-hint" class="perms-hint">Vale também para quem já está conectado.</p>
        {#if permError}<p class="perms-error" role="alert">{permError}</p>{/if}
      </div>
      <div class="opt service" role="group" aria-labelledby="service-label" aria-describedby="service-hint">
        <span id="service-label">Acesso permanente</span>
        <p class="service-status" data-tone={view.tone} aria-live="polite">
          <span class="dot" aria-hidden="true"></span>{view.status}
        </p>
        {#if view.action}
          <Button
            variant={view.action.variant}
            loading={serviceBusy}
            disabled={view.disabled}
            onclick={runService}>{view.action.label}</Button
          >
        {/if}
        {#if view.hint}<p id="service-hint" class="perms-hint">{view.hint}</p>{/if}
        {#if serviceMsg}<p class="service-msg" role="status">{serviceMsg}</p>{/if}
      </div>
      <div class="opt about" role="group" aria-labelledby="about-label">
        <span id="about-label">Sobre</span>
        {#if about}
          <p class="about-line">{aboutLine(about)}</p>
          <p class="perms-hint">
            Software livre sob a licença {about.license}, baseado no RustDesk. Você pode ler, mudar e
            redistribuir o código.
          </p>
          <div class="links">
            <button class="link" onclick={() => openLink('source')}>Código-fonte</button>
            <button class="link" onclick={() => openLink('license')}>Licença {about.license}</button>
          </div>
        {/if}
        {#if aboutError}<p class="perms-error" role="alert">{aboutError}</p>{/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .settings {
    position: fixed;
    top: var(--space-4);
    right: var(--space-4);
    z-index: 10;
    color: var(--text-muted);
  }
  .settings > :global(.btn) {
    color: var(--text-muted);
  }
  .settings > :global(.btn:hover) {
    color: var(--text);
  }
  .panel {
    position: absolute;
    top: calc(44px + var(--space-2));
    right: 0;
    display: grid;
    gap: var(--space-5);
    width: min(320px, calc(100vw - 2 * var(--space-4)));
    max-height: calc(100svh - 44px - 3 * var(--space-4));
    overflow-y: auto;
    box-sizing: border-box;
    padding: var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-glass);
    backdrop-filter: blur(18px);
    box-shadow: var(--shadow-float);
    color: var(--text);
    transform-origin: top right;
    animation: open var(--dur-base) var(--ease-out);
  }
  @keyframes open {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: calc(-1 * var(--space-2)) calc(-1 * var(--space-3)) 0 0;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-lead);
    font-weight: 600;
    letter-spacing: var(--track-title);
  }
  .opt {
    display: grid;
    gap: var(--space-2);
    font-size: var(--fs-body);
    color: var(--text-muted);
  }
  .perms {
    gap: var(--space-1);
  }
  .perms #perms-label {
    margin-bottom: var(--space-1);
  }
  .perms-hint,
  .perms-error {
    margin: var(--space-1) 0 0;
    font-size: var(--fs-label);
  }
  .perms-hint {
    color: var(--text-muted);
  }
  .perms-error {
    color: var(--danger);
  }
  .service-status {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    margin: 0;
    color: var(--text);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--line-strong);
    transform: translateY(-1px);
  }
  [data-tone='on'] .dot {
    border-color: var(--accent);
    background: var(--accent);
  }
  [data-tone='warn'] .dot {
    border-color: var(--warn);
    background: var(--warn);
  }
  [data-tone='warn'] {
    color: var(--warn);
  }
  .service-msg {
    margin: 0;
    font-size: var(--fs-label);
    color: var(--text);
  }
  .about-line {
    margin: 0;
    font-family: var(--font-code);
    font-size: var(--fs-label);
    color: var(--text);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }
  .link {
    padding: var(--space-1) 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--fs-label);
    text-decoration: underline;
    text-decoration-color: var(--line-strong);
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:hover {
    text-decoration-color: var(--text);
  }
  .link:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
    border-radius: 4px;
  }
  .switch {
    display: grid;
    grid-template-columns: 1fr 1fr;
    padding: var(--space-1);
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    background: var(--surface-2);
  }
  .switch button {
    min-height: 40px;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--fs-body);
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .switch button[aria-pressed='true'] {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.2);
  }
  .switch button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
