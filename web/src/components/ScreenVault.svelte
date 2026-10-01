<script lang="ts">
  import { vault as core } from '../lib/core'
  import type { VaultEntry } from '../lib/api'
  import { chain } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { blocksLeft as blocksLeftOf, unvaultOutpoint, vaultPhase } from '../lib/vaultPhase'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'

  let { v, stolen }: { v: VaultEntry; stolen: boolean } = $props()

  let view = $derived(core.view(v.params, v.outpoint))
  const st = useStatuses(() => view)
  let phase = $derived(vaultPhase(view, st.map))
  let left = $derived(blocksLeftOf(st.map.get(unvaultOutpoint(view)), v.params.delay, chain.height))

  let tone = $derived(
    phase === 'hot' && stolen ? 'thief' : phase === 'withdrawing' || phase === 'hot' ? 'hot' : phase === 'cold' || phase === 'panicked' ? 'cold' : undefined,
  ) as 'hot' | 'cold' | 'thief' | undefined
</script>

<Tile title={v.owner} subtitle={sats(v.params.amount_sat)} {tone} pulse={phase === 'withdrawing'}>
  {#if phase === 'loading' || phase === 'funding'}⏳ guardándose…
  {:else if phase === 'locked'}🔒 cerrado
  {:else if phase === 'withdrawing'}{left != null && left > 0 ? `⏱ retiro en curso · faltan ${left}` : '⏱ ¡tiempo cumplido!'}
  {:else if phase === 'cold'}🧊 en la caja fuerte
  {:else if phase === 'panicked'}🚨 salvado con el pánico
  {:else}{stolen ? '💰 ¡robado!' : '🔥 retirado'}
  {/if}
</Tile>
