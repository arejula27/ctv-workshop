<script lang="ts">
  import { secretPubkey, vault as core, walletSend, type VaultParams } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { faucet, listVaults, registerVault } from '../lib/api'
  import {
    addVault,
    balances,
    chain,
    me,
    refreshBalances,
    spendableUtxos,
    vaults,
  } from '../lib/store.svelte'
  import { blocksToTime, sats } from '../lib/format'
  import VaultCard from '../components/VaultCard.svelte'

  const FEE_SAT = 1_000

  let amount = $state(500_000)
  let delay = $state(10)
  let busy = $state(false)
  let error = $state('')
  let kitText = $state('')
  let kitError = $state('')

  let ownVaults = $derived(vaults.list.filter((v) => v.role === 'owner'))
  let stolen = $derived(vaults.list.filter((v) => v.role === 'thief'))

  async function create(e: SubmitEvent) {
    e.preventDefault()
    if (!me.id || !me.wallet) return
    busy = true
    error = ''
    try {
      const params: VaultParams = {
        amount_sat: amount,
        hot_pubkey: me.wallet.hot_pubkey,
        cold_pubkey: me.wallet.cold_pubkey,
        delay,
        fee_sat: FEE_SAT,
      }
      const utxos = await spendableUtxos()
      const tx = walletSend(me.id.seed, utxos, core.address(params), amount)
      await esplora.broadcast(tx.hex)
      // wallet_send always puts the payment in output 0
      const outpoint = `${tx.txid}:0`
      addVault({ id: tx.txid, role: 'owner', params, outpoint, createdAt: Date.now() })
      // Announce it so thieves (and /screen) can find it by its hot key.
      registerVault(me.id.name, params, outpoint).catch((e) => (error = `No se pudo anunciar el vault: ${e.message}`))
      refreshBalances().catch(() => {})
    } catch (e) {
      error = (e as Error).message
    } finally {
      busy = false
    }
  }

  async function askFaucet() {
    if (!me.wallet) return
    try {
      await faucet(me.wallet.main_address)
      await refreshBalances()
    } catch (e) {
      error = (e as Error).message
    }
  }

  async function importKit(e: SubmitEvent) {
    e.preventDefault()
    kitError = ''
    const secret = kitText.trim().toLowerCase()
    if (!/^[0-9a-f]{64}$/.test(secret)) {
      kitError = 'Una llave son 64 caracteres (0-9, a-f).'
      return
    }
    try {
      const pubkey = secretPubkey(secret)
      const targets = (await listVaults()).filter((v) => v.params.hot_pubkey === pubkey)
      if (!targets.length) throw new Error('Esa llave no abre ningún vault de la sala.')
      if (targets.every((v) => v.params.hot_pubkey === me.wallet?.hot_pubkey))
        throw new Error('Esa es tu propia llave 🙂')
      for (const t of targets)
        addVault({
          id: `thief:${t.outpoint}`,
          role: 'thief',
          params: t.params,
          outpoint: t.outpoint,
          owner: t.owner,
          hotSecret: secret,
          createdAt: Date.now(),
        })
      kitText = ''
    } catch (err) {
      kitError = (err as Error).message
    }
  }
</script>

<h1>Vault</h1>
<p class="intro">Una caja con temporizador entre tus dos carteras. Si te roban el móvil, tienes tiempo de reaccionar.</p>

<!-- Left: hot wallet. Right: safe box. Vaults sit between them. -->
<div class="split">
  <section class="half hot">
    <div class="icon">🔥</div>
    <h2>Cartera caliente</h2>
    <p class="hint">La del móvil. La usas a diario… y te la pueden robar.</p>
    <div class="bal">{balances.loaded ? sats(balances.main) : '…'}</div>
    {#if balances.loaded && balances.main < 10_000}
      <button class="hot" onclick={askFaucet}>Pedir sats al faucet</button>
    {:else}
      <form onsubmit={create}>
        <label>
          Guardar en un vault (sats)
          <input type="number" bind:value={amount} min="10000" max={Math.max(10_000, balances.main - 2_000)} step="1000" required />
        </label>
        <label>
          Espera para volver aquí (bloques)
          <input type="number" bind:value={delay} min="1" max="200" required />
          <span class="hint">{blocksToTime(delay, chain.miner?.interval_s)}</span>
        </label>
        <button type="submit" class="hot" disabled={busy}>Crear vault</button>
      </form>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </section>

  <section class="half cold">
    <div class="icon">🧊</div>
    <h2>Caja fuerte</h2>
    <p class="hint">La llave está en un papel guardado en casa. Casi nunca se toca.</p>
    <div class="bal">{balances.loaded ? sats(balances.cold) : '…'}</div>
    <p class="hint">Desde un vault, el dinero puede venir aquí <b>al instante</b>: es el botón del pánico.</p>
  </section>
</div>

{#each ownVaults as v (v.id)}
  <VaultCard {v} />
{/each}

<section class="thief">
  <div class="icon">🦹</div>
  <h2>Ladrona</h2>
  <p class="hint">
    ¿Te han pasado la llave caliente de otra persona? Pégala aquí e intenta llevarte su vault.
    Lo robado llegará a tu cartera caliente.
  </p>
  <form onsubmit={importKit}>
    <input bind:value={kitText} placeholder="La llave caliente robada (64 caracteres)" autocomplete="off" spellcheck="false" />
    <button type="submit" class="thief-btn" disabled={!kitText.trim()}>Usar la llave robada</button>
  </form>
  {#if kitError}<p class="error">{kitError}</p>{/if}
  {#each stolen as v (v.id)}
    <VaultCard {v} />
  {/each}
</section>

<style>
  .intro { margin-top: 0; }
  .split { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin: 1rem 0; }
  .half, .thief {
    border-radius: 16px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .half.hot { background: var(--hot-bg); border: 2px solid var(--hot); }
  .half.cold { background: var(--cold-bg); border: 2px solid var(--cold); }
  .thief { background: var(--thief-bg); border: 2px dashed var(--thief); margin-top: 2rem; }
  h2 { margin: 0; }
  .icon { font-size: 2rem; }
  .hint { font-size: 0.85rem; color: var(--muted); margin: 0; }
  .bal { font-size: 1.6rem; font-weight: 700; font-variant-numeric: tabular-nums; }
  form { display: flex; flex-direction: column; gap: 0.6rem; }
  label { display: flex; flex-direction: column; gap: 0.25rem; font-weight: 600; font-size: 0.9rem; }
  .thief-btn { background: var(--thief); color: #fff; }
  :global(article) { margin: 1rem 0; }
  @media (max-width: 40rem) {
    .split { grid-template-columns: 1fr; }
  }
</style>
