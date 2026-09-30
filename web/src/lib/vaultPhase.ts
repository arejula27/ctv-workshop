// Where a vault is in its story, from its tree view and on-chain statuses.
import type { ContractView } from './core'
import type { ContractStatus } from './treeStatus'

// Leaf indexes, see ctv_core::contracts::vault
export const VAULT_START = 0
export const UNVAULT_HOT = 0

export type Phase = 'loading' | 'funding' | 'locked' | 'withdrawing' | 'cold' | 'panicked' | 'hot'

export const unvaultOutpoint = (view: ContractView) =>
  view.leaves[VAULT_START].tx!.outputs[0].contract!.outpoint!

export function vaultPhase(view: ContractView, statuses: Map<string, ContractStatus>): Phase {
  const root = statuses.get(view.outpoint!)
  const unvault = statuses.get(unvaultOutpoint(view))
  if (!root || root.funding === 'unknown') return 'loading'
  if (root.funding === 'mempool') return 'funding'
  if (root.spentLeaf == null) return 'locked'
  if (root.spentLeaf !== VAULT_START) return 'cold'
  if (unvault?.spentLeaf == null) return 'withdrawing'
  return unvault.spentLeaf === UNVAULT_HOT ? 'hot' : 'panicked'
}

/** Blocks until the hot key may withdraw (BIP68: the next block must reach height + delay). */
export function blocksLeft(unvault: ContractStatus | undefined, delay: number, tip: number) {
  return unvault?.height != null ? unvault.height + delay - (tip + 1) : null
}
