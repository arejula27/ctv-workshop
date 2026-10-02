<script lang="ts">
  import { onMount } from 'svelte'
  import { newSeed, walletInfo, walletSecret, walletSweep } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { listHotWallets, listVaults, reportActivity, type HotWallet, type VaultEntry } from '../lib/api'
  import { friendly } from '../lib/act'
  import { chain, markMine, me, saveVaultGame, vaultGame } from '../lib/store.svelte'
  import { sats, short } from '../lib/format'
  import WalletPanel from './WalletPanel.svelte'
  import VaultCard from './VaultCard.svelte'

  let wallets = $state<HotWallet[]>([])
  let vaultList = $state<VaultEntry[]>([])
  let balances = $state<Record<string, number>>({})
  let busy = $state<string | null>(null)
  let errors = $state<Record<string, string>>({})
  let revealed = $state<Record<string, boolean>>({})

  let mine = $derived(vaultGame.thief ? walletInfo(vaultGame.thief.seed) : null)
  // Everyone's leaked hot wallets except our own.
  let victims = $derived(wallets.filter((w) => w.owner !== me.id?.name))

  $effect(() => {
    void chain.walletsTick
    listHotWallets().then((w) => (wallets = w)).catch(() => {})
  })
  $effect(() => {
    void chain.vaultsTick
    listVaults().then((v) => (vaultList = v)).catch(() => {})
  })

  async function refreshBalances() {
    const entries = await Promise.all(
      victims.map(async (w) => [w.address, (await esplora.utxos(w.address)).reduce((a, u) => a + u.value, 0)] as const),
    ).catch(() => [])
    balances = Object.fromEntries(entries)
  }
  onMount(() => {
    const t = setInterval(refreshBalances, 5000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    void victims.length
    refreshBalances()
  })

  async function sweep(w: HotWallet) {
    if (!mine) return
    busy = w.address
    errors[w.address] = ''
    try {
      const amount = balances[w.address] ?? 0
      const tx = walletSweep(w.seed, await esplora.utxos(w.address), mine.main_address)
      await esplora.broadcast(tx.hex)
      markMine(tx.txid)
      reportActivity(me.id!.name, 'wallet_swept', w.address, tx.txid, amount).catch(() => {})
      await refreshBalances()
    } catch (e) {
      errors[w.address] = friendly(e)
    } finally {
      busy = null
    }
  }
</script>

<div class="grid">
  {#if mine && vaultGame.thief}
    <WalletPanel title="Tu cartera" subtitle="Aquí llega lo que robes." seed={vaultGame.thief.seed} tone="thief"
      labelOf={(a) => { const w = wallets.find((w) => w.address === a); return w ? `cartera de ${w.owner}` : undefined }} />
  {:else}
    <section class="panel thief setup">
      <div class="eyebrow"><span class="dot thief"></span>Tu cartera</div>
      <h2>Primero, dónde guardar el botín</h2>
      <p class="muted">Una cartera tuya para recibir lo que robes.</p>
      <button onclick={() => saveVaultGame({ thief: { seed: newSeed() } })}>Crear cartera</button>
    </section>
  {/if}

  <section class="panel thief leaks">
    <div class="eyebrow"><span class="dot thief"></span>Semillas filtradas</div>
    <h2>Carteras calientes de la sala</h2>
    <p class="muted">Alguien ha filtrado las semillas de todas las carteras calientes. Con ellas puedes vaciar la cartera… y atacar sus vaults.</p>
  </section>
</div>

{#if victims.length === 0}
  <p class="muted">Todavía nadie ha creado su cartera caliente.</p>
{/if}

{#each victims as w (w.address)}
  {@const theirVaults = vaultList.filter((v) => v.params.hot_pubkey === w.pubkey)}
  <section class="panel victim">
    <div class="row">
      <div>
        <div class="eyebrow">Cartera caliente de {w.owner}</div>
        <div class="value">{sats(balances[w.address] ?? 0)}</div>
        <span class="mono muted">{short(w.address, 10)}</span>
      </div>
      <button class="danger" onclick={() => sweep(w)} disabled={!mine || busy === w.address || !(balances[w.address] > 0)}>
        {busy === w.address ? 'Vaciando…' : 'Vaciar cartera'}
      </button>
    </div>
    <div class="seed">
      <span class="eyebrow">Semilla</span>
      {#if revealed[w.address]}<code>{w.seed}</code>{:else}<button class="ghost" onclick={() => (revealed[w.address] = true)}>Mostrar</button>{/if}
    </div>
    {#if errors[w.address]}<p class="error">{errors[w.address]}</p>{/if}
    {#if !mine}<p class="muted small">Crea tu cartera para poder robar.</p>{/if}

    {#each theirVaults as v (v.outpoint)}
      <VaultCard params={v.params} outpoint={v.outpoint} role="thief" owner={w.owner} secret={walletSecret(w.seed)} destination={mine?.main_address ?? ''} />
    {/each}
  </section>
{/each}

<style>
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; align-items: start; margin-bottom: 1rem; }
  .setup, .leaks { display: flex; flex-direction: column; gap: 0.6rem; align-items: flex-start; }
  .setup p, .leaks p { margin: 0; font-size: 0.9rem; }
  .victim { display: flex; flex-direction: column; gap: 0.75rem; margin-bottom: 1rem; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 1rem; flex-wrap: wrap; }
  .value { font-size: 1.3rem; font-weight: 650; font-variant-numeric: tabular-nums; }
  .seed { display: flex; gap: 0.6rem; align-items: center; background: var(--card); border-radius: 8px; padding: 0.4rem 0.6rem; }
  .small { font-size: 0.85rem; margin: 0; }
  @media (max-width: 44rem) { .grid { grid-template-columns: 1fr; } }
</style>
