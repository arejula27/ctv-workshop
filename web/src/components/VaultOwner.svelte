<script lang="ts">
  import { newSeed, vault as core, walletInfo, walletSecret, walletSend, type VaultParams } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { listHotWallets, registerHotWallet, registerVault, type HotWallet } from '../lib/api'
  import { friendly } from '../lib/act'
  import { history, type HistoryEntry } from '../lib/history'
  import { addVault, chain, markMine, me, saveVaultGame, vaultGame, vaults } from '../lib/store.svelte'
  import { blocksToTime, sats, short, simDuration } from '../lib/format'
  import WalletPanel from './WalletPanel.svelte'
  import VaultCard from './VaultCard.svelte'
  import Modal from './Modal.svelte'
  import TxHistory from './TxHistory.svelte'

  const FEE_SAT = 1_000

  let wallets = $state<HotWallet[]>([])
  let safeChoice = $state('new')
  let hotBalance = $state(0)
  let hotPanel = $state<{ reload: () => Promise<void> }>()
  let moving = $state(false)
  let amount = $state<number | null>(null)
  let delay = $state(10)
  let busy = $state(false)
  let error = $state('')
  let friendHistory = $state<HistoryEntry[]>([])

  let hot = $derived(vaultGame.hot ? walletInfo(vaultGame.hot.seed) : null)
  let safe = $derived(vaultGame.safe)
  let safePubkey = $derived(safe?.kind === 'own' ? walletInfo(safe.seed).main_pubkey : safe?.pubkey)
  let safeAddress = $derived(safe?.kind === 'own' ? walletInfo(safe.seed).main_address : safe?.address)
  // Only vaults guarded by the current hot wallet.
  let myVaults = $derived(vaults.list.filter((v) => v.role === 'owner' && v.params.hot_pubkey === hot?.main_pubkey))

  $effect(() => {
    void chain.walletsTick
    listHotWallets().then((w) => (wallets = w)).catch(() => {})
  })
  // Friends who already have a hot wallet (latest one each), to use as a safe box.
  let friends = $derived.by(() => {
    const byOwner = new Map<string, HotWallet>()
    for (const w of wallets) if (w.address !== hot?.main_address && w.owner !== me.id?.name) byOwner.set(w.owner, w)
    return [...byOwner.values()]
  })

  // A friend's safe box isn't ours to spend, but we can watch what lands there.
  $effect(() => {
    void chain.tick
    if (safe?.kind === 'friend') history([safe.address]).then((h) => (friendHistory = h)).catch(() => {})
  })

  function labelOf(address: string): string | undefined {
    if (address === hot?.main_address) return 'tu cartera caliente'
    if (address === safeAddress) return safe?.kind === 'friend' ? `cartera de ${safe.owner}` : 'tu caja fuerte'
    for (const v of myVaults) {
      const view = core.view(v.params, v.outpoint)
      if (address === view.address || address === view.leaves[0].tx?.outputs[0].contract?.address) return 'vault'
    }
    const w = wallets.find((w) => w.address === address)
    return w ? `cartera de ${w.owner}` : undefined
  }

  async function createHot() {
    const seed = newSeed()
    saveVaultGame({ hot: { seed } })
    // Public on purpose: it's what the thieves play with.
    registerHotWallet(me.id!.name, seed).catch((e) => (error = friendly(e)))
  }

  function chooseSafe() {
    if (safeChoice === 'new') saveVaultGame({ safe: { kind: 'own', seed: newSeed() } })
    else {
      const w = friends.find((f) => f.address === safeChoice)!
      saveVaultGame({ safe: { kind: 'friend', owner: w.owner, address: w.address, pubkey: w.pubkey } })
    }
  }

  async function moveToVault(e: SubmitEvent) {
    e.preventDefault()
    if (!hot || !safePubkey || amount == null) return
    busy = true
    error = ''
    try {
      const params: VaultParams = { amount_sat: amount, hot_pubkey: hot.main_pubkey, cold_pubkey: safePubkey, delay, fee_sat: FEE_SAT }
      const tx = walletSend(vaultGame.hot!.seed, await esplora.utxos(hot.main_address), core.address(params), amount)
      await esplora.broadcast(tx.hex)
      markMine(tx.txid)
      const outpoint = `${tx.txid}:0` // wallet_send puts the payment first
      addVault({ id: tx.txid, role: 'owner', params, outpoint, createdAt: Date.now() })
      registerVault(me.id!.name, params, outpoint).catch(() => {})
      moving = false
      amount = null
      await hotPanel?.reload()
    } catch (err) {
      error = friendly(err)
    } finally {
      busy = false
    }
  }
