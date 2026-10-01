<script lang="ts">
  import { onMount } from 'svelte'
  import { inheritance as core, type InheritanceParams } from '../lib/core'
  import { broadcastAndReport } from '../lib/act'
  import { registerContract } from '../lib/api'
  import { chain, me, removeContract, saveContract, simDate } from '../lib/store.svelte'
  import { treeStatus, type ContractStatus } from '../lib/treeStatus'
  import { inheritancePhase } from '../lib/contractPhase'
  import { date, sats, simDuration } from '../lib/format'
  import TreeView from './TreeView.svelte'

  let {
    id,
    params,
    outpoint,
    owner,
    role,
  }: { id: string; params: InheritanceParams; outpoint: string; owner: string; role: 'owner' | 'heir' } =
    $props()

  let view = $derived(core.view(params, outpoint))
  let statuses = $state(new Map<string, ContractStatus>())
  let ph = $derived(inheritancePhase(view, statuses, params.delay, chain.height))
  let split = $derived(view.leaves[1].tx!.outputs)
  let mine = $derived(me.wallet ? params.heirs.findIndex((h) => h.address === me.wallet!.main_address) : -1)
  let busy = $state(false)
  let error = $state('')
  let showTree = $state(false)

  const refresh = () => treeStatus(view).then((s) => (statuses = s)).catch((e) => (error = e.message))
  onMount(() => {
    const t = setInterval(refresh, 4000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    void outpoint
    refresh()
  })

  async function run(fn: () => Promise<void>) {
    busy = true
    error = ''
    try {
      await fn()
      await refresh()
    } catch (e) {
      error = (e as Error).message
    } finally {
      busy = false
    }
  }

  const keepAlive = () =>
    run(async () => {
      const tx = core.keepAlive(params, outpoint, me.id!.seed)
      await broadcastAndReport(tx, 'inheritance_keep_alive', id)
      const next = { id, kind: 'inheritance' as const, params: tx.next_params, outpoint: `${tx.txid}:0`, createdAt: Date.now() }
      saveContract(next)
      await registerContract(id, 'inheritance', me.id!.name, next.params, next.outpoint)
    })
  const withdraw = () =>
    run(() => broadcastAndReport(core.withdraw(params, outpoint, me.id!.seed, me.wallet!.main_address), 'inheritance_withdraw', id))
  const claim = () => run(() => broadcastAndReport(core.claim(params, outpoint), 'inheritance_claim', id))

  const toneOf = (label: string) =>
    params.heirs.some((h) => h.label === label) ? 'cold' : /dueña/i.test(label) ? 'hot' : undefined
</script>

<article class="card {ph.phase}">
  <header>
    <strong>{role === 'owner' ? '📜 Tu herencia' : `📜 Herencia de ${owner}`}</strong>
    <span class="muted">{sats(params.amount_sat)} · se reparte tras {params.delay} bloques sin moverse ({simDuration(params.delay, chain.simBlock)})</span>
  </header>

  <div class="story">
    {#if ph.phase === 'loading'}
      <p class="muted">Buscando la herencia en la cadena…</p>
    {:else if ph.phase === 'funding'}
      <p>Guardando la herencia. Se confirma en el próximo bloque.</p>
    {:else if ph.phase === 'waiting'}
      <div class="timer">
        ⏱ Si {role === 'owner' ? 'no das' : `${owner} no da`} señales de vida en <b>{ph.blocksLeft}</b> bloques,
        se reparte: el <b>{date(simDate(chain.height + 1 + (ph.blocksLeft ?? 0)))}</b>
      </div>
    {:else if ph.phase === 'claimable'}
      <div class="timer due">⏰ ¡Plazo cumplido! Cualquiera puede ejecutar el reparto.</div>
    {:else if ph.phase === 'distributed'}
      <p>📜 Herencia repartida.</p>
    {:else}
      <p>↩ {role === 'owner' ? 'Recuperaste' : `${owner} recuperó`} el dinero: ya no hay herencia.</p>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>

  {#if ph.phase !== 'loading' && ph.phase !== 'funding'}
    <div class="sides">
      <div class="side hot" class:winner={ph.phase === 'moved'}>
        <p class="title">{role === 'owner' ? '🔥 Tú, la dueña' : `🔥 ${owner}, la dueña`}</p>
        {#if role === 'owner' && (ph.phase === 'waiting' || ph.phase === 'claimable')}
          <p>Mientras la muevas, nadie hereda nada. Cada vez que la mueves, el reloj vuelve a empezar.</p>
          <div class="row">
            <button class="hot" onclick={keepAlive} disabled={busy}>💓 Sigo viva</button>
            <button class="secondary" onclick={withdraw} disabled={busy}>Recuperar todo</button>
          </div>
        {:else if ph.phase === 'waiting' || ph.phase === 'claimable'}
          <p>Puede moverla en cualquier momento, y el reloj vuelve a empezar.</p>
        {:else}
          <p class="muted">Camino cerrado.</p>
        {/if}
      </div>
      <div class="side cold" class:winner={ph.phase === 'distributed'}>
        <p class="title">🧊 Herederos</p>
        <ul>
          {#each split as h, i}
            <li class:me={i === mine}>{h.label}{i === mine ? ' (tú)' : ''}: <b>{sats(h.value_sat)}</b></li>
          {/each}
        </ul>
        {#if ph.phase === 'claimable'}
          <button class="cold" onclick={claim} disabled={busy}>📜 Ejecutar el reparto</button>
        {/if}
      </div>
    </div>
  {/if}

  <footer>
    <button class="secondary" onclick={() => (showTree = !showTree)}>{showTree ? 'Ocultar' : 'Ver'} árbol de transacciones</button>
    {#if role === 'owner' && (ph.phase === 'distributed' || ph.phase === 'moved')}
      <button class="secondary" onclick={() => removeContract(id)}>Archivar</button>
    {/if}
  </footer>
  {#if showTree}
    <div class="tree"><TreeView contract={view} {statuses} {toneOf} /></div>
  {/if}
</article>

<style>
  .card { border: 2px solid var(--line); border-radius: 16px; padding: 1rem; display: flex; flex-direction: column; gap: 0.75rem; margin: 1rem 0; }
  .card.claimable { border-color: var(--cold); }
  header, .story { text-align: center; }
  header { display: flex; flex-wrap: wrap; gap: 0.5rem; justify-content: center; align-items: baseline; }
  .story p { margin: 0.25rem 0; }
  .timer { background: var(--hot-bg); border-radius: 10px; padding: 0.6rem 0.8rem; font-weight: 600; }
  .timer.due { background: var(--cold-bg); }
  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .side { border-radius: 12px; padding: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; border: 2px solid transparent; }
  .side p { margin: 0; font-size: 0.92rem; }
  .side .title { font-weight: 700; }
  .side.hot { background: var(--hot-bg); }
  .side.cold { background: var(--cold-bg); }
  .side.hot.winner { border-color: var(--hot); }
  .side.cold.winner { border-color: var(--cold); }
  ul { margin: 0; padding-left: 1.1rem; }
  li.me { font-weight: 700; }
  .row, footer { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .tree { overflow-x: auto; }
  @media (max-width: 40rem) { .sides { grid-template-columns: 1fr; } }
</style>
