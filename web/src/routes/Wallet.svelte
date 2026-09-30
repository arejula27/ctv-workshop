<script lang="ts">
  import { faucet } from '../lib/api'
  import { balances, chain, me, refreshBalances, setIdentity } from '../lib/store.svelte'
  import { sats } from '../lib/format'
  import { navigate } from '../lib/router.svelte'

  let busy = $state(false)
  let msg = $state('')
  let error = $state('')
  let showSeed = $state(false)

  async function askFaucet() {
    if (!me.wallet) return
    busy = true
    error = msg = ''
    try {
      const r = await faucet(me.wallet.main_address)
      msg = `¡Recibidos ${sats(r.amount_sat)}! Llegarán en el próximo bloque.`
      await refreshBalances()
    } catch (e) {
      error = (e as Error).message
    } finally {
      busy = false
    }
  }

  function copy(text: string) {
    navigator.clipboard?.writeText(text)
  }

  function logout() {
    if (confirm('¿Seguro? Si no has guardado la semilla, perderás esta cartera.')) setIdentity(null)
  }
</script>

<h1>Hola, {me.id?.name}</h1>

<section class="wallets">
  <div class="wallet hot">
    <div class="icon">🔥</div>
    <h2>Cartera caliente</h2>
    <p class="muted">La del móvil. La usas a diario.</p>
    <div class="balance">{balances.loaded ? sats(balances.main) : '…'}</div>
    <button onclick={askFaucet} disabled={busy}>Pedir sats al faucet</button>
  </div>
  <div class="wallet cold">
    <div class="icon">🧊</div>
    <h2>Caja fuerte</h2>
    <p class="muted">Llave guardada en papel. Casi nunca se toca.</p>
    <div class="balance">{balances.loaded ? sats(balances.cold) : '…'}</div>
    <button class="secondary" onclick={() => navigate('/vault')}>Proteger con un vault →</button>
  </div>
</section>

{#if msg}<p>{msg}</p>{/if}
{#if error}<p class="error">{error}</p>{/if}
{#if balances.pending > 0}
  <p class="muted">{sats(balances.pending)} pendientes de confirmar (bloque {chain.height + 1}).</p>
{/if}

<details>
  <summary>Detalles técnicos</summary>
  <p>
    Dirección de la cartera caliente:<br /><code>{me.wallet?.main_address}</code>
    <button class="secondary" onclick={() => copy(me.wallet?.main_address ?? '')}>Copiar</button>
  </p>
  <p>Dirección de la caja fuerte:<br /><code>{me.wallet?.cold_address}</code></p>
  <p>
    <button class="secondary" onclick={() => (showSeed = !showSeed)}>
      {showSeed ? 'Ocultar' : 'Ver'} semilla
    </button>
    {#if showSeed}<br /><code>{me.id?.seed}</code>{/if}
  </p>
  <button class="secondary" onclick={logout}>Salir de esta cartera</button>
</details>

<style>
  .wallets { display: grid; grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr)); gap: 1rem; margin: 1rem 0; }
  .wallet { border-radius: 16px; padding: 1rem; display: flex; flex-direction: column; gap: 0.5rem; }
  .wallet.hot { background: var(--hot-bg); border: 2px solid var(--hot); }
  .wallet.cold { background: var(--cold-bg); border: 2px solid var(--cold); }
  .wallet h2, .wallet p { margin: 0; }
  .icon { font-size: 2rem; }
  .balance { font-size: 1.8rem; font-weight: 700; font-variant-numeric: tabular-nums; }
  details { margin-top: 2rem; }
  details p { display: flex; flex-direction: column; gap: 0.3rem; align-items: flex-start; }
</style>
