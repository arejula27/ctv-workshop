<script lang="ts">
  import { inheritance as core, type InheritanceParams } from '../lib/core'
  import { chain } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { inheritancePhase } from '../lib/contractPhase'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'

  let { owner, params, outpoint }: { owner: string; params: InheritanceParams; outpoint: string } = $props()

  let view = $derived(core.view(params, outpoint))
  const st = useStatuses(() => view)
  let ph = $derived(inheritancePhase(view, st.map, params.delay, chain.height))
  let heirs = $derived(params.heirs.map((h) => h.label).join(', '))
</script>

<Tile
  title="📜 {owner}"
  subtitle="{sats(params.amount_sat)} → {heirs}"
  tone={ph.phase === 'claimable' || ph.phase === 'distributed' ? 'cold' : ph.phase === 'waiting' ? 'hot' : undefined}
  pulse={ph.phase === 'claimable'}
>
  {#if ph.phase === 'loading' || ph.phase === 'funding'}⏳ guardándose…
  {:else if ph.phase === 'waiting'}💓 se reparte en {ph.blocksLeft} bloques si no da señales
  {:else if ph.phase === 'claimable'}⏰ ¡plazo cumplido!
  {:else if ph.phase === 'distributed'}📜 repartida
  {:else}↩ recuperada
  {/if}
</Tile>
