<script lang="ts">
  import type { PoolTable } from '../lib/api'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'
  import { txUrl } from '../lib/explorer'

  let { table }: { table: PoolTable } = $props()
  let total = $derived(table.members.reduce((a, m) => a + m.balance_sat, 0))
  let updates = $derived(table.history.filter((s) => s.kind !== 'funded').length)
</script>

<Tile
  href={(table.outpoint ?? table.history[table.history.length - 1]?.txid) ? txUrl((table.outpoint ?? table.history[table.history.length - 1].txid).split(':')[0]) : undefined}
  title={table.name}
  subtitle={table.state === 'funded' ? `UTXO compartido · ${sats(total)} · ${updates} movimientos` : `${updates} movimientos`}
  tone={table.pending ? 'hot' : table.state === 'funded' ? 'cold' : undefined}
  pulse={!!table.pending}
>
  {#if table.state === 'funded'}
    {#each table.members as m (m.pubkey)}<div class="m"><span>{m.label}</span><span class="mono">{sats(m.balance_sat)}</span></div>{/each}
    {#if table.pending}<span class="badge hot sig">Firmando · {Object.keys(table.pending.sigs).length}/{table.members.length}</span>{/if}
  {:else}<span class="badge">Disuelto</span>
  {/if}
</Tile>

<style>
  .m { display: flex; justify-content: space-between; font-size: 0.92rem; padding: 0.2rem 0; border-top: 1px solid var(--line); }
  .sig { margin-top: 0.5rem; }
</style>
