/**
 * Accessed side: files the controller sends to this machine (C1: the owner sees every one).
 * `cm:file {conn_id, id, name, size, state, at}` arrives once per state change of a transfer
 * (never per progress tick); `cm_pending().files` holds the last 20. The badge says the latest
 * for NOTE_MS ("Mãe enviou foto.jpg (2,1 MB)") and keeps a count of what arrived.
 * Pure state, no DOM or Tauri, so `node --test` can run it.
 */

export const FILE_EVENT = 'cm:file'
/** The line in the badge fades after this long. */
export const NOTE_MS = 6000

export type FileState = 'started' | 'done' | 'cancelled' | 'error'

export interface CmFile {
  conn_id: number
  id: number
  name: string
  size: number
  state: FileState
  /** Unix ms. */
  at: number
}

/** Per connection: the latest transfer (when it was seen here) and how many arrived. */
export interface HostFiles {
  last: { file: CmFile; seenAt: number } | null
  received: number
}

export const NO_FILES: HostFiles = { last: null, received: 0 }

/** One `cm:file` for this connection, seen at `now`. */
export function applyFile(s: HostFiles, f: CmFile, now: number): HostFiles {
  return { last: { file: f, seenAt: now }, received: s.received + (f.state === 'done' ? 1 : 0) }
}

/** The connection's state from `cm_pending().files` (oldest first): the count of what arrived,
 *  and the latest as the note only while it is still recent. */
export function hydrateFiles(list: readonly CmFile[] | undefined, connId: number, now: number): HostFiles {
  const mine = (list ?? []).filter((f) => f.conn_id === connId)
  const latest = mine[mine.length - 1]
  return {
    last: latest && now - latest.at < NOTE_MS ? { file: latest, seenAt: latest.at } : null,
    received: mine.filter((f) => f.state === 'done').length,
  }
}

/** The note to show at `now`, or null once it has faded. */
export function currentNote(s: HostFiles, now: number): CmFile | null {
  return s.last && now - s.last.seenAt < NOTE_MS ? s.last.file : null
}

/** "2,1 MB", "340 KB", "12 bytes"; empty for an unknown size. */
export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return ''
  if (bytes < 1024) return `${Math.round(bytes)} ${bytes < 2 ? 'byte' : 'bytes'}`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`
  const units = ['MB', 'GB', 'TB']
  let v = bytes / (1024 * 1024)
  let u = 0
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024
    u++
  }
  return `${(v < 100 ? v.toFixed(1) : Math.round(v).toString()).replace('.', ',')} ${units[u]}`
}

/** What the badge says after the name ("Mãe" + this). */
export function fileVerb(f: Pick<CmFile, 'name' | 'size' | 'state'>): string {
  const size = formatSize(f.size)
  const what = size ? `${f.name} (${size})` : f.name
  switch (f.state) {
    case 'started':
      return `está enviando ${what}`
    case 'done':
      return `enviou ${what}`
    case 'cancelled':
      return `— envio de ${f.name} cancelado`
    case 'error':
      return `— ${f.name} não chegou inteiro`
  }
}

/** The counter's accessible text. */
export function receivedLabel(n: number): string {
  return n === 1 ? '1 arquivo recebido nesta sessão' : `${n} arquivos recebidos nesta sessão`
}

// ---- host permissions (settings) -------------------------------------------------------

export type PermissionName = 'file' | 'audio' | 'recording' | 'clipboard'
export type Permissions = Record<PermissionName, boolean>

/** The four switches in the settings, in order, with their plain names. */
export const PERMISSION_ROWS: readonly { name: PermissionName; label: string }[] = [
  { name: 'file', label: 'Receber arquivos' },
  { name: 'audio', label: 'Compartilhar o som' },
  { name: 'recording', label: 'Permitir gravar' },
  { name: 'clipboard', label: 'Área de transferência' },
]
