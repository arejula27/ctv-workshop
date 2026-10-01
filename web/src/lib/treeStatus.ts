// On-chain status of a contract tree, generic for every demo.
// For each funded contract: is it confirmed, and which leaf spent it (if any).
import type { ContractView } from './core'
import * as esplora from './esplora'

export type ContractStatus = {
  funding: 'unknown' | 'mempool' | 'confirmed'
  height?: number
  /** Index of the leaf that spent it, or undefined while unspent. */
  spentLeaf?: number
  spentBy?: string
  spentConfirmed?: boolean
}

/** Keyed by outpoint. Only walks down the branch that actually happened. */
export async function treeStatus(root: ContractView): Promise<Map<string, ContractStatus>> {
  const out = new Map<string, ContractStatus>()
  async function walk(c: ContractView) {
    if (!c.outpoint) return
    const [txStatus, spend] = await Promise.all([
      esplora.txStatus(c.outpoint.split(':')[0]),
      esplora.outspend(c.outpoint),
    ])
    const status: ContractStatus = {
      funding: !txStatus ? 'unknown' : txStatus.confirmed ? 'confirmed' : 'mempool',
      height: txStatus?.block_height,
    }
    if (spend.spent && spend.txid) {
      // A CTV leaf's txid is known in advance; anything else is a key leaf.
      let leaf = c.leaves.findIndex((l) => l.tx?.txid === spend.txid)
      if (leaf < 0) leaf = c.leaves.findIndex((l) => l.kind === 'key')
      status.spentLeaf = leaf
      status.spentBy = spend.txid
      status.spentConfirmed = spend.status?.confirmed ?? false
    }
    out.set(c.outpoint, status)
    const taken = status.spentLeaf != null ? c.leaves[status.spentLeaf]?.tx : null
    if (taken) await Promise.all(taken.outputs.map((o) => (o.contract ? walk(o.contract) : null)))
  }
  await walk(root)
  return out
}

/** First block at which a leaf with `csv` becomes spendable (BIP68). */
export const unlockHeight = (s: ContractStatus | undefined, csv: number | null) =>
  s?.height != null && csv != null ? s.height + csv : undefined

/** A copy that only expands what happened plus the next choices, for trees with
 *  many paths (a pool of 6 has 720). Leaves not taken keep their label and
 *  amounts, but the contracts behind them are folded away. */
export function prune(c: ContractView, statuses: Map<string, ContractStatus>): ContractView {
  const s = c.outpoint ? statuses.get(c.outpoint) : undefined
  return {
    ...c,
    leaves: c.leaves.map((leaf, i) => {
      if (!leaf.tx) return leaf
      const taken = s?.spentLeaf === i
      return {
        ...leaf,
        tx: {
          ...leaf.tx,
          outputs: leaf.tx.outputs.map((o) => ({
            ...o,
            contract: o.contract && taken ? prune(o.contract, statuses) : null,
          })),
        },
      }
    }),
  }
}
