/**
 * The "Mais" panel (spec §5.2/§5.3): session switches, quality and remote actions, as pure
 * state so `node --test` can exercise it. The Tauri side is `commands.rs` (`session_toggle`
 * with a closed list of engine option names, `session_quality`, `session_restart`, ...).
 */

/** Engine option names the panel may flip (mirrors `commands.rs::TOGGLES`). */
export type OptionName =
  | 'view-only'
  | 'disable-clipboard'
  | 'disable-audio'
  | 'show-remote-cursor'
  | 'follow-remote-cursor'
  | 'block-input'
  | 'lock-after-session-end'

export type SwitchKey = 'viewOnly' | 'blockInput' | 'audio' | 'clipboard' | 'cursor'

export interface SwitchDef {
  key: SwitchKey
  label: string
  option: OptionName
  /** The switch reads "on" when the option is off (`disable-audio` → "Som" on). */
  inverted: boolean
  /** The `session:permission` name that can take this switch away, if any. */
  permission: string | null
}

export const SWITCHES: readonly SwitchDef[] = [
  { key: 'viewOnly', label: 'Só olhar', option: 'view-only', inverted: false, permission: null },
  {
    key: 'blockInput',
    label: 'Bloquear teclado e mouse de lá',
    option: 'block-input',
    inverted: false,
    permission: 'block_input',
  },
  { key: 'audio', label: 'Som da outra máquina', option: 'disable-audio', inverted: true, permission: 'audio' },
  {
    key: 'clipboard',
    label: 'Área de transferência',
    option: 'disable-clipboard',
    inverted: true,
    permission: 'clipboard',
  },
  {
    key: 'cursor',
    label: 'Mostrar o cursor de lá',
    option: 'show-remote-cursor',
    inverted: false,
    permission: null,
  },
]

export type QualityLevel = 'auto' | 'sharp' | 'smooth'

export const QUALITY_LEVELS: readonly { level: QualityLevel; label: string }[] = [
  { level: 'auto', label: 'automática' },
  { level: 'sharp', label: 'nítida' },
  { level: 'smooth', label: 'fluida' },
]

/** UI level → engine image quality (mirrors `commands.rs::quality_value`). */
export function qualityValue(level: QualityLevel): 'balanced' | 'best' | 'low' {
  return level === 'sharp' ? 'best' : level === 'smooth' ? 'low' : 'balanced'
}

export function isQualityLevel(s: unknown): s is QualityLevel {
  return s === 'auto' || s === 'sharp' || s === 'smooth'
}

export type ActionKey = 'ctrlAltDel' | 'lock' | 'restart'

/** Which `session:permission` takes each remote action away. */
export const ACTION_PERMISSION: Record<ActionKey, string> = {
  ctrlAltDel: 'keyboard',
  lock: 'keyboard',
  restart: 'restart',
}

export const DENIED_NOTE = 'a outra máquina não permite'
export const VIEW_ONLY_NOTE = 'desligado enquanto você só olha'
/** Why a file drop (or the Arquivos tool) does nothing. */
export const VIEW_ONLY_FILES = 'Só olhar está ligado'

export interface ExtrasState {
  /** The session's stored options have been read (`session_get_toggles`). Until then the
   *  view-only state is unknown and input stays closed (fail-closed, Review Focus 5). */
  hydrated: boolean
  /** Raw engine option states (`disable-audio: true` means no sound). */
  options: Partial<Record<OptionName, boolean>>
  /** Permissions the host has taken away (`session:permission` with value false). */
  denied: string[]
  quality: QualityLevel
}

export const EXTRAS_EMPTY: ExtrasState = { hydrated: false, options: {}, denied: [], quality: 'auto' }

export type ExtrasAction =
  /** `session_get_toggles` + `session_get_quality` when the panel opens. */
  | { type: 'hydrate'; options: Record<string, boolean>; quality?: string }
  /** An option's state, from `session_toggle`'s answer (or an optimistic flip). */
  | { type: 'set'; option: OptionName; value: boolean }
  /** `session:block_input {on}`. */
  | { type: 'blockInput'; on: boolean }
  /** `session:permission {name, value}`. */
  | { type: 'permission'; name: string; value: boolean }
  | { type: 'quality'; level: QualityLevel }

const KNOWN: readonly OptionName[] = [
  'view-only',
  'disable-clipboard',
  'disable-audio',
  'show-remote-cursor',
  'follow-remote-cursor',
  'block-input',
  'lock-after-session-end',
]

export function extrasReducer(s: ExtrasState, a: ExtrasAction): ExtrasState {
  switch (a.type) {
    case 'hydrate': {
      const options = { ...s.options }
      for (const [k, v] of Object.entries(a.options))
        if ((KNOWN as readonly string[]).includes(k) && typeof v === 'boolean') options[k as OptionName] = v
      return { ...s, hydrated: true, options, quality: isQualityLevel(a.quality) ? a.quality : s.quality }
    }
    case 'set':
      return { ...s, options: { ...s.options, [a.option]: a.value } }
    case 'blockInput':
      return { ...s, options: { ...s.options, 'block-input': a.on } }
    case 'permission': {
      const denied = s.denied.filter((n) => n !== a.name)
      return { ...s, denied: a.value ? denied : [...denied, a.name] }
    }
    case 'quality':
      return { ...s, quality: a.level }
  }
}

export interface SwitchView {
  checked: boolean
  disabled: boolean
  note: string | null
}

/** How one switch reads: its state, whether it can be used and why not. */
export function switchView(s: ExtrasState, def: SwitchDef): SwitchView {
  const raw = s.options[def.option] ?? false
  let checked = def.inverted ? !raw : raw
  if (def.permission && s.denied.includes(def.permission)) return { checked, disabled: true, note: DENIED_NOTE }
  // Só olhar turns the clipboard off on the host and sends no input at all: these two are
  // meaningless while it is on.
  if (viewOnly(s) && (def.key === 'clipboard' || def.key === 'blockInput')) {
    if (def.key === 'clipboard') checked = false
    return { checked, disabled: true, note: VIEW_ONLY_NOTE }
  }
  return { checked, disabled: false, note: null }
}

export function actionDenied(s: ExtrasState, action: ActionKey): boolean {
  return s.denied.includes(ACTION_PERMISSION[action]) || (action !== 'restart' && viewOnly(s))
}

/** The option name to send when the user flips a switch to `on`. Block input is two
 *  commands, not a toggle. */
export function toggleCommand(def: SwitchDef, on: boolean): string {
  if (def.option === 'block-input') return on ? 'block-input' : 'unblock-input'
  return def.option
}

/** The raw option value the switch will have after the flip (for the optimistic update). */
export function optionAfter(def: SwitchDef, on: boolean): boolean {
  return def.inverted ? !on : on
}

export const viewOnly = (s: ExtrasState) => s.options['view-only'] === true

/** Review Focus 5: with "Só olhar" on — or before the session's options are known (a peer
 *  saved as view-only) — no pointer, key or wheel event leaves this machine. */
export function inputAllowed(g: { viewOnly: boolean; hydrated: boolean }): boolean {
  return g.hydrated && !g.viewOnly
}

/** The gate for the current state. */
export const inputOpen = (s: ExtrasState) => inputAllowed({ viewOnly: viewOnly(s), hydrated: s.hydrated })

/** Sending files (drop or the Arquivos tool) follows the same gate: Só olhar sends nothing. */
export const filesAllowed = inputOpen

/** The pill's quality reading: "automática" / "nítida" / "fluida". */
export function qualityLabel(level: QualityLevel): string {
  return QUALITY_LEVELS.find((q) => q.level === level)?.label ?? 'automática'
}
