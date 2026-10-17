<!--
  AccessCard — the access request (accessed side), presentational. IncomingCard.svelte owns the
  events/commands and passes state in; the Gallery renders it with fixtures.
  Props:
    name, code, isFileTransfer, remaining (s), offset (stack inset, px)
    identity?: Identity — cm:identity; missing counts as unverified
    rememberable: boolean — canRemember(identity, offer): "lembrar" is never shown otherwise
    keyChanged: boolean — the offer says this id was paired under another key
    remember?: { on, policy, replaceKey } — current answers
    error?: string — failed accept; the card stays
    busy?: boolean — accept in flight
    onremember(on), onpolicy(key), onreplacekey(on), onaccept(), ondeny()
-->
<script lang="ts">
  import ShieldCheck from 'lucide-svelte/icons/shield-check'
  import ShieldAlert from 'lucide-svelte/icons/shield-alert'
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert'
  import { CLICK_APPROVE_TIMEOUT_SECS, type Identity } from '../cm'
  import PolicySelect from '../PolicySelect.svelte'
  import Button from '../ui/Button.svelte'
  import Icon from '../ui/Icon.svelte'
  import { FILES_LINE, INCOMING_LINE, identityLine } from './cards'
  import RequestCard from './RequestCard.svelte'
  import Toggle from './Toggle.svelte'

  interface Props {
    name: string
    code: string
    isFileTransfer?: boolean
    remaining: number
    offset?: number
    identity?: Identity
    rememberable: boolean
    keyChanged: boolean
    remember?: { on: boolean; policy: string; replaceKey: boolean }
    error?: string
    busy?: boolean
    onremember: (on: boolean) => void
    onpolicy: (key: string) => void
    onreplacekey: (on: boolean) => void
    onaccept: () => void
    ondeny: () => void
  }
  let {
    name,
    code,
    isFileTransfer = false,
    remaining,
    offset = 0,
    identity,
    rememberable,
    keyChanged,
    remember,
    error = '',
    busy = false,
    onremember,
    onpolicy,
    onreplacekey,
    onaccept,
    ondeny,
  }: Props = $props()

  const id = $derived(identityLine(identity, name))
  const icons = { ok: ShieldCheck, warn: TriangleAlert, danger: ShieldAlert }
</script>

<RequestCard
  {name}
  {code}
  line={isFileTransfer ? FILES_LINE : INCOMING_LINE}
  total={CLICK_APPROVE_TIMEOUT_SECS}
  {remaining}
  {offset}
>
  <p class="identity t-label {id.tone}">
    <Icon icon={icons[id.tone]} size={18} />
    <span>{id.text}</span>
  </p>

  {#if rememberable && remember}
    <div class="remember">
      {#if keyChanged}
        <p class="note t-label danger">
          A chave desta máquina mudou. Só marque "lembrar" se tiver certeza.
        </p>
      {/if}
      <Toggle checked={remember.on} onchange={onremember}>
        Lembrar este contato
        {#snippet hint()}
          {#if remember.on}
            <span class="next t-label">
              <span class="muted">Da próxima vez:</span>
              <PolicySelect value={remember.policy} onchange={onpolicy} />
            </span>
          {/if}
        {/snippet}
      </Toggle>
      {#if keyChanged && remember.on}
        <Toggle tone="danger" checked={remember.replaceKey} onchange={onreplacekey}>
          Substituir a chave guardada
          {#snippet hint()}
            <span class="muted t-label">Só se a pessoa confirmou que trocou de computador.</span>
          {/snippet}
        </Toggle>
      {/if}
    </div>
  {/if}

  {#if error}<p class="note t-label danger" role="alert">{error}</p>{/if}

  {#snippet actions()}
    <Button onclick={ondeny}>Recusar</Button>
    <Button variant="primary" loading={busy} onclick={onaccept}>Aceitar</Button>
  {/snippet}
</RequestCard>

<style>
  p {
    margin: 0;
  }
  .identity {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  /* verified: quiet, text colour (the accent is reserved) */
  .identity.ok {
    color: var(--text);
  }
  .identity.warn {
    color: var(--warn);
  }
  /* someone claims a known code without its key: loud */
  .identity.danger {
    align-items: flex-start;
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--danger);
    border-radius: var(--radius-md);
    background: var(--danger-soft);
    color: var(--danger);
    font-weight: 600;
  }
  .identity.danger :global(.icon) {
    margin-top: 1px;
  }

  .remember {
    display: grid;
    gap: var(--space-3);
    padding-top: var(--space-4);
    border-top: 1px solid var(--line);
  }
  .next {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 var(--space-3);
  }
  .muted {
    color: var(--text-muted);
  }
  .danger {
    color: var(--danger);
  }

  /* warning tone (unverified): amber, local to the request card; >= 4.5:1 on its surfaces */
  :global(:root),
  :global([data-theme='dark']) {
    --warn: #f2c46d;
  }
  :global([data-theme='light']) {
    --warn: #8a5300;
  }
</style>
