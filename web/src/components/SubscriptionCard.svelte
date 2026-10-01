<script lang="ts">
  import { onMount } from 'svelte'
  import { subscription as core, type SubscriptionParams } from '../lib/core'
  import { broadcastAndReport } from '../lib/act'
  import { chain, me, removeContract, simDate } from '../lib/store.svelte'
  import { treeStatus, type ContractStatus } from '../lib/treeStatus'
  import { subscriptionState } from '../lib/contractPhase'
  import { date, sats, simDuration } from '../lib/format'
  import TreeView from './TreeView.svelte'

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
  let statuses = $state(new Map<string, ContractStatus>())
  let st = $derived(subscriptionState(view, statuses, params.count, params.every, chain.height))
  let due = $derived(st.phase === 'active' && !st.pending && st.blocksLeft != null && st.blocksLeft <= 0)
  let left = $derived((params.count - st.paid) * params.payment_sat)
  let busy = $state(false)
  let error = $state('')
  let showTree = $state(false)
  // The receiver's app collects due payments by itself (anyone could push them).
  let autoCollect = $state(true)
  let pushedStep = 0

  const refresh = () => treeStatus(view).then((s) => (statuses = s)).catch((e) => (error = e.message))
  onMount(() => {
    const t = setInterval(refresh, 4000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    refresh()
  })
  $effect(() => {
    void chain.tick
    if (role === 'receiver' && autoCollect && due && pushedStep !== st.step && !busy) collect(true)
  })

  async function run(fn: () => Promise<void>): Promise<boolean> {
    busy = true
    error = ''
    try {
      await fn()
      await refresh()
      return true
    } catch (e) {
      error = (e as Error).message
      return false
    } finally {
      busy = false
    }
  }

  async function collect(auto = false) {
    const k = st.step
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
      broadcastAndReport(
        core.cancel(params, st.step, outpoint, me.id!.seed, me.wallet!.main_address),
        'subscription_cancel',
        id,
      ),
    )

  const toneOf = (label: string) => (/cobro|pago/i.test(label) ? 'cold' : /cancelar/i.test(label) ? 'hot' : undefined)
</script>

<article class="card {st.phase}">
  <header>
    <strong>{role === 'payer' ? `📆 Pagas a ${receiver}` : `📆 ${owner} te paga`}</strong>
    <span class="muted">
      {sats(params.payment_sat)} cada {params.every} bloques ({simDuration(params.every, chain.simBlock)}) · {params.count} pagos
    </span>
  </header>

  <!-- One dot per payment -->
  <div class="dots" aria-label="Pagos">
    {#each Array(params.count) as _, i}
      {@const k = i + 1}
      <span
        class="dot"
        class:paid={k <= st.paid}
        class:current={st.phase === 'active' && k === st.step}
        class:void={st.phase === 'cancelled' && k >= st.step}
        title="Pago {k}"
      >{k <= st.paid ? '✓' : st.phase === 'cancelled' && k >= st.step ? '✗' : k}</span>
    {/each}
  </div>

  <div class="story">
    {#if st.phase === 'loading'}
      <p class="muted">Buscando la suscripción en la cadena…</p>
    {:else if st.phase === 'funding'}
      <p>Reservando el dinero de todos los pagos. Se confirma en el próximo bloque.</p>
    {:else if st.phase === 'active'}
      <div class="timer" class:due>
        {#if st.pending}
          💸 Pago {st.paid} en camino…
        {:else if due}
          ⏰ El pago {st.step} ya se puede cobrar
        {:else}
          ⏱ Pago {st.step} de {params.count} en <b>{st.blocksLeft}</b> bloques: el
          <b>{date(simDate(chain.height + 1 + (st.blocksLeft ?? 0)))}</b>
        {/if}
      </div>
    {:else if st.phase === 'cancelled'}
      <p>✋ Suscripción cancelada tras {st.paid} {st.paid === 1 ? 'pago' : 'pagos'}. Lo que quedaba volvió a {role === 'payer' ? 'tu cartera' : owner}.</p>
    {:else}
      <p>✅ Suscripción completada: {params.count} pagos.</p>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>

  {#if st.phase === 'active'}
    <div class="sides">
      <div class="side hot">
        <p class="title">🔥 {role === 'payer' ? 'Tú pagas' : `${owner} paga`}</p>
        <p>Quedan <b>{sats(left)}</b> reservados para {params.count - st.paid} {params.count - st.paid === 1 ? 'pago' : 'pagos'}.</p>
        {#if role === 'payer'}
          <p>Puedes cancelar cuando quieras: lo que no se ha pagado vuelve a ti.</p>
          <button class="hot" onclick={cancel} disabled={busy}>✋ Cancelar suscripción</button>
        {:else}
          <p>Puede cancelar cuando quiera, pero lo ya cobrado es tuyo.</p>
        {/if}
      </div>
      <div class="side cold">
        <p class="title">🧊 {role === 'receiver' ? 'Tú cobras' : `${receiver} cobra`}</p>
        <p>Cobrados <b>{sats(st.paid * params.payment_sat)}</b> de {sats(params.count * params.payment_sat)}.</p>
        <p class="muted">Nadie tiene que firmar: cuando vence un pago, cualquiera puede publicarlo.</p>
        <button class="cold" onclick={() => collect()} disabled={busy || !due}>💸 Cobrar pago {st.step}</button>
        {#if role === 'receiver'}
          <label class="auto"><input type="checkbox" bind:checked={autoCollect} /> Cobrar automáticamente</label>
        {/if}
      </div>
    </div>
  {/if}

  <footer>
    <button class="secondary" onclick={() => (showTree = !showTree)}>{showTree ? 'Ocultar' : 'Ver'} árbol de transacciones</button>
    {#if role === 'payer' && (st.phase === 'cancelled' || st.phase === 'done')}
      <button class="secondary" onclick={() => removeContract(id)}>Archivar</button>
    {/if}
  </footer>
  {#if showTree}
    <div class="tree"><TreeView contract={view} {statuses} {toneOf} /></div>
  {/if}
</article>

<style>
  .card { border: 2px solid var(--line); border-radius: 16px; padding: 1rem; display: flex; flex-direction: column; gap: 0.75rem; margin: 1rem 0; }
  header, .story { text-align: center; }
  header { display: flex; flex-wrap: wrap; gap: 0.5rem; justify-content: center; align-items: baseline; }
  .story p { margin: 0.25rem 0; }
  .dots { display: flex; flex-wrap: wrap; gap: 0.4rem; justify-content: center; }
  .dot {
    width: 2rem; height: 2rem; border-radius: 999px; display: grid; place-items: center;
    background: var(--card); color: var(--muted); font-weight: 700; font-size: 0.85rem;
  }
  .dot.paid { background: var(--cold); color: #fff; }
  .dot.current { outline: 3px solid var(--hot); color: var(--fg); }
  .dot.void { background: var(--thief-bg); text-decoration: line-through; }
  .timer { background: var(--hot-bg); border-radius: 10px; padding: 0.6rem 0.8rem; font-weight: 600; }
  .timer.due { background: var(--cold-bg); }
  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .side { border-radius: 12px; padding: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; }
  .side p { margin: 0; font-size: 0.92rem; }
  .side .title { font-weight: 700; }
  .side.hot { background: var(--hot-bg); }
  .side.cold { background: var(--cold-bg); }
  .auto { font-size: 0.85rem; display: flex; gap: 0.4rem; align-items: center; }
  .auto input { width: auto; }
  footer { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .tree { overflow-x: auto; }
  @media (max-width: 40rem) { .sides { grid-template-columns: 1fr; } }
</style>
