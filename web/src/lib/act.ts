// Broadcast a transaction and tell the room about it (the feed never blocks the action).
import { reportActivity, type ActivityKind } from './api'
import type { SignedTx } from './core'
import * as esplora from './esplora'
import { me, refreshBalances } from './store.svelte'

export async function broadcastAndReport(tx: SignedTx, kind: ActivityKind, ref: string) {
  await esplora.broadcast(tx.hex)
  if (me.id) reportActivity(me.id.name, kind, ref, tx.txid).catch(() => {})
  refreshBalances().catch(() => {})
}

/** Core/wallet errors in words for a non-technical audience. */
export function friendly(e: unknown): string {
  const msg = e instanceof Error ? e.message : String(e)
  if (msg.includes('not enough funds'))
    return 'No tienes saldo suficiente. Si acabas de pedir sats, espera unos segundos a que lleguen.'
  if (msg.includes('every heir must get')) return 'La cantidad es demasiado pequeña para repartirla entre tanta gente.'
  if (msg.includes('minimum is')) return 'La cantidad es demasiado pequeña.'
  return msg
}
