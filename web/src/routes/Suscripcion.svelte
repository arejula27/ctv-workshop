<script lang="ts">
  import { subscription as core, walletSend, type SubscriptionParams } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { registerContract } from '../lib/api'
  import { friendly } from '../lib/act'
  import { balances, chain, contracts, me, saveContract, spendableUtxos } from '../lib/store.svelte'
  import { nameOf, room, syncRoom } from '../lib/room.svelte'
  import { blocksToTime, sats, simDuration } from '../lib/format'
  import SubscriptionCard from '../components/SubscriptionCard.svelte'

  const FEE_SAT = 500
  let receiver = $state('')
  let payment = $state(10_000)
  let count = $state(4)
  let every = $state(6)
  let busy = $state(false)
  let error = $state('')

  $effect(() => {
    void chain.feed.length
    void chain.contractsTick
    syncRoom()
  })

  let others = $derived(room.people.filter((p) => p.address !== me.wallet?.main_address))
  let mine = $derived(contracts.list.filter((c) => c.kind === 'subscription'))
  let forMe = $derived(
    room.contracts.filter((c) => c.kind === 'subscription' && c.params.receiver_address === me.wallet?.main_address),
  )
  let params = $derived<SubscriptionParams | null>(
    me.wallet && receiver
      ? { payment_sat: payment, count, every, receiver_address: receiver, payer_pubkey: me.wallet.main_pubkey, fee_sat: FEE_SAT }
      : null,
  )
  let total = $derived.by(() => {
    try {
      return params ? core.amount(params) : null
    } catch {
      return null
    }
  })

  async function create(e: SubmitEvent) {
    e.preventDefault()
    if (!me.id || !params || total == null) return
    busy = true
    error = ''
    try {
      const tx = walletSend(me.id.seed, await spendableUtxos(), core.address(params), total)
      await esplora.broadcast(tx.hex)
      const outpoint = `${tx.txid}:0`
      saveContract({ id: tx.txid, kind: 'subscription', params, outpoint, createdAt: Date.now() })
      await registerContract(tx.txid, 'subscription', me.id.name, params, outpoint)
    } catch (err) {
      error = friendly(err)
    } finally {
      busy = false
    }
  }
</script>

<h1>Suscripción</h1>
<p class="intro">Pagos que llegan solos cada cierto tiempo, sin dar permiso a nadie para cobrarte. Y puedes cancelar cuando quieras.</p>

<div class="split">
  <section class="half hot">
    <div class="icon">🔥</div>
    <h2>Tú pagas</h2>
    <div class="bal">{balances.loaded ? sats(balances.main) : '…'}</div>
    <form onsubmit={create}>
      <label>
        ¿A quién?
        <select bind:value={receiver} required>
          <option value="" disabled>Elige a alguien de la sala</option>
          {#each others as p (p.address)}<option value={p.address}>{p.name}</option>{/each}
        </select>
      </label>
      <label>
        Cada pago (sats)
        <input type="number" bind:value={payment} min="1000" step="1000" required />
      </label>
      <label>
        Número de pagos
        <input type="number" bind:value={count} min="1" max="24" required />
      </label>
      <label>
        Cada (bloques)
        <input type="number" bind:value={every} min="1" max="2000" required />
        <span class="hint">= {simDuration(every, chain.simBlock)} en Bitcoin real · {blocksToTime(every, chain.miner?.interval_s)} hoy en la sala</span>
      </label>
      {#if total != null}
        <p class="hint">Se reservan {sats(total)}: los {count} pagos y las comisiones.</p>
      {/if}
      <button type="submit" class="hot" disabled={busy || total == null}>Crear suscripción</button>
    </form>
    {#if error}<p class="error">{error}</p>{/if}
  </section>

  <section class="half cold">
    <div class="icon">🧊</div>
    <h2>Tú cobras</h2>
    <div class="bal">{forMe.length} {forMe.length === 1 ? 'suscripción' : 'suscripciones'}</div>
    <p class="hint">Cuando vence un pago, tu app lo cobra sola. Nadie tiene que firmar nada.</p>
  </section>
</div>

{#each mine as c (c.id)}
  {#if c.kind === 'subscription'}
    <SubscriptionCard id={c.id} params={c.params} outpoint={c.outpoint} owner={me.id?.name ?? ''} receiver={nameOf(c.params.receiver_address)} role="payer" />
  {/if}
{/each}
{#each forMe as c (c.id)}
  {#if c.kind === 'subscription'}
    <SubscriptionCard id={c.id} params={c.params} outpoint={c.outpoint} owner={c.owner} receiver={me.id?.name ?? ''} role="receiver" />
  {/if}
{/each}

<style>
  .intro { margin-top: 0; }
  .split { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin: 1rem 0; }
  .half { border-radius: 16px; padding: 1rem; display: flex; flex-direction: column; gap: 0.5rem; }
  .half.hot { background: var(--hot-bg); border: 2px solid var(--hot); }
  .half.cold { background: var(--cold-bg); border: 2px solid var(--cold); }
  h2 { margin: 0; }
  .icon { font-size: 2rem; }
  .hint { font-size: 0.85rem; color: var(--muted); margin: 0; }
  .bal { font-size: 1.6rem; font-weight: 700; font-variant-numeric: tabular-nums; }
  form { display: flex; flex-direction: column; gap: 0.6rem; }
  label { display: flex; flex-direction: column; gap: 0.25rem; font-weight: 600; font-size: 0.9rem; }
  select { font: inherit; padding: 0.6rem 0.75rem; border-radius: 10px; border: 1px solid var(--line); background: var(--bg); color: var(--fg); }
  @media (max-width: 40rem) { .split { grid-template-columns: 1fr; } }
</style>
