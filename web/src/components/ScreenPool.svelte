<script lang="ts">
  import { pool as core } from '../lib/core'
  import type { PoolTable } from '../lib/api'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { poolState } from '../lib/contractPhase'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'

  let { table }: { table: PoolTable } = $props()

  let params = $derived({ members: table.members, share_sat: table.share_sat, fee_sat: table.fee_sat })
  let view = $derived(core.view(params, table.outpoint ?? undefined))
  const st = useStatuses(() => view)
  let s = $derived(poolState(view, st.map, table.members.length))
  const names = (ids: number[]) => ids.map((i) => table.members[i].label).join(', ')
</script>

<Tile
  title="🏊 {table.name}"
  subtitle={s.phase === 'active' ? `UTXO compartido · ${sats(s.inside.length * table.share_sat)}` : `${table.members.length} personas`}
  tone={s.phase === 'active' ? 'cold' : s.phase === 'dissolved' ? 'hot' : undefined}
>
  {#if s.phase === 'loading' || s.phase === 'funding'}⏳ creando el UTXO compartido…
  {:else if s.phase === 'active'}
    <div>dentro: {names(s.inside)}</div>
    {#if s.exited.length}<div class="out">🚪 salieron: {names(s.exited)}</div>{/if}
  {:else}🎉 disuelto: cada cual con su parte
  {/if}
</Tile>

<style>
  .out { color: var(--muted); font-weight: 500; }
</style>
