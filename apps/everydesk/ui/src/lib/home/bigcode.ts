// Pure helpers behind BigCode.svelte (tested in tests/BigCode.test.ts).

export const CODE_MIN_PX = 32
/** The floor of the cap: at 1440 wide the code reads as a plate (~72% width), not edge to edge. */
export const CODE_MAX_PX = 128
/** The cap grows with the window (7.8125vw) so the plate stays a plate on big screens… */
export const CODE_MAX_VW = 0.078125
/** …up to 200px (reached around 2560 wide). Mirrors --fs-display in typography.css. */
export const CODE_CAP_PX = 200
/** Rendered advance of one Geist Mono glyph with the -0.03em tracking. */
export const CODE_ADVANCE_EM = 0.57
/** Horizontal page gutters (2 × 32px) the code must stay inside. */
export const CODE_GUTTERS_PX = 64
/** Geist Mono advance (0.6em) plus a little slack for the -0.03em tracking and rounding. */
export const CODE_GLYPH_EM = 0.62
/** The code never takes more than this share of the window height. */
export const CODE_HEIGHT_SHARE = 0.2

/** The cap for a window this wide: CODE_MAX_PX, growing with the width up to CODE_CAP_PX. */
export function codeCap(viewportWidth: number): number {
  return Math.min(CODE_CAP_PX, Math.max(CODE_MAX_PX, viewportWidth * CODE_MAX_VW))
}

/** Font size in px for the machine code: as wide as the window allows, capped by height and
 *  by codeCap(width), never below CODE_MIN_PX. Mirrors codeFontCss() below. */
export function codeFontSize(viewportWidth: number, viewportHeight = 900, chars = 13): number {
  const byWidth = (viewportWidth - CODE_GUTTERS_PX) / (Math.max(chars, 1) * CODE_GLYPH_EM)
  const byHeight = viewportHeight * CODE_HEIGHT_SHARE
  return Math.max(CODE_MIN_PX, Math.min(byWidth, byHeight, codeCap(viewportWidth)))
}

/** The same size as a CSS expression, so the browser resizes it live. */
export function codeFontCss(chars = 13): string {
  const n = Math.max(chars || 13, 1) * CODE_GLYPH_EM
  return `max(${CODE_MIN_PX}px, min((100vw - ${CODE_GUTTERS_PX}px) / ${n}, ${CODE_HEIGHT_SHARE * 100}vh, clamp(${CODE_MAX_PX}px, ${CODE_MAX_VW * 100}vw, ${CODE_CAP_PX}px)))`
}

/** Width of the "Para onde você quer ir?" block: the code's own width, never under 640px,
 *  never over the room there is. */
export function goWidthCss(chars = 13): string {
  return `min(100%, max(640px, calc(${Math.max(chars || 13, 1) * CODE_ADVANCE_EM} * ${codeFontCss(chars)})))`
}

/** The same rule in px (tests). */
export function goWidth(viewportWidth: number, viewportHeight = 900, chars = 13, room = viewportWidth - CODE_GUTTERS_PX): number {
  return Math.min(room, Math.max(640, chars * CODE_ADVANCE_EM * codeFontSize(viewportWidth, viewportHeight, chars)))
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
