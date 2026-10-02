<script lang="ts">
  import { inheritance as core, walletSend, type InheritanceParams } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { registerContract } from '../lib/api'
  import { friendly } from '../lib/act'
  import { balances, chain, contracts, me, saveContract, spendableUtxos } from '../lib/store.svelte'
  import { room, syncRoom } from '../lib/room.svelte'
  import { blocksToTime, sats, simDuration } from '../lib/format'
  import InheritanceCard from '../components/InheritanceCard.svelte'

  const FEE_SAT = 1_000
  let amount = $state(300_000)
  let delay = $state(12)
  let picked = $state<string[]>([])
  let busy = $state(false)
  let error = $state('')

  $effect(() => {
    void chain.feed.length
    void chain.contractsTick
    syncRoom()
  })

  let others = $derived(room.people.filter((p) => p.address !== me.wallet?.main_address))
  let mine = $derived(contracts.list.filter((c) => c.kind === 'inheritance'))
  let forMe = $derived(
    room.contracts.filter(
      (c) => c.kind === 'inheritance' && c.params.heirs.some((h) => h.address === me.wallet?.main_address),
    ),
  )

  async function create(e: SubmitEvent) {
    e.preventDefault()
    if (!me.id || !me.wallet) return
    if (!picked.length) {
      error = 'Elige al menos una persona.'
      return
    }
    busy = true
    error = ''
    try {
      const params: InheritanceParams = {
        amount_sat: amount,
        owner_pubkey: me.wallet.main_pubkey,
        heirs: picked.map((address) => ({ label: others.find((p) => p.address === address)!.name, address, share: 1 })),
        delay,
        fee_sat: FEE_SAT,
      }
      const tx = walletSend(me.id.seed, await spendableUtxos(), core.address(params), amount)
      await esplora.broadcast(tx.hex)
      const outpoint = `${tx.txid}:0`
      saveContract({ id: tx.txid, kind: 'inheritance', params, outpoint, createdAt: Date.now() })
      await registerContract(tx.txid, 'inheritance', me.id.name, params, outpoint)
      picked = []
    } catch (err) {
      error = friendly(err)
    } finally {
      busy = false
    }
  }
</script>

<h1>Herencia</h1>
<p class="intro">Si dejas de dar señales de vida, tu dinero se reparte solo, como dejaste escrito. Sin notario y sin que nadie pueda cambiar el reparto.</p>

<div class="split">
  <section class="half hot">
    <div class="icon">🔥</div>
    <h2>Tu cartera caliente</h2>
    <div class="bal">{balances.loaded ? sats(balances.main) : '…'}</div>
    <form onsubmit={create}>
      <label>
        Dejar en herencia (sats)
        <input type="number" bind:value={amount} min="10000" step="1000" required />
      </label>
      <label>
        Se reparte si no la mueves en (bloques)
        <input type="number" bind:value={delay} min="1" max="2000" required />
        <span class="hint">= {simDuration(delay, chain.simBlock)} en Bitcoin real · {blocksToTime(delay, chain.miner?.interval_s)} hoy en la sala</span>
      </label>
      <fieldset>
        <legend>¿Para quién? (a partes iguales)</legend>
        {#if others.length === 0}
          <p class="hint">Todavía no hay nadie más en la sala.</p>
        {/if}
        {#each others as p (p.address)}
          <label class="check"><input type="checkbox" value={p.address} bind:group={picked} /> {p.name}</label>
        {/each}
      </fieldset>
      <button type="submit" class="hot" disabled={busy || !picked.length}>Dejar herencia</button>
    </form>
    {#if error}<p class="error">{error}</p>{/if}
  </section>

  <section class="half cold">
    <div class="icon">🧊</div>
    <h2>Te han dejado en herencia</h2>
    <div class="bal">{forMe.length} {forMe.length === 1 ? 'herencia' : 'herencias'}</div>
    <p class="hint">Si la dueña deja de dar señales de vida, cualquiera puede ejecutar el reparto y tu parte llega a tu cartera.</p>
  </section>
</div>

{#each mine as c (c.id)}
  <InheritanceCard id={c.id} params={c.params} outpoint={c.outpoint} owner={me.id?.name ?? ''} role="owner" />
{/each}
{#each forMe as c (c.id)}
  {#if c.kind === 'inheritance'}
    <InheritanceCard id={c.id} params={c.params} outpoint={c.outpoint} owner={c.owner} role="heir" />
  {/if}
{/each}

<style>
  .intro { margin-top: 0; }
  .split { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin: 1rem 0; }
  .half { border-radius: 16px; padding: 1rem; display: flex; flex-direction: column; gap: 0.5rem; }
  .half { background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow); }
  h2 { margin: 0; }
  .icon { font-size: 2rem; }
  .hint { font-size: 0.85rem; color: var(--muted); margin: 0; }
  .bal { font-size: 1.6rem; font-weight: 700; font-variant-numeric: tabular-nums; }
  form { display: flex; flex-direction: column; gap: 0.6rem; }
  label { display: flex; flex-direction: column; gap: 0.25rem; font-weight: 600; font-size: 0.9rem; }
  fieldset { border: 1px solid var(--line); border-radius: 10px; display: flex; flex-direction: column; gap: 0.3rem; max-height: 14rem; overflow-y: auto; }
  legend { font-weight: 600; font-size: 0.9rem; }
  .check { flex-direction: row; align-items: center; font-weight: 400; }
  .check input { width: auto; }
  @media (max-width: 40rem) { .split { grid-template-columns: 1fr; } }
</style>
