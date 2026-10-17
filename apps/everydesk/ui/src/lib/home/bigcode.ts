// Pure helpers behind BigCode.svelte (tested in tests/BigCode.test.ts).

export const CODE_MIN_PX = 32
/** About 128px: at 1440 wide the code reads as a plate (~72% width), not edge to edge. */
export const CODE_MAX_PX = 128
/** Horizontal page gutters (2 × 32px) the code must stay inside. */
export const CODE_GUTTERS_PX = 64
/** Geist Mono advance (0.6em) plus a little slack for the -0.03em tracking and rounding. */
export const CODE_GLYPH_EM = 0.62
/** The code never takes more than this share of the window height. */
export const CODE_HEIGHT_SHARE = 0.2

/** Font size in px for the machine code: as wide as the window allows, capped by height,
 *  clamped to [CODE_MIN_PX, CODE_MAX_PX]. Mirrors the CSS clamp() in BigCode.svelte. */
export function codeFontSize(viewportWidth: number, viewportHeight = 900, chars = 13): number {
  const byWidth = (viewportWidth - CODE_GUTTERS_PX) / (Math.max(chars, 1) * CODE_GLYPH_EM)
  const byHeight = viewportHeight * CODE_HEIGHT_SHARE
  return Math.min(CODE_MAX_PX, Math.max(CODE_MIN_PX, Math.min(byWidth, byHeight)))
}

/** "azul-tigre-42" → ["azul", "tigre", "42"]; numeric ids stay whole. */
export function codeParts(code: string): string[] {
  return code.split('-').filter((p) => p.length > 0)
}

/** `get_server_status` text → the quiet line under the code. */
export function serverStatus(raw: string): { label: string; online: boolean } {
  switch (raw) {
    case 'registrado':
      return { label: 'disponível', online: true }
    case 'aguardando autorização do servidor':
      return { label: 'aguardando autorização', online: false }
    case 'conectando':
      return { label: 'conectando…', online: false }
    default:
      return { label: 'sem conexão', online: false }
  }
}
