<script lang="ts">
  // A wallet that feels like a real one: balance, receive, send, history.
  import { onMount, type Snippet } from 'svelte'
  import { walletInfo, walletSend, walletSweep } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { history, type HistoryEntry } from '../lib/history'
  import { friendly } from '../lib/act'
  import { chain, markMine } from '../lib/store.svelte'
  import { sats, short } from '../lib/format'
  import Modal from './Modal.svelte'
  import TxHistory from './TxHistory.svelte'

  let {
    title,
    subtitle = '',
    seed,
    tone,
    balance = $bindable(0),
    actions,
    labelOf = () => undefined,
  }: {
    title: string
    subtitle?: string
    seed: string
    tone?: 'hot' | 'cold' | 'thief'
    balance?: number
    actions?: Snippet
    labelOf?: (address: string) => string | undefined
  } = $props()

  let address = $derived(walletInfo(seed).main_address)
  let pending = $state(0)
  let entries = $state<HistoryEntry[]>([])
  let modal = $state<'receive' | 'send' | null>(null)
  let to = $state('')
  let amount = $state<number | null>(null)
  let busy = $state(false)
  let error = $state('')
  let copied = $state(false)

  async function refresh() {
    try {
      const [utxos, h] = await Promise.all([esplora.utxos(address), history([address])])
      balance = utxos.reduce((a, u) => a + u.value, 0)
      pending = utxos.filter((u) => !u.status.confirmed).reduce((a, u) => a + u.value, 0)
      entries = h
    } catch {
      // the next tick retries
    }
  }
  onMount(() => {
    const t = setInterval(refresh, 4000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    void address
    refresh()
  })

  function copy() {
    navigator.clipboard?.writeText(address)
    copied = true
    setTimeout(() => (copied = false), 1500)
  }

  async function send(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = ''
    try {
      const utxos = await esplora.utxos(address)
      const tx = amount != null && amount >= balance ? walletSweep(seed, utxos, to.trim()) : walletSend(seed, utxos, to.trim(), amount ?? 0)
      await esplora.broadcast(tx.hex)
      markMine(tx.txid)
      modal = null
      to = ''
      amount = null
      await refresh()
    } catch (err) {
      error = friendly(err)
    } finally {
      busy = false
    }
  }

  /** Lets the parent refresh after it spends from this wallet (e.g. into a vault). */
  export function reload() {
    return refresh()
  }
</script>

<section class="panel wallet {tone ?? ''}">
  <div class="top">
    <div>
      <div class="eyebrow"><span class="dot {tone ?? ''}"></span>{title}</div>
      {#if subtitle}<div class="muted sub">{subtitle}</div>{/if}
    </div>
    <span class="mono addr" title={address}>{short(address, 6)}</span>
  </div>

  <div class="amount">{sats(balance)}</div>
  {#if pending > 0}<div class="muted sub">{sats(pending)} pendientes de confirmar</div>{/if}

  <div class="actions">
    <button class="secondary" onclick={() => (modal = 'receive')}>Recibir</button>
    <button class="secondary" onclick={() => (modal = 'send')} disabled={balance === 0}>Enviar</button>
    {@render actions?.()}
  </div>

  <div class="history">
    <div class="eyebrow">Movimientos</div>
    <TxHistory {entries} {labelOf} />
  </div>
</section>

{#if modal === 'receive'}
  <Modal title="Recibir" onclose={() => (modal = null)}>
    <p class="muted">Comparte esta dirección para recibir bitcoin en <b>{title.toLowerCase()}</b>.</p>
    <div class="address mono">{address}</div>
    <button onclick={copy}>{copied ? 'Copiada ✓' : 'Copiar dirección'}</button>
    <p class="muted hint">¿Necesitas fondos de prueba? Pega la dirección en el <a href="/faucet" target="_blank">faucet</a>.</p>
  </Modal>
{:else if modal === 'send'}
  <Modal title="Enviar" onclose={() => (modal = null)}>
    <form onsubmit={send}>
      <label>Dirección de destino <input bind:value={to} placeholder="bcrt1…" required autocomplete="off" spellcheck="false" /></label>
      <label>
        Cantidad (sats)
        <div class="row">
          <input type="number" bind:value={amount} min="330" max={balance} required />
          <button type="button" class="secondary" onclick={() => (amount = balance)}>Todo</button>
        </div>
      </label>
      {#if error}<p class="error">{error}</p>{/if}
      <button type="submit" disabled={busy}>{busy ? 'Enviando…' : 'Enviar'}</button>
    </form>
  </Modal>
{/if}

<style>
  .wallet { display: flex; flex-direction: column; gap: 0.6rem; }
  .top { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; }
  .sub { font-size: 0.82rem; }
  .addr { color: var(--muted); background: var(--card); padding: 0.15rem 0.45rem; border-radius: 6px; white-space: nowrap; }
  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .history { margin-top: 0.6rem; }
  .address { background: var(--card); border-radius: 8px; padding: 0.75rem; font-size: 0.9rem; }
  .hint { font-size: 0.85rem; margin: 0; }
  form { display: flex; flex-direction: column; gap: 0.75rem; }
  .row { display: flex; gap: 0.5rem; }
</style>
