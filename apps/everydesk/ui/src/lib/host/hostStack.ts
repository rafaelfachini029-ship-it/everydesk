/**
 * The host's top-right card stack (I1): the voice card, the elevation card and any later host
 * card render through one stack, so two never sit on top of each other. Cards are listed in
 * arrival order: the oldest is nearest the corner (top), each newer one below it and one
 * `stackOffset` step further in, like the access cards in the bottom-right corner. The DOM
 * order is the same, so Tab follows the eye. Pure state; the action that moves a card into the
 * stack lives in HostStack.svelte.
 */
import { writable } from 'svelte/store'
import { stackOffset } from '../cards/cards.ts'

/** Card keys, oldest first. */
export type StackList = readonly string[]

export function addCard(list: StackList, key: string): string[] {
  return list.includes(key) ? [...list] : [...list, key]
}

export function removeCard(list: StackList, key: string): string[] {
  return list.filter((k) => k !== key)
}

/** Where the card `key` sits: `corner` counted from the corner (0 = oldest, at the top),
 *  `inset` in px from the right edge (cards.ts `stackOffset`). Null when not in the stack. */
export function placement(list: StackList, key: string): { corner: number; inset: number } | null {
  const corner = list.indexOf(key)
  return corner < 0 ? null : { corner, inset: stackOffset(corner) }
}

export const hostCards = writable<string[]>([])
