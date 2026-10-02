<script lang="ts">
  // A stand-alone faucet, like the ones testnets have: paste an address, get coins.
  import { faucet } from '../lib/api'
  import { txUrl } from '../lib/explorer'
  import { chain } from '../lib/store.svelte'
  import { sats, short } from '../lib/format'

  let address = $state('')
  let busy = $state(false)
  let error = $state('')
  let sent = $state<{ txid: string; amount: number; to: string } | null>(null)

  async function ask(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = ''
    sent = null
    try {
      const r = await faucet(address.trim())
      sent = { txid: r.txid, amount: r.amount_sat, to: address.trim() }
      address = ''
    } catch (err) {
      const msg = (err as Error).message
      error = msg.startsWith('wait')
        ? `Ya has pedido hace poco. Vuelve a intentarlo en ${msg.match(/\d+/)?.[0] ?? 'unos'} segundos.`
        : msg.includes('address')
          ? 'Esa dirección no es válida en esta red.'
          : msg
    } finally {
      busy = false
    }
  }
</script>

<div class="page">
  <section class="panel">
    <div class="eyebrow">Regtest del workshop</div>
    <h1>Faucet</h1>
    <p class="muted">Bitcoin de prueba, sin valor. Pega una dirección de tu cartera (botón <b>Recibir</b>) y te enviamos fondos.</p>
    <form onsubmit={ask}>
      <label>Dirección <input bind:value={address} placeholder="bcrt1p…" required autocomplete="off" spellcheck="false" /></label>
      <button type="submit" disabled={busy || !address.trim()}>{busy ? 'Enviando…' : 'Enviarme bitcoin de prueba'}</button>
    </form>
    {#if error}<p class="error">{error}</p>{/if}
    {#if sent}
      <div class="ok">
        <b>Enviados {sats(sent.amount)}</b> a <span class="mono">{short(sent.to, 8)}</span>.
        Llegarán con el próximo bloque{chain.miner && !chain.miner.paused ? ` (~${chain.miner.interval_s} s)` : ''}.
        <a href={txUrl(sent.txid)} target="_blank" rel="noopener">Ver transacción ↗</a>
      </div>
    {/if}
  </section>
</div>

<style>
  .page { max-width: 32rem; margin: 2rem auto; padding: 0 1rem; }
  .panel { display: flex; flex-direction: column; gap: 0.75rem; }
  form { display: flex; flex-direction: column; gap: 0.75rem; }
  .ok { background: var(--ok-bg); border-radius: 8px; padding: 0.75rem; font-size: 0.92rem; }
</style>
