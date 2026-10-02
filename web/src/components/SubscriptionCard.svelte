<script lang="ts">
  import { subscription as core, type ContractView, type SubscriptionParams } from '../lib/core'
  import { broadcastAndReport, friendly } from '../lib/act'
  import { chain, me, removeContract, simDate } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { subscriptionState } from '../lib/contractPhase'
  import { txUrl } from '../lib/explorer'
  import { date, sats, short, simDuration } from '../lib/format'

  let {
    id,
    params,
    outpoint,
    owner,
    receiver,
    role,
  }: {
    id: string
    params: SubscriptionParams
    outpoint: string
    owner: string
    receiver: string
    role: 'payer' | 'receiver'
  } = $props()

  let view = $derived(core.view(params, outpoint))
  const st = useStatuses(() => view)
  let s = $derived(subscriptionState(view, st.map, params.count, params.every, chain.height))
  let due = $derived(s.phase === 'active' && !s.pending && s.blocksLeft != null && s.blocksLeft <= 0)
  let left = $derived((params.count - s.paid) * params.payment_sat)
  let busy = $state(false)
  let error = $state('')
  // The receiver's app collects due payments by itself (anyone could push them).
  let autoCollect = $state(true)
  let pushedStep = 0

  // Creation, then every payment (and the cancellation) as they happen.
  let links = $derived.by(() => {
    const out = [{ label: 'Creación', txid: id }]
    let c: ContractView | null = view
    for (let k = 1; c; k++) {
      const spent = st.map.get(c.outpoint!)
      if (!spent?.spentBy) break
      out.push({ label: spent.spentLeaf === 1 ? 'Cancelación' : `Pago ${k}`, txid: spent.spentBy })
      c = spent.spentLeaf === 0 ? (c.leaves[0].tx?.outputs[1]?.contract ?? null) : null
    }
    return out
  })

  $effect(() => {
    void chain.tick
    if (role === 'receiver' && autoCollect && due && pushedStep !== s.step && !busy) collect(true)
  })

  async function run(fn: () => Promise<void>): Promise<boolean> {
    busy = true
    error = ''
    try {
      await fn()
      return true
    } catch (e) {
      error = friendly(e)
      return false
    } finally {
      busy = false
    }
  }

  async function collect(auto = false) {
    const k = s.step
    pushedStep = k
    const ok = await run(() => broadcastAndReport(core.pay(params, k, outpoint), 'subscription_pay', id))
    if (!ok) {
      // e.g. the indexer was a block behind: let the next block retry it
      pushedStep = 0
      // and don't alarm anyone over an automatic attempt (it may have raced a cancel)
      if (auto) error = ''
    }
  }
  const cancel = () =>
    run(() =>
      broadcastAndReport(core.cancel(params, s.step, outpoint, me.id!.seed, me.wallet!.main_address), 'subscription_cancel', id),
    )
</script>

