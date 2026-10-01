export const sats = (n: number) => `${n.toLocaleString('es-ES')} sats`

export const short = (s: string, n = 8) => (s.length > 2 * n + 1 ? `${s.slice(0, n)}…${s.slice(-n)}` : s)

/** "~2 min" for a number of blocks at the current block interval. */
export function blocksToTime(blocks: number, intervalS: number | undefined) {
  if (!intervalS || blocks <= 0) return ''
  const s = blocks * intervalS
  return s < 90 ? `~${s} s` : `~${Math.round(s / 60)} min`
}

const dateFmt = new Intl.DateTimeFormat('es-ES', {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
})

/** "3 oct 2026, 14:30" */
export const date = (d: Date | null) => (d ? dateFmt.format(d) : '—')

/** Length of `blocks` on the simulated calendar: "1 h 40 min", "2 días 3 h". */
export function simDuration(blocks: number, blockS = 600) {
  const min = Math.round((blocks * blockS) / 60)
  const days = Math.floor(min / 1440)
  const hours = Math.floor((min % 1440) / 60)
  const mins = min % 60
  const parts = []
  if (days) parts.push(`${days} ${days === 1 ? 'día' : 'días'}`)
  if (hours) parts.push(`${hours} h`)
  if (mins && !days) parts.push(`${mins} min`)
  return parts.join(' ') || '0 min'
}
