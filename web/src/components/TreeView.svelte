<script lang="ts">
  import type { ContractView } from '../lib/core'
  import type { ContractStatus } from '../lib/treeStatus'
  import { chain } from '../lib/store.svelte'
  import { sats } from '../lib/format'
  import TreeView from './TreeView.svelte'

  type Tone = 'hot' | 'cold' | undefined

  let {
    contract,
    statuses,
    toneOf = () => undefined,
  }: {
    contract: ContractView
    statuses: Map<string, ContractStatus>
    toneOf?: (label: string) => Tone
  } = $props()

  let status = $derived(contract.outpoint ? statuses.get(contract.outpoint) : undefined)

  function leafTone(i: number): Tone {
    const leaf = contract.leaves[i]
    if (leaf.kind === 'key') return toneOf(leaf.label) ?? 'hot'
    const tones = leaf.tx?.outputs.map((o) => toneOf(o.label)) ?? []
    return tones.length && tones.every((t) => t === tones[0]) ? tones[0] : toneOf(leaf.label)
  }

  function leafState(i: number): 'taken' | 'skipped' | 'open' {
    if (status?.spentLeaf == null) return 'open'
    return status.spentLeaf === i ? 'taken' : 'skipped'
  }

  function lockText(csv: number | null): string | null {
    if (csv == null) return null
    if (status?.height == null) return `espera ${csv} bloques`
    const left = status.height + csv - (chain.height + 1)
    return left > 0 ? `faltan ${left} bloques` : 'temporizador cumplido'
  }
</script>

<div class="contract" class:funded={status?.funding === 'confirmed'}>
  <div class="box">
    <div class="title">🔒 {contract.label}</div>
    <div class="amount">{sats(contract.value_sat)}</div>
    {#if status}
      <div class="badge">
        {status.funding === 'confirmed'
          ? status.spentLeaf != null
            ? 'gastado'
            : 'bloqueado en la cadena'
          : status.funding === 'mempool'
            ? 'esperando bloque…'
            : 'sin publicar'}
      </div>
    {/if}
  </div>

  <div class="leaves">
    {#each contract.leaves as leaf, i}
      <div class="leaf {leafTone(i) ?? ''} {leafState(i)}">
        <div class="edge">
          <span class="leaf-label">{leaf.label}</span>
          <span class="chips">
            {#if leaf.csv != null}<span class="chip">⏱ {lockText(leaf.csv)}</span>{/if}
            {#if leaf.signer}<span class="chip">✍ firma</span>{:else if leaf.kind === 'ctv'}<span class="chip">cualquiera puede</span>{/if}
          </span>
        </div>
        {#if leaf.tx}
          <div class="tx">
            {#each leaf.tx.outputs as out}
              {#if out.contract}
                <TreeView contract={out.contract} {statuses} {toneOf} />
              {:else}
                <div class="pay {toneOf(out.label) ?? ''}">
                  {toneOf(out.label) === 'cold' ? '🧊' : toneOf(out.label) === 'hot' ? '🔥' : '→'}
                  {out.label} · {sats(out.value_sat)}
                </div>
              {/if}
            {/each}
          </div>
        {:else}
          <div class="pay {leafTone(i) ?? ''}">🔥 A donde decida quien tenga la llave</div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .contract { display: flex; flex-direction: column; gap: 0.5rem; }
  .box {
    border: 2px solid var(--line);
    border-radius: 12px;
    padding: 0.6rem 0.8rem;
    background: var(--card);
  }
  .funded > .box { border-color: var(--fg); }
  .title { font-weight: 600; }
  .amount { font-variant-numeric: tabular-nums; }
  .badge { font-size: 0.8rem; color: var(--muted); }
  .leaves { display: flex; flex-direction: column; gap: 0.75rem; padding-left: 1rem; border-left: 2px solid var(--line); }
  .leaf { display: flex; flex-direction: column; gap: 0.4rem; padding-left: 0.75rem; border-left: 4px solid var(--line); }
  .leaf.hot { border-left-color: var(--hot); }
  .leaf.cold { border-left-color: var(--cold); }
  .leaf.skipped { opacity: 0.35; }
  /* direct child only: nested leaves have their own state */
  .leaf.taken > .edge > .leaf-label::after { content: ' ✓'; }
  .edge { display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: baseline; }
  .leaf-label { font-weight: 600; }
  .chip { font-size: 0.75rem; background: var(--card); border-radius: 999px; padding: 0.1rem 0.5rem; color: var(--muted); }
  .tx { display: flex; flex-direction: column; gap: 0.5rem; }
  .pay { border-radius: 10px; padding: 0.4rem 0.7rem; background: var(--card); font-size: 0.9rem; }
  .pay.hot { background: var(--hot-bg); }
  .pay.cold { background: var(--cold-bg); }
</style>
