// Light/dark choice, kept per machine. Dark is the default (tokens.css :root).
export type Theme = 'dark' | 'light'
const KEY = 'everydesk.theme'

export function storedTheme(): Theme {
  try {
    return localStorage.getItem(KEY) === 'light' ? 'light' : 'dark'
  } catch {
    return 'dark'
  }
}

export function applyTheme(t: Theme, persist = false) {
  document.documentElement.dataset.theme = t
  if (!persist) return
  try {
    localStorage.setItem(KEY, t)
  } catch {
    /* private storage: the choice lasts until the window closes */
  }
}