</script>

<div class="grid">
  {#if hot && vaultGame.hot}
    <WalletPanel bind:this={hotPanel} bind:balance={hotBalance} title="Cartera caliente" subtitle="Tu día a día. La llave vive en tu móvil." seed={vaultGame.hot.seed} tone="hot" {labelOf}>
      {#snippet actions()}
        <button onclick={() => (moving = true)} disabled={!safe || hotBalance === 0} title={safe ? '' : 'Configura antes la caja fuerte'}>Mover al vault</button>
      {/snippet}
    </WalletPanel>
  {:else}
    <section class="panel hot setup">
      <div class="eyebrow"><span class="dot hot"></span>Cartera caliente</div>
      <h2>Tu cartera del día a día</h2>
      <p class="muted">La que llevas en el móvil. Cómoda, pero si te la roban, se llevan su llave.</p>
      <button onclick={createHot}>Crear cartera</button>
    </section>
  {/if}

  {#if safe?.kind === 'own'}
    <WalletPanel title="Caja fuerte" subtitle="La llave está en un papel, guardada en casa." seed={safe.seed} tone="cold" {labelOf} />
  {:else if safe?.kind === 'friend'}
    <section class="panel cold">
      <div class="eyebrow"><span class="dot cold"></span>Caja fuerte</div>
      <h2>La cartera de {safe.owner}</h2>
      <p class="muted">Una persona de confianza. Si pulsas el pánico, el dinero irá a su cartera.</p>
      <span class="mono addr">{short(safe.address, 10)}</span>
      <div class="history">
        <div class="eyebrow">Lo que ha llegado</div>
        <TxHistory entries={friendHistory} {labelOf} />
      </div>
    </section>
  {:else}
    <section class="panel cold setup">
      <div class="eyebrow"><span class="dot cold"></span>Caja fuerte</div>
      <h2>¿A dónde va el dinero en una emergencia?</h2>
      <p class="muted">Una llave que casi nunca se usa: en papel, en casa, o en manos de alguien de confianza.</p>
      <label>
        Caja fuerte
        <select bind:value={safeChoice}>
          <option value="new">Crear una caja fuerte nueva</option>
          {#each friends as f (f.address)}<option value={f.address}>La cartera de {f.owner}</option>{/each}
        </select>
      </label>
      <button class="secondary" onclick={chooseSafe}>Configurar caja fuerte</button>
    </section>
  {/if}
</div>

{#if error}<p class="error">{error}</p>{/if}

{#if myVaults.length}
  <h2 class="section">Tus vaults</h2>
  {#each myVaults as v (v.id)}
    <VaultCard params={v.params} outpoint={v.outpoint} role="owner" secret={walletSecret(vaultGame.hot!.seed)} destination={hot!.main_address} />
  {/each}
{/if}

{#if moving}
  <Modal title="Mover al vault" onclose={() => (moving = false)}>
    <p class="muted">Desde un vault, el dinero solo puede volver a tu cartera caliente <b>después de una espera</b>, o ir <b>al instante</b> a tu caja fuerte.</p>
    <form onsubmit={moveToVault}>
      <label>
        Cantidad (sats)
        <input type="number" bind:value={amount} min="10000" max={Math.max(10_000, hotBalance - 2_000)} required />
      </label>
      <label>
        Espera para volver a la cartera caliente (bloques)
        <input type="number" bind:value={delay} min="1" max="200" required />
        <span class="muted hint">= {simDuration(delay, chain.simBlock)} en Bitcoin real · {blocksToTime(delay, chain.miner?.interval_s)} hoy en la sala</span>
      </label>
      {#if error}<p class="error">{error}</p>{/if}
      <button type="submit" disabled={busy}>{busy ? 'Moviendo…' : `Mover${amount ? ` ${sats(amount)}` : ''} al vault`}</button>
    </form>
  </Modal>
{/if}

<style>
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; align-items: start; }
  .setup { display: flex; flex-direction: column; gap: 0.6rem; align-items: flex-start; }
  .setup p { margin: 0; font-size: 0.9rem; }
  .setup label { width: 100%; }
  .addr { color: var(--muted); }
  .history { margin-top: 0.6rem; width: 100%; }
  .section { margin: 1.75rem 0 0.75rem; }
  :global(.card) { margin-bottom: 1rem; }
  form { display: flex; flex-direction: column; gap: 0.75rem; }
  .hint { font-weight: 400; font-size: 0.8rem; }
  @media (max-width: 44rem) { .grid { grid-template-columns: 1fr; } }
</style>
