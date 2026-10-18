// Auto-update card (Etapa 5, Task 7): what the discreet "Nova versão x.y.z. Atualizar agora?"
// card says for the `update_status` the app reports (`update:status` on every change), plus the
// two moments only the UI knows: the user said "Depois", or someone is connected and the click
// needs a second, explicit "Encerrar e atualizar".
export type UpdateRoute = 'disabled' | 'installer' | 'portable'
export type UpdatePhase = 'idle' | 'available' | 'downloading' | 'installing' | 'failed'
export interface UpdateStatus {
  route: UpdateRoute
  phase: UpdatePhase
  version: string | null
  progress: number | null
  message: string | null
}
export const UPDATE_EVENT = 'update:status'
/** `update_install` error when a session is open and `force` was false. */
export const SESSION_ACTIVE = 'session-active'

export interface UpdateCardState {
  status: UpdateStatus | null
  /** A session is open: the next click ends it (`update_install(force: true)`). */
  confirming: boolean
  /** "Depois" for this version: hidden until a newer one shows up. */
  dismissed: string | null
  /** An invoke failure the backend did not turn into a status (e.g. nothing pending). */
  error: string
}

export type UpdateAction =
  | { type: 'status'; status: UpdateStatus }
  | { type: 'later' }
  | { type: 'session-active' }
  | { type: 'cancel' }
  | { type: 'error'; message: string }

export const initialUpdateCard: UpdateCardState = { status: null, confirming: false, dismissed: null, error: '' }

export function updateReducer(s: UpdateCardState, a: UpdateAction): UpdateCardState {
  switch (a.type) {
    case 'status': {
      // a newer version than the one put off brings the card back
      const dismissed = a.status.version && a.status.version !== s.dismissed ? null : s.dismissed
      const confirming = s.confirming && a.status.phase === 'available'
      return { ...s, status: a.status, dismissed, confirming, error: a.status.phase === 'failed' ? '' : s.error }
    }
    case 'later':
      return { ...s, confirming: false, error: '', dismissed: s.status?.version ?? s.dismissed }
    case 'session-active':
      return { ...s, confirming: true, error: '' }
    case 'cancel':
      return { ...s, confirming: false }
    case 'error':
      return { ...s, confirming: false, error: a.message || 'Não consegui atualizar. Tente de novo mais tarde.' }
  }
}

export interface UpdateCardView {
  visible: boolean
  title: string
  body: string
  /** Main button: what it says and whether it ends the open session. */
  primary: { label: string; force: boolean } | null
  secondary: string | null
  busy: boolean
  /** 0-100 while downloading with a known size, else null. */
  progress: number | null
  tone: 'quiet' | 'warn' | 'danger'
}

const hidden: UpdateCardView = {
  visible: false,
  title: '',
  body: '',
  primary: null,
  secondary: null,
  busy: false,
  progress: null,
  tone: 'quiet',
}

export function updateCardView(s: UpdateCardState): UpdateCardView {
  const st = s.status
  if (!st || st.route === 'disabled' || st.phase === 'idle' || !st.version) return hidden
  const v = st.version
  if (st.phase === 'available' && s.dismissed === v) return hidden
  const how =
    st.route === 'installer'
      ? 'O Windows pede permissão de administrador; o EveryDesk reabre sozinho.'
      : 'O EveryDesk fecha e abre de novo, já atualizado.'
  if (s.error)
    return { ...hidden, visible: true, title: 'Atualização não instalada.', body: s.error, primary: { label: 'Tentar de novo', force: false }, secondary: 'Depois', tone: 'danger' }
  switch (st.phase) {
    case 'available':
      if (s.confirming)
        return {
          ...hidden,
          visible: true,
          title: 'Há uma sessão ativa.',
          body: 'Ela será encerrada para atualizar. Quem estiver conectado vai ver a conexão cair.',
          primary: { label: 'Encerrar e atualizar', force: true },
          secondary: 'Cancelar',
          tone: 'warn',
        }
      return { ...hidden, visible: true, title: `Nova versão ${v}.`, body: `Atualizar agora? ${how}`, primary: { label: 'Atualizar agora', force: false }, secondary: 'Depois' }
    case 'downloading':
      return {
        ...hidden,
        visible: true,
        title: `Baixando a versão ${v}…`,
        body: st.progress == null ? 'Conferindo a assinatura ao terminar.' : `${st.progress}%. Conferindo a assinatura ao terminar.`,
        busy: true,
        progress: st.progress,
      }
    case 'installing':
      return { ...hidden, visible: true, title: `Instalando a versão ${v}…`, body: how, busy: true }
    case 'failed':
      return {
        ...hidden,
        visible: true,
        title: 'Atualização não instalada.',
        body: st.message || 'Não consegui atualizar. Tente de novo mais tarde.',
        primary: { label: 'Tentar de novo', force: false },
        secondary: 'Depois',
        tone: 'danger',
      }
  }
  return hidden
}

export interface About {
  version: string
  commit: string | null
  source_url: string
  license: string
}

/** "Versão 0.1.0 · commit 82b42c3ea" (or "· compilação local" without a commit). */
export function aboutLine(a: About): string {
  return `Versão ${a.version} · ${a.commit ? `commit ${a.commit}` : 'compilação local'}`
}
