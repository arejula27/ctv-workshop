<script lang="ts">
  import { onMount } from 'svelte'
  import { newSeed, walletInfo } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { createPool, joinPool, listPools, proposePool, startPool, type PoolTable } from '../lib/api'
  import { friendly } from '../lib/act'
  import { chain, me, poolGame, savePoolGame } from '../lib/store.svelte'
  import { sats } from '../lib/format'
  import WalletPanel from '../components/WalletPanel.svelte'
  import PoolInstant from '../components/PoolInstant.svelte'
  import Modal from '../components/Modal.svelte'

  let tables = $state<PoolTable[]>([])
  let name = $state('')
  let busy = $state(false)
  let error = $state('')
  let depositing = $state(false)
  let amount = $state<number | null>(null)
  let onchainBalance = $state(0)

  let wallet = $derived(poolGame.seed ? walletInfo(poolGame.seed) : null)
  const refresh = () => listPools().then((t) => (tables = t)).catch(() => {})
  $effect(() => {
    void chain.poolsTick
    refresh()
  })
  onMount(() => {
    // Signatures come and go fast: keep the pool fresh.
    const t = setInterval(refresh, 3000)
    return () => clearInterval(t)
  })

  const seated = (t: PoolTable) => t.members.some((m) => m.pubkey === wallet?.main_pubkey)
  let open = $derived(tables.filter((t) => t.state === 'open' || t.state === 'funding'))
  let mine = $derived(tables.find((t) => t.state === 'funded' && seated(t)))

  async function run(fn: () => Promise<unknown>) {
    busy = true
    error = ''
    try {
      await fn()
      await refresh()
    } catch (e) {
      error = friendly(e)
    } finally {
      busy = false
    }
  }

  const member = () => ({ label: me.id!.name, pubkey: wallet!.main_pubkey, address: wallet!.main_address })

  function create(e: SubmitEvent) {
    e.preventDefault()
    run(async () => {
      const t = await createPool(name.trim(), me.id!.name)
      await joinPool(t.id, member())
      name = ''
    })
  }

  function deposit(e: SubmitEvent) {
    e.preventDefault()
    run(async () => {
      const utxos = (await esplora.utxos(wallet!.main_address)).map(({ txid, vout, value, address }) => ({ txid, vout, value, address }))
      await proposePool(mine!.id, { by: wallet!.main_pubkey, op: 'deposit', amount_sat: amount!, utxos })
      depositing = false
      amount = null
    })
  }
</script>

<div class="head">
  <div class="eyebrow">Demo · Payment pool</div>
  <h1>Tu cartera</h1>
  <p class="muted">Dos cuentas: la <b>on-chain</b>, como cualquier cartera, y la <b>instantánea</b>, tu parte de un UTXO que compartes con tu mesa.</p>
</div>

{#if !wallet}
  <section class="panel setup">
    <h2>Crea tu cartera</h2>
    <p class="muted">Tendrá una cuenta on-chain y, cuando te sientes en una mesa, una cuenta dentro del pool.</p>
    <button onclick={() => savePoolGame({ seed: newSeed() })}>Crear cartera</button>
  </section>
{:else}
  <div class="grid">
    <WalletPanel bind:balance={onchainBalance} title="Cuenta on-chain" subtitle="Una dirección normal de bitcoin." seed={poolGame.seed!}>
      {#snippet actions()}
        {#if mine}<button onclick={() => (depositing = true)} disabled={onchainBalance === 0 || !!mine.pending}>Meter en el pool</button>{/if}
      {/snippet}
    </WalletPanel>

    {#if mine}
      <PoolInstant table={mine} onchange={refresh} />
    {:else}
      <section class="panel cold setup">
        <div class="eyebrow">Cuenta instantánea</div>
        <h2>Siéntate en una mesa</h2>
        <p class="muted">Entre 2 y 6 personas comparten un solo UTXO. La casa pone {sats(100_000)} por persona para empezar.</p>
        {#each open as t (t.id)}
          <div class="table">
            <div><b>«{t.name}»</b> <span class="muted">· {t.members.length}/6</span></div>
            <div class="muted small">{t.members.map((m) => m.label).join(', ') || 'vacía'}</div>
            <div class="row">
              {#if !seated(t)}
                <button class="secondary" onclick={() => run(() => joinPool(t.id, member()))} disabled={busy || t.members.length >= 6 || t.state !== 'open'}>Sentarme</button>
              {:else}
                <span class="muted small">Ya estás en esta mesa</span>
              {/if}
              <button onclick={() => run(() => startPool(t.id))} disabled={busy || t.members.length < 2 || !seated(t) || t.state !== 'open'}>
                {t.state === 'funding' ? 'Empezando…' : 'Empezar'}
              </button>
            </div>
          </div>
        {/each}
        <form onsubmit={create}>
          <input bind:value={name} placeholder="Nombre de una mesa nueva" maxlength="24" required aria-label="Nombre de una mesa nueva" />
          <button type="submit" class="secondary" disabled={busy || !name.trim()}>Abrir mesa</button>
        </form>
      </section>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
{/if}

{#if depositing && mine}
  <Modal title="Meter en el pool" onclose={() => (depositing = false)}>
    <p class="muted">Pasa dinero de tu cuenta on-chain a tu saldo instantáneo. Como cambia el reparto, firma toda la mesa (sus apps lo hacen solas).</p>
    <form onsubmit={deposit}>
      <label>Cantidad (sats) <input type="number" bind:value={amount} min="330" max={onchainBalance - mine.fee_sat} required /></label>
      {#if error}<p class="error">{error}</p>{/if}
      <button type="submit" disabled={busy}>{busy ? 'Proponiendo…' : 'Meter en el pool'}</button>
    </form>
  </Modal>
{/if}

<style>
  .head { margin-bottom: 1rem; }
  .head p { margin: 0; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; align-items: start; }
  .setup { display: flex; flex-direction: column; gap: 0.6rem; align-items: flex-start; }
  .setup p { margin: 0; font-size: 0.9rem; }
  .table { width: 100%; border: 1px solid var(--line); border-radius: 10px; padding: 0.6rem 0.75rem; display: flex; flex-direction: column; gap: 0.3rem; }
  .row, form { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; width: 100%; }
  form input { flex: 1; min-width: 10rem; }
  .small { font-size: 0.85rem; }
  @media (max-width: 44rem) { .grid { grid-template-columns: 1fr; } }
</style>
