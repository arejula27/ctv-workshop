<script lang="ts">
  import { onMount } from 'svelte'
  import { vault as core } from '../lib/core'
  import type { VaultEntry } from '../lib/api'
  import { chain } from '../lib/store.svelte'
  import { treeStatus, type ContractStatus } from '../lib/treeStatus'
  import { blocksLeft as blocksLeftOf, unvaultOutpoint, vaultPhase } from '../lib/vaultPhase'
  import { sats } from '../lib/format'

  let { v, stolen }: { v: VaultEntry; stolen: boolean } = $props()

  let view = $derived(core.view(v.params, v.outpoint))
  let statuses = $state(new Map<string, ContractStatus>())
  let phase = $derived(vaultPhase(view, statuses))
  let left = $derived(blocksLeftOf(statuses.get(unvaultOutpoint(view)), v.params.delay, chain.height))

  const refresh = () => treeStatus(view).then((s) => (statuses = s)).catch(() => {})
  onMount(() => {
    const t = setInterval(refresh, 5000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    refresh()
  })

  let label = $derived.by(() => {
    switch (phase) {
      case 'loading':
      case 'funding':
        return '⏳ guardándose…'
      case 'locked':
        return '🔒 cerrado'
      case 'withdrawing':
        return left != null && left > 0 ? `⏱ retiro en curso · faltan ${left}` : '⏱ ¡tiempo cumplido!'
      case 'cold':
        return '🧊 en la caja fuerte'
      case 'panicked':
        return '🚨 salvado con el pánico'
      case 'hot':
        return stolen ? '💰 ¡robado!' : '🔥 retirado'
    }
  })
</script>

<div class="vault {phase}" class:stolen>
  <div class="owner">{v.owner}</div>
  <div class="amount">{sats(v.params.amount_sat)}</div>
  <div class="state">{label}</div>
</div>

<style>
  .vault {
    border-radius: 14px;
    padding: 0.8rem 1rem;
    background: var(--card);
    border: 3px solid var(--line);
  }
  .owner { font-weight: 700; font-size: 1.3rem; }
  .amount { color: var(--muted); font-variant-numeric: tabular-nums; }
  .state { font-weight: 600; margin-top: 0.3rem; }
  .withdrawing { border-color: var(--hot); background: var(--hot-bg); animation: pulse 1.2s ease-in-out infinite; }
  .cold, .panicked { border-color: var(--cold); background: var(--cold-bg); }
  .hot { border-color: var(--hot); }
  .stolen { border-color: var(--thief); background: var(--thief-bg); }
  @keyframes pulse { 50% { transform: scale(1.03); } }
  @media (prefers-reduced-motion: reduce) { .withdrawing { animation: none; } }
</style>
