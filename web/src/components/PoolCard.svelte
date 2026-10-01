<script lang="ts">
  import { onMount } from 'svelte'
  import { pool as core, type PoolParams } from '../lib/core'
  import type { PoolTable } from '../lib/api'
  import { broadcastAndReport, friendly } from '../lib/act'
  import { chain, me } from '../lib/store.svelte'
  import { prune, treeStatus, type ContractStatus } from '../lib/treeStatus'
  import { poolState } from '../lib/contractPhase'
  import { sats } from '../lib/format'
  import TreeView from './TreeView.svelte'

  let { table }: { table: PoolTable } = $props()

  let params: PoolParams = $derived({ members: table.members, share_sat: table.share_sat, fee_sat: table.fee_sat })
  let view = $derived(core.view(params, table.outpoint ?? undefined))
  let statuses = $state(new Map<string, ContractStatus>())
  let st = $derived(poolState(view, statuses, table.members.length))
  let me_i = $derived(table.members.findIndex((m) => m.pubkey === me.wallet?.main_pubkey))
  let busy = $state(false)
  let error = $state('')
  let note = $state('')
  let showTree = $state(false)

  const refresh = () => treeStatus(view).then((s) => (statuses = s)).catch((e) => (error = e.message))
  onMount(() => {
    const t = setInterval(refresh, 4000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    refresh()
  })
  // A note about pushing someone out is stale once anyone actually leaves.
  $effect(() => {
    void st.exited.length
    note = ''
  })

  /** What a member got when leaving: the last one out pays no fee. */
  const payout = (i: number) =>
    st.phase === 'dissolved' && st.exited[st.exited.length - 1] === i ? table.share_sat : table.share_sat - table.fee_sat

  async function leave() {
    busy = true
    error = note = ''
    try {
      const tx = core.exit(params, st.exited, me_i, st.current!, me.id!.seed)
      await broadcastAndReport(tx, 'pool_exit', table.id)
      await refresh()
    } catch (e) {
      error = friendly(e)
    } finally {
      busy = false
    }
  }

  /** Try to push someone else out: it can't be done without their signature. */
  function pushOut(i: number) {
    error = ''
    try {
      core.exit(params, st.exited, i, st.current!, me.id!.seed)
      note = 'Esto no debería pasar.'
    } catch (e) {
      note = `❌ No puedes sacar a ${table.members[i].label}. ${friendly(e)}`
    }
  }
</script>

<article class="card {st.phase}">
  <header>
    <strong>🏊 Mesa «{table.name}»</strong>
    <span class="muted">{table.members.length} personas · {sats(table.share_sat)} cada una</span>
  </header>

  <div class="story">
    {#if st.phase === 'loading'}
      <p class="muted">Buscando el pool en la cadena…</p>
    {:else if st.phase === 'funding'}
      <p>Creando el UTXO compartido. Se confirma en el próximo bloque.</p>
    {:else if st.phase === 'active'}
      <p>Todo el dinero de dentro es <b>un solo UTXO</b>. Cualquiera puede salir cuando quiera, sin pedir permiso, y el resto sigue dentro.</p>
    {:else}
      <p>El pool se ha disuelto: cada persona tiene su parte.</p>
    {/if}
  </div>

  <div class="pool">
    <div class="utxo" class:empty={st.inside.length === 0}>
      <div class="utxo-label">
        {st.inside.length ? `UTXO compartido · ${sats(st.inside.length * table.share_sat)}` : 'Ya no queda nadie dentro'}
      </div>
      <div class="chips">
        {#each st.inside as i (i)}
          <div class="chip in" class:me={i === me_i}>
            <span>{table.members[i].label}{i === me_i ? ' (tú)' : ''}</span>
            {#if st.phase === 'active' && !st.pending && me_i >= 0 && st.inside.includes(me_i)}
              {#if i === me_i}
                <button class="hot small" onclick={leave} disabled={busy}>🚪 Salir</button>
              {:else}
                <button class="secondary small" onclick={() => pushOut(i)}>Sacar</button>
              {/if}
            {/if}
          </div>
        {/each}
      </div>
    </div>
    {#if st.exited.length}
      <div class="out">
        {#each st.exited as i, n (i)}
          <div class="chip gone" class:me={i === me_i}>
            {n + 1}º {table.members[i].label}{i === me_i ? ' (tú)' : ''} · {sats(payout(i))}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if st.pending && st.phase === 'active'}<p class="muted center">🚪 Alguien está saliendo… se confirma en el próximo bloque.</p>{/if}
  {#if note}<p class="note">{note}</p>{/if}
  {#if error}<p class="error">{error}</p>{/if}

  <footer>
    <button class="secondary" onclick={() => (showTree = !showTree)}>{showTree ? 'Ocultar' : 'Ver'} árbol de transacciones</button>
  </footer>
  {#if showTree}
    <p class="muted">Solo se dibuja lo que ha pasado y las salidas posibles ahora.</p>
    <div class="tree"><TreeView contract={prune(view, statuses)} {statuses} toneOf={() => undefined} /></div>
  {/if}
</article>

<style>
  .card { border: 2px solid var(--line); border-radius: 16px; padding: 1rem; display: flex; flex-direction: column; gap: 0.75rem; margin: 1rem 0; }
  header, .story, .center { text-align: center; }
  header { display: flex; flex-wrap: wrap; gap: 0.5rem; justify-content: center; align-items: baseline; }
  .story p { margin: 0.25rem 0; }
  .pool { display: grid; grid-template-columns: 2fr 1fr; gap: 0.75rem; align-items: start; }
  .utxo { border: 3px solid var(--cold); background: var(--cold-bg); border-radius: 999px; padding: 1.2rem 1.5rem; text-align: center; }
  .utxo.empty { border-style: dashed; background: transparent; }
  .utxo-label { font-weight: 700; margin-bottom: 0.6rem; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.5rem; justify-content: center; }
  .chip { display: inline-flex; gap: 0.4rem; align-items: center; border-radius: 999px; padding: 0.35rem 0.75rem; background: var(--bg); font-weight: 600; }
  .chip.me { outline: 2px solid var(--hot); }
  .out { display: flex; flex-direction: column; gap: 0.4rem; }
  .chip.gone { background: var(--hot-bg); justify-content: flex-start; }
  .small { padding: 0.25rem 0.6rem; font-size: 0.8rem; }
  .note { background: var(--thief-bg); border-radius: 10px; padding: 0.5rem 0.75rem; margin: 0; }
  footer { display: flex; gap: 0.5rem; }
  .tree { overflow-x: auto; }
  @media (max-width: 40rem) { .pool { grid-template-columns: 1fr; } }
</style>
