/** Pure geometry for Ring.svelte (countdown arc). */

/** Fraction of time left, clamped to [0, 1]. A non-positive total counts as finished. */
export function ringFraction(total: number, remaining: number): number {
  if (!(total > 0) || !Number.isFinite(remaining)) return 0
  return Math.min(1, Math.max(0, remaining / total))
}

export function ringGeometry(size: number, stroke: number) {
  const r = Math.max(0, (size - stroke) / 2)
  return { r, c: 2 * Math.PI * r, center: size / 2 }
}

/** stroke-dashoffset that leaves an arc of length C * fraction visible. */
export function ringDashOffset(total: number, remaining: number, circumference: number): number {
  return circumference * (1 - ringFraction(total, remaining))
}
