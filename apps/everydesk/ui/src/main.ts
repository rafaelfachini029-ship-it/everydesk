import { mount } from 'svelte'
import './design/tokens.css'
import './design/typography.css'
import './app.css'
import App from './App.svelte'
import { applyTheme, storedTheme } from './lib/home/theme.ts'

applyTheme(storedTheme())

// Browser preview without Tauri (screenshots): fake backend, dev builds only.
if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) await import('./dev/mock.ts')

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
