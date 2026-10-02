<script lang="ts">
  // One vault as a row inside a person's card on the projector.
  import { vault as core } from '../lib/core'
  import type { VaultEntry } from '../lib/api'
  import { chain } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { blocksLeft as blocksLeftOf, unvaultOutpoint, vaultPhase } from '../lib/vaultPhase'
  import { sats } from '../lib/format'
  import { txUrl } from '../lib/explorer'

  let { v, stolen }: { v: VaultEntry; stolen: boolean } = $props()

  let view = $derived(core.view(v.params, v.outpoint))
  const st = useStatuses(() => view)
  let phase = $derived(vaultPhase(view, st.map))
  let left = $derived(blocksLeftOf(st.map.get(unvaultOutpoint(view)), v.params.delay, chain.height))
  // The vault's latest transaction: deposit, withdrawal start, or how it ended.
  let latest = $derived(
    st.map.get(unvaultOutpoint(view))?.spentBy ?? st.map.get(v.outpoint)?.spentBy ?? v.outpoint.split(':')[0],
  )
</script>

<div class="row" class:alarm={phase === 'withdrawing'}>
  <span class="what">Vault · <span class="mono">{sats(v.params.amount_sat)}</span></span>
  {#if phase === 'loading' || phase === 'funding'}<span class="badge">Guardándose…</span>
  {:else if phase === 'locked'}<span class="badge ok">Protegido</span>
  {:else if phase === 'withdrawing'}<span class="badge hot">{left != null && left > 0 ? `Retiro · faltan ${left}` : 'Retiro · tiempo cumplido'}</span>
  {:else if phase === 'cold' || phase === 'panicked'}<span class="badge cold">Salvado</span>
  {:else}<span class="badge">{stolen ? 'Robado' : 'Retirado'}</span>
  {/if}
  <a href={txUrl(latest)} target="_blank" rel="noopener" title="Ver en el explorador">↗</a>
</div>

<style>
  .row { display: grid; grid-template-columns: 1fr auto 1rem; align-items: center; gap: 0.5rem; padding: 0.45rem 0.5rem; border-top: 1px solid var(--line); font-size: 0.95rem; }
  /* a withdrawal in progress: visible from the back of the room */
  .alarm { background: var(--hot-bg); border-radius: 8px; animation: ring 1.6s ease-in-out infinite; }
  @keyframes ring { 50% { box-shadow: 0 0 0 3px var(--hot-bg); } }
  @media (prefers-reduced-motion: reduce) { .alarm { animation: none; } }
  a { text-decoration: none; color: var(--muted); font-weight: 600; text-align: center; }
  a:hover { color: var(--cold); }
</style>
