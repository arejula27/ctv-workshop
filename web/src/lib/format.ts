export const sats = (n: number) => `${n.toLocaleString('es-ES')} sats`

export const short = (s: string, n = 8) => (s.length > 2 * n + 1 ? `${s.slice(0, n)}…${s.slice(-n)}` : s)

/** "~2 min" for a number of blocks at the current block interval. */
export function blocksToTime(blocks: number, intervalS: number | undefined) {
  if (!intervalS || blocks <= 0) return ''
  const s = blocks * intervalS
  return s < 90 ? `~${s} s` : `~${Math.round(s / 60)} min`
}
