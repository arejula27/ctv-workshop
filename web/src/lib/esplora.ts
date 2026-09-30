// Esplora REST API (electrs), served by Caddy under /esplora.
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

export async function utxos(address: string) {
  const list = (await get<{ txid: string; vout: number; value: number; status: TxStatus }[]>(
    `/address/${address}/utxo`,
  )) ?? []
  return list.map((u) => ({ ...u, address }))
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
