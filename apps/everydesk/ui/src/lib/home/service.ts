// Permanent access (spec 4.6): what the "Acesso permanente" section of the settings says and
// offers for the `service_status` the app reports (`service:status` on every change).
export type ServiceMode = 'portable' | 'installed' | 'installed-stopped'
export interface ServiceStatus {
  mode: ServiceMode
  platform_supported: boolean
}
export const SERVICE_EVENT = 'service:status'

export type ServiceCommand = 'install_permanent' | 'uninstall_permanent'
export interface ServiceView {
  /** Dot beside the status: on = the service hosts, warn = it should but is stopped. */
  tone: 'off' | 'on' | 'warn'
  status: string
  hint: string
  action: { label: string; command: ServiceCommand; variant: 'neutral' | 'ghost' } | null
  disabled: boolean
}

export const UNSUPPORTED_HINT = 'Disponível no Windows nesta versão.'

export function serviceView(s: ServiceStatus | null): ServiceView {
  if (!s) return { tone: 'off', status: 'Verificando…', hint: '', action: null, disabled: true }
  const install = { label: 'Instalar para acesso permanente', command: 'install_permanent', variant: 'neutral' } as const
  if (!s.platform_supported)
    return {
      tone: 'off',
      status: 'Portátil: funciona enquanto o app está aberto.',
      hint: UNSUPPORTED_HINT,
      action: install,
      disabled: true,
    }
  switch (s.mode) {
    case 'installed':
      return {
        tone: 'on',
        status: 'Instalado: inicia com o Windows e funciona na tela de bloqueio.',
        hint: 'Remover pede permissão de administrador. Depois, o acesso volta a funcionar só com o app aberto.',
        action: { label: 'Remover acesso permanente', command: 'uninstall_permanent', variant: 'ghost' },
        disabled: false,
      }
    case 'installed-stopped':
      return {
        tone: 'warn',
        status: 'Serviço parado.',
        hint: 'Enquanto isso, o acesso funciona só com o app aberto. Reiniciar pede permissão de administrador.',
        action: { label: 'Reiniciar serviço', command: 'install_permanent', variant: 'neutral' },
        disabled: false,
      }
    default:
      return {
        tone: 'off',
        status: 'Portátil: funciona enquanto o app está aberto.',
        hint: 'Pede permissão de administrador uma única vez. O EveryDesk fecha e abre de novo sozinho, já instalado.',
        action: install,
        disabled: false,
      }
  }
}

/** Text shown when the command fails (UAC refused, or not on Windows). */
export function serviceError(e: unknown): string {
  const msg = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
  return msg || 'Não consegui concluir. Tente de novo.'
}