<article class="panel card">
  <header>
    <div>
      <div class="eyebrow">{role === 'payer' ? `Pagas a ${receiver}` : `${owner} te paga`}</div>
      <div class="value">{sats(params.payment_sat)} <span class="muted per">× {params.count}</span></div>
    </div>
    <div class="meta">
      <span class="badge">cada {params.every} bloques · {simDuration(params.every, chain.simBlock)}</span>
      {#if s.phase === 'active'}<span class="badge ok">Activa · {s.paid}/{params.count}</span>
      {:else if s.phase === 'cancelled'}<span class="badge hot">Cancelada</span>
      {:else if s.phase === 'done'}<span class="badge cold">Completada</span>
      {:else}<span class="badge">Confirmando…</span>{/if}
    </div>
  </header>

  <div class="progress" aria-label="Pagos">
    {#each Array(params.count) as _, i}
      {@const k = i + 1}
      <span class="pay" class:paid={k <= s.paid} class:current={s.phase === 'active' && k === s.step} class:void={s.phase === 'cancelled' && k >= s.step}></span>
    {/each}
  </div>

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

  {#if s.phase === 'loading' || s.phase === 'funding'}
    <p class="muted">Reservando el dinero de todos los pagos. Se confirma en el próximo bloque.</p>
  {:else if s.phase === 'active'}
    <div class="timer" class:due>
      {#if s.pending}
        Pago {s.paid} en camino…
      {:else if due}
        El pago {s.step} ya se puede cobrar.
      {:else}
        Pago {s.step} de {params.count} en <b>{s.blocksLeft} bloques</b>: el {date(simDate(chain.height + 1 + (s.blocksLeft ?? 0)))}
      {/if}
    </div>
  {:else if s.phase === 'cancelled'}
    <p>Suscripción cancelada tras {s.paid} {s.paid === 1 ? 'pago' : 'pagos'}. Lo que quedaba volvió a {role === 'payer' ? 'tu cartera' : owner}.</p>
  {:else}
    <p>Suscripción completada: {params.count} pagos.</p>
  {/if}

  {#if s.phase === 'active'}
    <div class="sides">
      <div class="side">
        <div class="eyebrow"><span class="dot hot"></span>{role === 'payer' ? 'Tú pagas' : `${owner} paga`}</div>
        <p>Quedan <b>{sats(left)}</b> reservados para {params.count - s.paid} {params.count - s.paid === 1 ? 'pago' : 'pagos'}.</p>
        {#if role === 'payer'}
          <p class="muted">Puedes cancelar cuando quieras: lo que no se ha pagado vuelve a ti.</p>
          <button class="secondary" onclick={cancel} disabled={busy}>Cancelar suscripción</button>
        {:else}
          <p class="muted">Puede cancelar cuando quiera, pero lo ya cobrado es tuyo.</p>
        {/if}
      </div>
      <div class="side">
        <div class="eyebrow"><span class="dot cold"></span>{role === 'receiver' ? 'Tú cobras' : `${receiver} cobra`}</div>
        <p>Cobrados <b>{sats(s.paid * params.payment_sat)}</b> de {sats(params.count * params.payment_sat)}.</p>
        <p class="muted">Nadie tiene que firmar: cuando vence un pago, cualquiera puede publicarlo.</p>
        <button class="cold" onclick={() => collect()} disabled={busy || !due}>Cobrar pago {s.step}</button>
        {#if role === 'receiver'}
          <label class="auto"><input type="checkbox" bind:checked={autoCollect} /> Cobrar automáticamente</label>
        {/if}
      </div>
    </div>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}
  {#if role === 'payer' && (s.phase === 'cancelled' || s.phase === 'done')}
    <footer><button class="ghost" onclick={() => removeContract(id)}>Archivar</button></footer>
  {/if}
</article>

<style>
  .card { display: flex; flex-direction: column; gap: 0.8rem; margin: 1rem 0; }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; flex-wrap: wrap; }
  .value { font-size: 1.3rem; font-weight: 650; font-variant-numeric: tabular-nums; }
  .per { font-size: 1rem; font-weight: 500; }
  .meta { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .progress { display: flex; gap: 0.3rem; }
  .pay { flex: 1; height: 6px; border-radius: 999px; background: var(--card); }
  .pay.paid { background: var(--cold); }
  .pay.current { background: var(--hot); }
  .pay.void { background: var(--line); opacity: 0.5; }
  .txs ol { list-style: none; margin: 0.35rem 0 0; padding: 0; }
  .txs li { display: grid; grid-template-columns: 1.4rem 1fr auto; gap: 0.5rem; align-items: center; padding: 0.4rem 0; border-top: 1px solid var(--line); font-size: 0.88rem; }
  .step { width: 1.4rem; height: 1.4rem; border-radius: 999px; background: var(--card); color: var(--muted); display: grid; place-items: center; font-size: 0.72rem; font-weight: 600; }
  .txs a { text-decoration: none; font-size: 0.8rem; }
  .timer { background: var(--hot-bg); border-radius: 8px; padding: 0.6rem 0.8rem; font-size: 0.92rem; }
  .timer.due { background: var(--cold-bg); }
  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .side { border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem; display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; }
  .side p { margin: 0; font-size: 0.9rem; }
  .auto { flex-direction: row; align-items: center; gap: 0.4rem; font-weight: 400; }
  .auto input { width: auto; }
  footer { display: flex; }
  @media (max-width: 40rem) { .sides { grid-template-columns: 1fr; } }
</style>
