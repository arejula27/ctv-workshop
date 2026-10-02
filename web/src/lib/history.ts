// A wallet's transaction history, from esplora.
const BASE = '/esplora'

type EsploraTx = {
  txid: string
  status: { confirmed: boolean; block_height?: number }
  vin: { prevout: { scriptpubkey_address?: string; value: number } | null }[]
  vout: { scriptpubkey_address?: string; value: number }[]
}

export type HistoryEntry = {
  txid: string
  /** Net change for the wallet, in sats (negative: sent). */
  delta: number
  confirmed: boolean
  height?: number
  /** The other side: who paid us, or where we sent it. */
  counterparty?: string
}

/** Newest first (esplora returns mempool transactions first). */
export async function history(addresses: string[]): Promise<HistoryEntry[]> {
  const mine = new Set(addresses)
  const lists = await Promise.all(
    addresses.map(async (a) => {
      const r = await fetch(`${BASE}/address/${a}/txs`)
      return r.ok ? ((await r.json()) as EsploraTx[]) : []
    }),
  )
  const seen = new Map<string, EsploraTx>()
  for (const tx of lists.flat()) seen.set(tx.txid, tx)
  return [...seen.values()]
    .map((tx) => {
      const spent = tx.vin.reduce((a, i) => a + (i.prevout && mine.has(i.prevout.scriptpubkey_address ?? '') ? i.prevout.value : 0), 0)
      const received = tx.vout.reduce((a, o) => a + (mine.has(o.scriptpubkey_address ?? '') ? o.value : 0), 0)
      const delta = received - spent
      const counterparty =
        delta < 0
          ? tx.vout.find((o) => !mine.has(o.scriptpubkey_address ?? ''))?.scriptpubkey_address
          : tx.vin.find((i) => i.prevout && !mine.has(i.prevout.scriptpubkey_address ?? ''))?.prevout?.scriptpubkey_address
      return { txid: tx.txid, delta, confirmed: tx.status.confirmed, height: tx.status.block_height, counterparty }
    })
    .sort((a, b) => (a.confirmed === b.confirmed ? (b.height ?? 0) - (a.height ?? 0) : a.confirmed ? 1 : -1))
}
