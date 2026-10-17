/**
 * Motion helpers. Every animation goes through `animate()` so "reduzir movimento" is honoured
 * in one place: with the system preference on, the animation still runs (so `finished` and
 * end states behave the same) but with duration 0.
 */

export const DUR = { fast: 150, base: 250, slow: 400 } as const
export const EASE = {
  out: 'cubic-bezier(0.16, 1, 0.3, 1)',
  spring: 'cubic-bezier(0.34, 1.56, 0.64, 1)',
} as const

type MatchMedia = (q: string) => { matches: boolean }

export function prefersReducedMotion(
  matchMedia: MatchMedia | undefined = globalThis.matchMedia?.bind(globalThis),
): boolean {
  return matchMedia ? matchMedia('(prefers-reduced-motion: reduce)').matches : false
}

/** Options actually passed to the element: duration forced to 0 under reduced motion. */
export function motionOptions(
  opts: KeyframeAnimationOptions = {},
  reduced = prefersReducedMotion(),
): KeyframeAnimationOptions {
  return {
    duration: DUR.base,
    easing: EASE.out,
    fill: 'both',
    ...opts,
    ...(reduced ? { duration: 0, delay: 0, iterations: 1 } : {}),
  }
}

export function animate(
  el: Pick<Element, 'animate'>,
  keyframes: Keyframe[] | PropertyIndexedKeyframes,
  opts?: KeyframeAnimationOptions,
): Animation {
  return el.animate(keyframes, motionOptions(opts))
}
