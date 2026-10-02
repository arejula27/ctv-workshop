<script lang="ts">
  // The "instant" account: your balance inside the pool. Paying someone in the
  // pool or sending out needs everyone's signature (their apps sign by
  // themselves); leaving needs nobody's.
  import { pool as core, walletInfo, type PoolParams } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { cancelPool, proposePool, reportPoolExit, signPool, type PoolTable } from '../lib/api'
  import { friendly } from '../lib/act'
  import { txUrl } from '../lib/explorer'
  import { markMine, poolGame } from '../lib/store.svelte'
  import { sats, short } from '../lib/format'
  import Modal from './Modal.svelte'

  let { table, onchange }: { table: PoolTable; onchange: () => void } = $props()

  let me = $derived(walletInfo(poolGame.seed!))
  let mine = $derived(table.members.find((m) => m.pubkey === me.main_pubkey))
  let params: PoolParams = $derived({ members: table.members, fee_sat: table.fee_sat })
  let pending = $derived(table.pending)
  let signed = $derived(pending ? Object.keys(pending.sigs).length : 0)
  let modal = $state<'send' | 'receive' | 'exit' | null>(null)
  let target = $state('')
  let address = $state('')
  let amount = $state<number | null>(null)
  let busy = $state(false)
  let error = $state('')
  let signError = $state('')

  // Sign pending updates by ourselves, after checking them.
  let signing = ''
  $effect(() => {
    const p = pending
    if (!p || !mine || p.sigs[me.main_pubkey] || signing === p.txid) return
    signing = p.txid
    try {
      // Rebuild the very same transaction from the proposal; never sign
      // something that takes our money unless we started it.
      const next = p.next.find((m) => m.pubkey === me.main_pubkey)
      if (!next || (next.balance_sat < mine.balance_sat && p.by !== mine.label)) throw new Error('Esta propuesta te quita saldo: no la firmo.')
      const sig = core.updateSign({ now: params, next: p.next, outpoint: table.outpoint!, utxos: p.deposit_utxos, outputs: p.outputs }, poolGame.seed!)
      if (sig.txid !== p.txid) throw new Error('La transacción propuesta no coincide con la que calcula tu app.')
      signPool(table.id, me.main_pubkey, sig).then(onchange).catch((e) => (signError = friendly(e)))
    } catch (e) {
      signError = friendly(e)
    }
  })

  async function run(fn: () => Promise<unknown>) {
    busy = true
    error = ''
    try {
      await fn()
      modal = null
      amount = null
      onchange()
    } catch (e) {
      error = friendly(e)
    } finally {
      busy = false
    }
  }

  function send(e: SubmitEvent) {
    e.preventDefault()
    run(() =>
      target === 'external'
        ? proposePool(table.id, { by: me.main_pubkey, op: 'withdraw', amount_sat: amount!, address: address.trim() })
        : proposePool(table.id, { by: me.main_pubkey, op: 'pay', amount_sat: amount!, to: target }),
    )
  }

  const leave = () =>
    run(async () => {
      const who = table.members.findIndex((m) => m.pubkey === me.main_pubkey)
      const tx = core.exit(params, who, table.outpoint!, poolGame.seed!)
      await esplora.broadcast(tx.hex)
      markMine(tx.txid)
      await reportPoolExit(table.id, me.main_pubkey, tx.txid)
    })

  const others = $derived(table.members.filter((m) => m.pubkey !== me.main_pubkey))
  const describe: Record<string, (s: PoolTable['history'][number]) => string> = {
    funded: (s) => `Pool creado con ${sats(s.amount_sat)}`,
    pay: (s) => `${s.by} paga ${sats(s.amount_sat)} a ${s.to}`,
    withdraw: (s) => `${s.by} envía ${sats(s.amount_sat)} fuera del pool`,
    deposit: (s) => `${s.by} mete ${sats(s.amount_sat)}`,
    exit: (s) => `${s.by} sale con ${sats(s.amount_sat)}`,
  }
</script>

