import assert from 'node:assert/strict'
import { test } from 'node:test'
import { animate, motionOptions, prefersReducedMotion } from '../src/design/motion.ts'

const media = (matches: boolean) => (q: string) => ({
  matches: q === '(prefers-reduced-motion: reduce)' && matches,
})

function fakeEl() {
  const calls: KeyframeAnimationOptions[] = []
  return {
    calls,
    animate: (_k: unknown, o?: number | KeyframeAnimationOptions) => {
      calls.push(o as KeyframeAnimationOptions)
      return {} as Animation
    },
  }
}

test('prefersReducedMotion reads the system query', () => {
  assert.equal(prefersReducedMotion(media(true)), true)
  assert.equal(prefersReducedMotion(media(false)), false)
  assert.equal(prefersReducedMotion(undefined), false)
})

test('reduced motion forces duration 0 and drops delay/loops', () => {
  const o = motionOptions({ duration: 400, delay: 100, iterations: Infinity }, true)
  assert.equal(o.duration, 0)
  assert.equal(o.delay, 0)
  assert.equal(o.iterations, 1)
})

test('normal motion keeps the requested duration, defaults to 250 ms', () => {
  assert.equal(motionOptions({ duration: 150 }, false).duration, 150)
  assert.equal(motionOptions({}, false).duration, 250)
})

test('animate() passes duration 0 to the element when the system asks for less motion', () => {
  const saved = globalThis.matchMedia
  try {
    globalThis.matchMedia = media(true) as unknown as typeof globalThis.matchMedia
    const el = fakeEl()
    animate(el, [{ opacity: 0 }, { opacity: 1 }], { duration: 400 })
    assert.equal(el.calls[0].duration, 0)

    globalThis.matchMedia = media(false) as unknown as typeof globalThis.matchMedia
    animate(el, [{ opacity: 0 }, { opacity: 1 }], { duration: 400 })
    assert.equal(el.calls[1].duration, 400)
  } finally {
    globalThis.matchMedia = saved
  }
})
