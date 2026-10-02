<script lang="ts">
  import { subscription as core, type SubscriptionParams } from '../lib/core'
  import { chain } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { subscriptionState } from '../lib/contractPhase'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'
  import { txUrl } from '../lib/explorer'

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
  href={txUrl(outpoint.split(':')[0])}
  title="{owner} → {receiver}"
  subtitle="{sats(params.payment_sat)} cada {params.every} bloques"
  tone={s.phase === 'cancelled' ? 'hot' : s.phase === 'done' ? 'cold' : undefined}
>
  <div class="mono dots">{dots}</div>
  {#if s.phase === 'loading' || s.phase === 'funding'}<span class="badge">Reservando…</span>
  {:else if s.phase === 'active'}<span class="badge {s.pending || (s.blocksLeft ?? 1) <= 0 ? 'cold' : ''}">{s.pending ? 'Pago en camino' : s.blocksLeft != null && s.blocksLeft > 0 ? `Próximo pago en ${s.blocksLeft}` : 'Pago listo'}</span>
  {:else if s.phase === 'cancelled'}<span class="badge hot">Cancelada</span>
  {:else}<span class="badge ok">Completada</span>
  {/if}
</Tile>

<style>
  .dots { margin-bottom: 0.4rem; color: var(--muted); }
</style>
