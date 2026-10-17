/**
 * Accessed side, Windows portable run: the administrator-permission card (ElevationCard).
 * The backend (cm.rs) decides once per episode: a remembered "Permitir sempre" elevates on its
 * own (`auto`), "não perguntar mais" stays silent, otherwise `ask` shows the card; `done`
 * means the elevated helper runs (or is no longer needed). On macOS/Linux, or when EveryDesk
 * is installed, no event ever comes, so the card never shows.
 * The pure reducer below drives the card; the setting helpers are for the settings panel.
 */
import { invoke } from '@tauri-apps/api/core'

export const ELEVATION_EVENT = 'cm:elevation'

export type ElevationState = 'ask' | 'auto' | 'done'
export type ElevationEvent = { conn_id: number; state: ElevationState }

export type ElevationStep =
  | { type: 'event'; event: ElevationEvent }
  /** `cm:removed`: that session is gone. */
  | { type: 'removed'; conn_id: number }
  /** The owner pressed one of the card's buttons. */
  | { type: 'answered' }
  /** `cm_elevation_pending` on mount (events are not replayed). */
  | { type: 'hydrate'; conn_id: number | null }

/** The session whose card is up, or null (no card). */
export type ElevationCardState = number | null

export function applyElevation(card: ElevationCardState, step: ElevationStep): ElevationCardState {
  switch (step.type) {
    case 'event':
      // One episode for the whole machine: `auto` and `done` both mean no card.
      return step.event.state === 'ask' ? step.event.conn_id : null
    case 'removed':
      return card === step.conn_id ? null : card
    case 'answered':
      return null
    case 'hydrate':
      return card ?? step.conn_id
  }
}

/** Remembered answer: 'Y' elevate without asking, 'N' never ask, '' ask (default). */
export type ElevationSetting = 'Y' | 'N' | ''

export function parseElevationSetting(v: unknown): ElevationSetting {
  return v === 'Y' || v === 'N' ? v : ''
}

export async function getElevationSetting(): Promise<ElevationSetting> {
  return parseElevationSetting(await invoke<string>('cm_elevation_setting'))
}

/** For the settings toggle (SettingsPanel, a later task): '' goes back to asking. */
export async function setElevationSetting(value: ElevationSetting): Promise<void> {
  await invoke('cm_elevation_set_setting', { value })
}

/** "Permitir sempre" (`remember`) or "Só desta vez": Windows shows its own UAC prompt. */
export async function elevate(connId: number, remember: boolean): Promise<void> {
  await invoke('cm_elevate', { connId, remember })
}

/** "Agora não" (this time) or "não perguntar mais" (`rememberNo`). */
export async function dismissElevation(rememberNo: boolean): Promise<void> {
  await invoke('cm_elevation_dismiss', { rememberNo })
}

export const ELEVATION_TITLE = 'Permissão de administrador'
export const ELEVATION_BODY =
  'Para controlar janelas que pedem administrador, o EveryDesk precisa de uma permissão do Windows. Vai aparecer a janela de confirmação do sistema.'
