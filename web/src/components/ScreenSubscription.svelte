<script lang="ts">
  import { subscription as core, type SubscriptionParams } from '../lib/core'
  import { chain } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { subscriptionState } from '../lib/contractPhase'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'

  let { owner, receiver, params, outpoint }: { owner: string; receiver: string; params: SubscriptionParams; outpoint: string } =
    $props()

  let view = $derived(core.view(params, outpoint))
  const st = useStatuses(() => view)
  let s = $derived(subscriptionState(view, st.map, params.count, params.every, chain.height))
  let dots = $derived(
    Array.from({ length: params.count }, (_, i) =>
      i < s.paid ? '✓' : s.phase === 'cancelled' ? '✗' : s.phase === 'active' && i + 1 === s.step ? '⏱' : '·',
    ).join(' '),
  )
</script>

<Tile
  title="📆 {owner} → {receiver}"
  subtitle="{sats(params.payment_sat)} cada {params.every} bloques"
  tone={s.phase === 'cancelled' ? 'hot' : s.phase === 'done' ? 'cold' : undefined}
>
  <div>{dots}</div>
  {#if s.phase === 'loading' || s.phase === 'funding'}⏳ reservando…
  {:else if s.phase === 'active'}{s.pending ? '💸 pago en camino' : s.blocksLeft != null && s.blocksLeft > 0 ? `próximo pago en ${s.blocksLeft}` : '⏰ pago listo para cobrar'}
  {:else if s.phase === 'cancelled'}✋ cancelada
  {:else}✅ completada
  {/if}
</Tile>
