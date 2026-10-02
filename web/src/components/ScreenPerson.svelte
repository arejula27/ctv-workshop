<script lang="ts">
  // A participant in the vault demo: their hot wallet and every vault, in one card.
  import { onMount } from 'svelte'
  import type { HotWallet, VaultEntry } from '../lib/api'
  import * as esplora from '../lib/esplora'
  import { chain } from '../lib/store.svelte'
  import { sats } from '../lib/format'
  import Tile from './Tile.svelte'
  import ScreenVault from './ScreenVault.svelte'

  let { owner, wallets, vaults, stolen }: { owner: string; wallets: HotWallet[]; vaults: VaultEntry[]; stolen: Set<string> } =
    $props()

  let hot = $state<number | null>(null)
  async function refresh() {
    const lists = await Promise.all(wallets.map((w) => esplora.utxos(w.address))).catch(() => null)
    if (lists) hot = lists.flat().reduce((a, u) => a + u.value, 0)
  }
  onMount(() => {
    const t = setInterval(refresh, 5000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    void wallets.length
    refresh()
  })
</script>

<Tile title={owner} subtitle={wallets.length ? `Cartera caliente · ${hot == null ? '…' : sats(hot)}` : 'Sin cartera caliente'} tone="hot">
  {#each vaults as v (v.outpoint)}
    <ScreenVault {v} stolen={stolen.has(v.outpoint)} />
  {:else}
    <div class="muted none">Todavía sin vaults</div>
  {/each}
</Tile>

<style>
  .none { font-size: 0.9rem; }
</style>
