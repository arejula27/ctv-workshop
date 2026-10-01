// Where an inheritance or a subscription is in its story.
import type { ContractView } from './core'
import type { ContractStatus } from './treeStatus'

// Leaf indexes, see ctv_core::contracts::{inheritance, subscription}
const OWNER = 0
const SPLIT = 1
const PAY = 0
const CANCEL = 1

export type InheritancePhase = 'loading' | 'funding' | 'waiting' | 'claimable' | 'distributed' | 'moved'

export function inheritancePhase(
  view: ContractView,
  statuses: Map<string, ContractStatus>,
  delay: number,
  tip: number,
): { phase: InheritancePhase; blocksLeft: number | null } {
  const s = statuses.get(view.outpoint!)
  if (!s || s.funding === 'unknown') return { phase: 'loading', blocksLeft: null }
  if (s.funding === 'mempool') return { phase: 'funding', blocksLeft: null }
  if (s.spentLeaf === SPLIT) return { phase: 'distributed', blocksLeft: null }
  if (s.spentLeaf === OWNER) return { phase: 'moved', blocksLeft: null }
  const left = s.height! + delay - (tip + 1)
  return { phase: left > 0 ? 'waiting' : 'claimable', blocksLeft: left }
}

export type SubscriptionState = {
  phase: 'loading' | 'funding' | 'active' | 'cancelled' | 'done'
  /** Current step (1-based): the next payment to make. */
  step: number
  paid: number
  /** Blocks until payment `step` can be made (≤ 0: due now). */
  blocksLeft: number | null
  /** The current step is still waiting for its first confirmation. */
  pending: boolean
}

export function subscriptionState(
  view: ContractView,
  statuses: Map<string, ContractStatus>,
  count: number,
  every: number,
  tip: number,
): SubscriptionState {
  let c: ContractView | null = view
  for (let k = 1; c; k++) {
    const s = statuses.get(c.outpoint!)
    const base = { step: k, paid: k - 1, blocksLeft: null, pending: false }
    if (!s || s.funding === 'unknown') return { ...base, phase: k === 1 ? 'loading' : 'active', pending: k > 1 }
    if (s.funding === 'mempool') return { ...base, phase: k === 1 ? 'funding' : 'active', pending: true }
    if (s.spentLeaf == null) return { ...base, phase: 'active', blocksLeft: s.height! + every - (tip + 1) }
    if (s.spentLeaf === CANCEL) return { ...base, phase: 'cancelled' }
    if (k === count) return { ...base, phase: 'done', paid: count }
    c = c.leaves[PAY].tx?.outputs[1]?.contract ?? null
  }
  return { phase: 'loading', step: 1, paid: 0, blocksLeft: null, pending: false }
}
