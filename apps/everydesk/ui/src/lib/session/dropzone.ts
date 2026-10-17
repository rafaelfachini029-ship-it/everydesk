/**
 * Pure state for the file drop zone (spec §6.2 moment 5). Stage 3 is visual only: nothing
 * here reads or sends a file. No DOM or Tauri dependency so `node --test` can exercise it.
 */
import { writable } from 'svelte/store'

/** What the drag events tell the counter. */
export type DragStep = 'enter' | 'leave' | 'drop' | 'reset'

/**
 * `dragenter`/`dragleave` fire for every element the pointer crosses, children included
 * (entering a child fires its `dragenter` before the parent's `dragleave`), so a boolean
 * flickers. Count instead: the drag is over the window while the depth is above 0. A drop,
 * or anything that ends the drag, resets it.
 */
export function dragDepth(depth: number, step: DragStep): number {
  switch (step) {
    case 'enter':
      return depth + 1
    case 'leave':
      return Math.max(0, depth - 1)
    default:
      return 0
  }
}

/** The veil and the destination show only while something is being dragged over the window. */
export function overlayActive(depth: number): boolean {
  return depth > 0
}

/** Only drags that carry files count (not text or a link dragged from a page). */
export function hasFiles(dt: { types?: ArrayLike<string> | null } | null | undefined): boolean {
  const types = dt?.types
  if (!types) return false
  for (let i = 0; i < types.length; i++) if (types[i] === 'Files') return true
  return false
}

/** "Solte para enviar para Mãe"; the code when there is no contact name. */
export function dropTarget(name: string | null | undefined, peerId: string): string {
  return `Solte para enviar para ${name?.trim() || peerId}`
}

/** The trail in the pill fills over this long… */
export const TRAIL_FILL_MS = 1200
/** …then the note stays this long and everything fades. */
export const TRAIL_NOTE_MS = 2800
export const TRAIL_NOTE = 'Transferência de arquivos chega em breve'

export type TrailPhase = 'idle' | 'fill' | 'note'

/** Where the "em breve" trail is, `elapsed` ms after the drop. */
export function trailPhase(elapsed: number): TrailPhase {
  if (elapsed < 0) return 'idle'
  if (elapsed < TRAIL_FILL_MS) return 'fill'
  if (elapsed < TRAIL_FILL_MS + TRAIL_NOTE_MS) return 'note'
  return 'idle'
}

/** Shared between DropZone (writes) and the pill's FileTrail (reads). */
export const trail = writable<TrailPhase>('idle')
