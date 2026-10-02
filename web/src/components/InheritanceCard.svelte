<script lang="ts">
  import { inheritance as core, type InheritanceParams } from '../lib/core'
  import { broadcastAndReport, friendly } from '../lib/act'
  import { registerContract } from '../lib/api'
  import { chain, me, removeContract, saveContract, simDate } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { inheritancePhase } from '../lib/contractPhase'
  import { txUrl } from '../lib/explorer'
  import { date, sats, short, simDuration } from '../lib/format'

  let {
    id,
    params,
    outpoint,
    owner,
    role,
  }: { id: string; params: InheritanceParams; outpoint: string; owner: string; role: 'owner' | 'heir' } =
    $props()

  let view = $derived(core.view(params, outpoint))
  const st = useStatuses(() => view)
  let ph = $derived(inheritancePhase(view, st.map, params.delay, chain.height))
  let split = $derived(view.leaves[1].tx!.outputs)
  let mine = $derived(me.wallet ? params.heirs.findIndex((h) => h.address === me.wallet!.main_address) : -1)
  let ended = $derived(ph.phase === 'distributed' || ph.phase === 'moved')
  let busy = $state(false)
  let error = $state('')

  // Creation, the latest "I'm alive" (it moves the coins), and how it ended.
  let links = $derived.by(() => {
    const out = [{ label: 'Creación', txid: id }]
    const current = outpoint.split(':')[0]
    if (current !== id) out.push({ label: 'Última señal de vida', txid: current })
    const spent = st.map.get(outpoint)?.spentBy
    if (spent) out.push({ label: ph.phase === 'distributed' ? 'Reparto' : 'Recuperada', txid: spent })
    return out
  })

  async function run(fn: () => Promise<void>) {
    busy = true
    error = ''
    try {
      await fn()
    } catch (e) {
      error = friendly(e)
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
</script>

<article class="panel card">
  <header>
    <div>
      <div class="eyebrow">{role === 'owner' ? 'Tu herencia' : `Herencia de ${owner}`}</div>
      {#if ended}
        <div class="value">{ph.phase === 'distributed' ? 'Repartida' : 'Recuperada'}</div>
        <div class="muted small">Tenía {sats(params.amount_sat)}</div>
      {:else}
        <div class="value">{sats(params.amount_sat)}</div>
      {/if}
    </div>
    <div class="meta">
      <span class="badge">sin moverse {params.delay} bloques · {simDuration(params.delay, chain.simBlock)}</span>
      {#if ph.phase === 'waiting'}<span class="badge ok">Esperando</span>
      {:else if ph.phase === 'claimable'}<span class="badge cold">Plazo cumplido</span>
      {:else if ended}<span class="badge">{ph.phase === 'distributed' ? 'Repartida' : 'Recuperada'}</span>
      {:else}<span class="badge">Confirmando…</span>{/if}
    </div>
  </header>

  <div class="txs">
    <div class="eyebrow">Transacciones</div>
    <ol>
      {#each links as l, i (l.txid)}
        <li>
          <span class="step">{i + 1}</span>
          <span>{l.label}</span>
          <a class="mono" href={txUrl(l.txid)} target="_blank" rel="noopener" title={l.txid}>{short(l.txid, 8)} ↗</a>
        </li>
      {/each}
    </ol>
  </div>

  {#if ph.phase === 'waiting'}
    <div class="timer">
      Si {role === 'owner' ? 'no das' : `${owner} no da`} señales de vida en <b>{ph.blocksLeft} bloques</b>, se reparte:
      el {date(simDate(chain.height + 1 + (ph.blocksLeft ?? 0)))}
    </div>
  {:else if ph.phase === 'claimable'}
    <div class="timer due">Plazo cumplido: cualquiera puede ejecutar el reparto.</div>
  {:else if ph.phase === 'loading' || ph.phase === 'funding'}
    <p class="muted">Guardando la herencia. Se confirma en el próximo bloque.</p>
  {/if}

  {#if ph.phase !== 'loading' && ph.phase !== 'funding'}
    <div class="sides">
      <div class="side">
        <div class="eyebrow"><span class="dot hot"></span>{role === 'owner' ? 'Tú, la dueña' : `${owner}, la dueña`}</div>
        {#if role === 'owner' && !ended}
          <p>Mientras la muevas, nadie hereda nada. Cada vez que la mueves, el reloj vuelve a empezar.</p>
          <div class="row">
            <button onclick={keepAlive} disabled={busy}>Sigo viva</button>
            <button class="secondary" onclick={withdraw} disabled={busy}>Recuperar todo</button>
          </div>
        {:else if !ended}
          <p>Puede moverla en cualquier momento, y el reloj vuelve a empezar.</p>
        {:else if ph.phase === 'moved'}
          <p>{role === 'owner' ? 'Recuperaste' : `${owner} recuperó`} el dinero: ya no hay herencia.</p>
        {:else}
          <p class="muted">Camino cerrado.</p>
        {/if}
      </div>
      <div class="side">
        <div class="eyebrow"><span class="dot cold"></span>Herederos</div>
        <ul>
          {#each split as h, i}
            <li class:me={i === mine}><span>{h.label}{i === mine ? ' (tú)' : ''}</span><span class="mono">{sats(h.value_sat)}</span></li>
          {/each}
        </ul>
        {#if ph.phase === 'claimable'}
          <button class="cold" onclick={claim} disabled={busy}>Ejecutar el reparto</button>
        {:else if ph.phase === 'distributed'}
          <p>Herencia repartida.</p>
        {/if}
      </div>
    </div>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}
  {#if role === 'owner' && ended}
    <footer><button class="ghost" onclick={() => removeContract(id)}>Archivar</button></footer>
  {/if}
</article>

<style>
  .card { display: flex; flex-direction: column; gap: 0.8rem; margin: 1rem 0; }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; flex-wrap: wrap; }
  .value { font-size: 1.3rem; font-weight: 650; font-variant-numeric: tabular-nums; }
  .small { font-size: 0.82rem; }
  .meta { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .txs ol { list-style: none; margin: 0.35rem 0 0; padding: 0; }
  .txs li { display: grid; grid-template-columns: 1.4rem 1fr auto; gap: 0.5rem; align-items: center; padding: 0.4rem 0; border-top: 1px solid var(--line); font-size: 0.88rem; }
  .step { width: 1.4rem; height: 1.4rem; border-radius: 999px; background: var(--card); color: var(--muted); display: grid; place-items: center; font-size: 0.72rem; font-weight: 600; }
  .txs a { text-decoration: none; font-size: 0.8rem; }
  .timer { background: var(--hot-bg); border-radius: 8px; padding: 0.6rem 0.8rem; font-size: 0.92rem; }
  .timer.due { background: var(--cold-bg); }
  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .side { border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem; display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; }
  .side p { margin: 0; font-size: 0.9rem; }
  ul { list-style: none; margin: 0; padding: 0; width: 100%; }
  ul li { display: flex; justify-content: space-between; font-size: 0.9rem; padding: 0.3rem 0; border-top: 1px solid var(--line); }
  ul li.me { font-weight: 650; }
  .row, footer { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  @media (max-width: 40rem) { .sides { grid-template-columns: 1fr; } }
</style>
