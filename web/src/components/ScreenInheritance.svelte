<script lang="ts">
  import { inheritance as core, type InheritanceParams } from '../lib/core'
  import { chain } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { inheritancePhase } from '../lib/contractPhase'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'
  import { txUrl } from '../lib/explorer'

  let { owner, params, outpoint }: { owner: string; params: InheritanceParams; outpoint: string } = $props()

  let view = $derived(core.view(params, outpoint))
  const st = useStatuses(() => view)
  let ph = $derived(inheritancePhase(view, st.map, params.delay, chain.height))
  let heirs = $derived(params.heirs.map((h) => h.label).join(', '))
</script>

<Tile
  href={txUrl(st.map.get(outpoint)?.spentBy ?? outpoint.split(':')[0])}
  title={owner}
  subtitle="{sats(params.amount_sat)} → {heirs}"
  tone={ph.phase === 'claimable' || ph.phase === 'distributed' ? 'cold' : ph.phase === 'waiting' ? 'hot' : undefined}
  pulse={ph.phase === 'claimable'}
>
  {#if ph.phase === 'loading' || ph.phase === 'funding'}<span class="badge">Guardándose…</span>
  {:else if ph.phase === 'waiting'}<span class="badge hot">Se reparte en {ph.blocksLeft} bloques</span>
  {:else if ph.phase === 'claimable'}<span class="badge cold">Plazo cumplido</span>
  {:else if ph.phase === 'distributed'}<span class="badge cold">Repartida</span>
  {:else}<span class="badge">Recuperada</span>
  {/if}
</Tile>
