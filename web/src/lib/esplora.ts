// Esplora REST API (electrs), served by Caddy under /esplora.
import { txSummary } from './core'

const BASE = '/esplora'

export type TxStatus = { confirmed: boolean; block_height?: number }
export type Outspend = { spent: boolean; txid?: string; status?: TxStatus }

async function get<T>(path: string): Promise<T | null> {
  const r = await fetch(BASE + path)
  if (r.status === 404) return null
  if (!r.ok) throw new Error(`${path}: ${r.status} ${await r.text()}`)
  return r.json()
}

export const tipHeight = async () => (await get<number>('/blocks/tip/height')) ?? 0

type ApiUtxo = { txid: string; vout: number; value: number; status: TxStatus }

/** Coins of `address`, corrected with what this app has just broadcast: the
 *  indexer lags a few seconds behind, so without this two quick spends would
 *  pick the same coin ("already spent"), and change wouldn't be usable until
 *  it shows up. */
export async function utxos(address: string) {
  const list = (await get<ApiUtxo[]>(`/address/${address}/utxo`)) ?? []
  const seen = new Set(list.map((u) => `${u.txid}:${u.vout}`))
  // The indexer caught up with these, or should have by now (someone else may
  // have spent our change before it showed up): stop overriding it.
  const now = Date.now()
  for (const [op, c] of [...created]) if (seen.has(op) || now - c.at > TRUST_INDEX_MS) created.delete(op)
  const fresh = [...created.entries()]
    .filter(([op, c]) => c.address === address && !spent.has(op))
    .map(([op, c]) => ({ txid: op.split(':')[0], vout: Number(op.split(':')[1]), value: c.value, status: { confirmed: false } }))
  return [...list.filter((u) => !spent.has(`${u.txid}:${u.vout}`)), ...fresh].map((u) => ({ ...u, address }))
}

// What we broadcast recently: inputs it spent, outputs it created (by outpoint).
const spent = new Map<string, number>()
const created = new Map<string, { address: string; value: number; at: number }>()
const FORGET_MS = 10 * 60_000
/** The indexer takes a second or two to see a broadcast: after this, trust it. */
const TRUST_INDEX_MS = 10_000

function remember(hex: string) {
  const now = Date.now()
  for (const [op, at] of spent) if (now - at > FORGET_MS) spent.delete(op)
  const tx = txSummary(hex)
  for (const op of tx.inputs) {
    spent.set(op, now)
    created.delete(op)
  }
  for (const o of tx.outputs) if (o.address) created.set(`${tx.txid}:${o.vout}`, { address: o.address, value: o.value, at: now })
}

/** null when the node has never seen the transaction. */
export const txStatus = (txid: string) => get<TxStatus>(`/tx/${txid}/status`)

export async function outspend(outpoint: string): Promise<Outspend> {
  const [txid, vout] = outpoint.split(':')
  return (await get<Outspend>(`/tx/${txid}/outspend/${vout}`)) ?? { spent: false }
}

/** Broadcasts a raw transaction; throws with the node's reason on rejection. */
export async function broadcast(hex: string): Promise<string> {
  const r = await fetch(`${BASE}/tx`, { method: 'POST', body: hex })
  const text = await r.text()
  if (!r.ok) throw new Error(explain(text))
  remember(hex)
  return text
}

/** Node rejection reasons in words a non-technical audience understands. */
function explain(reason: string): string {
  if (reason.includes('non-BIP68-final')) return 'Todavía no: el temporizador no ha terminado.'
  if (reason.includes('missingorspent') || reason.includes('bad-txns-inputs-missingorspent'))
    return 'Demasiado tarde: esas monedas ya se han movido.'
  if (reason.includes('txn-mempool-conflict')) return 'Otra transacción ya está gastando esas monedas.'
  if (reason.includes('script-verify-flag-failed')) return 'Las reglas del contrato no lo permiten.'
  return reason
}
