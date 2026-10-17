import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { contrast } from '../src/design/contrast.ts'

const css = readFileSync(new URL('../src/design/tokens.css', import.meta.url), 'utf8').replace(
  /\/\*[\s\S]*?\*\//g,
  '',
)

/** Custom properties declared in the first top-level block whose selector matches. */
function block(selectorPart: string): Record<string, string> {
  for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = m[1].split(',').map((s) => s.trim())
    if (!selectors.includes(selectorPart)) continue
    const vars: Record<string, string> = {}
    for (const d of m[2].matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) vars[d[1]] = d[2].trim()
    return vars
  }
  throw new Error(`no block for ${selectorPart}`)
}

const themes = { dark: block("[data-theme='dark']"), light: block("[data-theme='light']") }

test('contrast() matches WCAG reference values', () => {
  assert.equal(contrast('#000', '#fff').toFixed(2), '21.00')
  assert.equal(contrast('#fff', '#fff').toFixed(2), '1.00')
  assert.equal(contrast('#767676', '#ffffff').toFixed(2), '4.54')
})

test(':root carries the dark theme (dark is the default)', () => {
  assert.equal(block(':root')['--bg'], themes.dark['--bg'])
})

for (const [name, t] of Object.entries(themes)) {
  const ratio = (a: string, b: string) => contrast(t[a], t[b])

  test(`${name}: --text on --bg >= 7:1 (AAA body text)`, () => {
    assert.ok(ratio('--text', '--bg') >= 7, `${ratio('--text', '--bg')}`)
    assert.ok(ratio('--text', '--surface-2') >= 7)
  })

  test(`${name}: --text-muted >= 4.5:1 on every surface (no small grey text)`, () => {
    for (const s of ['--bg', '--surface', '--surface-2']) {
      assert.ok(ratio('--text-muted', s) >= 4.5, `${s}: ${ratio('--text-muted', s)}`)
    }
  })

  test(`${name}: --accent on --bg >= 3:1, label on accent >= 4.5:1`, () => {
    assert.ok(ratio('--accent', '--bg') >= 3, `${ratio('--accent', '--bg')}`)
    assert.ok(ratio('--accent', '--surface') >= 3)
    assert.ok(ratio('--on-accent', '--accent') >= 4.5, `${ratio('--on-accent', '--accent')}`)
  })

  test(`${name}: focus ring >= 3:1 and danger text >= 4.5:1`, () => {
    for (const s of ['--bg', '--surface', '--surface-2']) {
      assert.ok(ratio('--ring', s) >= 3, `ring on ${s}`)
      assert.ok(ratio('--danger', s) >= 4.5, `danger on ${s}`)
    }
  })
}