<section class="panel cold instant">
  <div class="top">
    <div>
      <div class="eyebrow"><span class="dot cold"></span>Cuenta instantánea · pool «{table.name}»</div>
      <div class="muted sub">Tu saldo dentro del UTXO compartido</div>
    </div>
    {#if table.outpoint}
      <a class="mono addr" href={txUrl(table.outpoint.split(':')[0])} target="_blank" rel="noopener" title="El UTXO del pool en el explorador">UTXO {short(table.outpoint.split(':')[0], 5)} ↗</a>
    {/if}
  </div>

  <div class="amount">{sats(mine?.balance_sat ?? 0)}</div>

  {#if mine}
    <div class="actions">
      <button class="secondary" onclick={() => (modal = 'receive')}>Recibir</button>
      <button class="secondary" onclick={() => { target = others[0]?.pubkey ?? 'external'; modal = 'send' }} disabled={!!pending}>Enviar</button>
      <button class="secondary" onclick={() => (modal = 'exit')} disabled={!!pending}>Salir del pool</button>
    </div>
  {:else}
    <p class="muted">Ya no estás en este pool.</p>
  {/if}

  {#if pending}
    <div class="pending">
      <div>
        <b>{pending.op === 'pay' ? `${pending.by} paga ${sats(pending.amount_sat)} a ${pending.to}` : pending.op === 'withdraw' ? `${pending.by} envía ${sats(pending.amount_sat)} fuera` : `${pending.by} mete ${sats(pending.amount_sat)}`}</b>
        <div class="muted small">Cambiar el reparto necesita la firma de todos · {signed}/{table.members.length} firmas</div>
      </div>
      {#if pending.by === mine?.label}<button class="ghost" onclick={() => run(() => cancelPool(table.id))}>Cancelar</button>{/if}
    </div>
  {/if}
  {#if signError}<p class="error">{signError}</p>{/if}

  <div class="members">
    <div class="eyebrow">Reparto actual</div>
    {#each table.members as m (m.pubkey)}
      <div class="member" class:me={m.pubkey === me.main_pubkey}>
        <span>{m.label}{m.pubkey === me.main_pubkey ? ' (tú)' : ''}</span>
        <span class="mono">{sats(m.balance_sat)}</span>
      </div>
    {/each}
  </div>

  <div class="history">
    <div class="eyebrow">Estados del pool</div>
    <ul>
      {#each [...table.history].reverse() as s (s.txid + s.kind + s.by)}
        <li>
          <span>{describe[s.kind]?.(s) ?? s.kind}</span>
          <a href={txUrl(s.txid)} target="_blank" rel="noopener" title="Ver en el explorador">↗</a>
        </li>
      {/each}
    </ul>
  </div>
</section>

{#if modal === 'receive'}
  <Modal title="Recibir en el pool" onclose={() => (modal = null)}>
    <p>Hay dos maneras de que suba tu saldo instantáneo:</p>
    <ul class="muted">
      <li>Que alguien de tu mesa te pague <b>dentro del pool</b>: le basta con elegir tu nombre en «Enviar».</li>
      <li>Meter dinero desde tu <b>cuenta on-chain</b> con «Meter en el pool».</li>
    </ul>
  </Modal>
{:else if modal === 'send'}
  <Modal title="Enviar desde el pool" onclose={() => (modal = null)}>
    <form onsubmit={send}>
      <label>
        A quién
        <select bind:value={target}>
          {#each others as m (m.pubkey)}<option value={m.pubkey}>{m.label} (dentro del pool)</option>{/each}
          <option value="external">Una dirección on-chain…</option>
        </select>
      </label>
      {#if target === 'external'}
        <label>Dirección <input bind:value={address} placeholder="bcrt1…" required autocomplete="off" spellcheck="false" /></label>
      {/if}
      <label>Cantidad (sats) <input type="number" bind:value={amount} min="330" required /></label>
      <p class="muted small">Todos los de la mesa tienen que firmar el nuevo reparto. Sus apps lo hacen solas en un momento. Comisión: {sats(table.fee_sat)}.</p>
      {#if error}<p class="error">{error}</p>{/if}
      <button type="submit" disabled={busy}>{busy ? 'Proponiendo…' : 'Enviar'}</button>
    </form>
  </Modal>
{:else if modal === 'exit'}
  <Modal title="Salir del pool" onclose={() => (modal = null)}>
    <p>Te llevas <b>{sats((mine?.balance_sat ?? 0) - table.fee_sat)}</b> a tu cuenta on-chain: tu saldo actual menos la comisión.</p>
    <p class="muted">No necesitas la firma de nadie: la salida estaba fijada de antemano con CTV. El resto sigue dentro, en un pool nuevo.</p>
    {#if error}<p class="error">{error}</p>{/if}
    <button onclick={leave} disabled={busy}>{busy ? 'Saliendo…' : 'Salir del pool'}</button>
  </Modal>
{/if}

<style>
  .instant { display: flex; flex-direction: column; gap: 0.6rem; }
  .top { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; }
  .sub, .small { font-size: 0.82rem; }
  .addr { color: var(--muted); background: var(--card); padding: 0.15rem 0.45rem; border-radius: 6px; text-decoration: none; white-space: nowrap; }
  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .pending { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; background: var(--cold-bg); border-radius: 8px; padding: 0.6rem 0.75rem; }
  .members { display: flex; flex-direction: column; gap: 0.2rem; margin-top: 0.4rem; }
  .member { display: flex; justify-content: space-between; font-size: 0.9rem; padding: 0.3rem 0; border-top: 1px solid var(--line); }
  .member.me { font-weight: 650; }
  .history ul { list-style: none; margin: 0.3rem 0 0; padding: 0; }
  .history li { display: flex; justify-content: space-between; font-size: 0.88rem; padding: 0.35rem 0; border-top: 1px solid var(--line); }
  .history a { text-decoration: none; color: var(--muted); font-weight: 600; }
  form { display: flex; flex-direction: column; gap: 0.75rem; }
</style>
