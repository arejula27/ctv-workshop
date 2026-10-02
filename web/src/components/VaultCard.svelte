<script lang="ts">
  import { vault as core, type SignedTx, type VaultParams } from '../lib/core'
  import { broadcastAndReport, friendly } from '../lib/act'
  import type { ActivityKind } from '../lib/api'
  import { chain, isMine, markMine, simDate } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { blocksLeft as blocksLeftOf, unvaultOutpoint, vaultPhase } from '../lib/vaultPhase'
  import { blocksToTime, date, sats, short, simDuration } from '../lib/format'
  import { txUrl } from '../lib/explorer'

  let {
    params,
    outpoint,
    role,
    owner = '',
    secret,
    destination,
  }: {
    params: VaultParams
    outpoint: string
    /** guardian: the vault's safe box is your wallet; you can press panic for them. */
    role: 'owner' | 'thief' | 'guardian'
    /** The vault owner's name (for the thief's view). */
    owner?: string
    /** Secret of the hot key: the owner's own, or a stolen one. */
    secret: string
    /** Where a hot-key withdrawal goes. */
    destination: string
  } = $props()

  let view = $derived(core.view(params, outpoint))
  const st = useStatuses(() => view)
  let phase = $derived(vaultPhase(view, st.map))
  let unvault = $derived(st.map.get(unvaultOutpoint(view)))
  let blocksLeft = $derived(blocksLeftOf(unvault, params.delay, chain.height))
  let isThief = $derived(role === 'thief')
  let isGuardian = $derived(role === 'guardian')
  let hotByMe = $derived(isMine(unvault?.spentBy))
  let busy = $state(false)
  let error = $state('')
  // After an action, keep its buttons off until the chain shows the new phase:
  // pressing twice must not do (or announce) it twice.
  let actedIn = $state<string | null>(null)
  let locked = $derived(busy || actedIn === phase)

  async function act(kind: ActivityKind, build: () => SignedTx) {
    busy = true
    error = ''
    try {
      const tx = build()
      markMine(tx.txid)
      await broadcastAndReport(tx, kind, outpoint)
      actedIn = phase
    } catch (e) {
      error = friendly(e)
    } finally {
      busy = false
    }
  }

  const start = () => act(isThief ? 'theft_start' : 'vault_start', () => core.start(params, outpoint))
  const toCold = () => act('vault_to_cold', () => core.toCold(params, outpoint))
  const panic = () => act('vault_panic', () => core.panic(params, outpoint))
  // Where the hot key sends it: the wallet by default, any address if you like.
  let dest = $state('')
  $effect(() => {
    if (!dest && destination) dest = destination
  })
  const withdrawHot = () =>
    act(isThief ? 'theft_success' : 'vault_withdraw', () => core.withdrawHot(params, outpoint, dest.trim(), secret))

  const ready = $derived(blocksLeft != null && blocksLeft <= 0)
  const ended = $derived(phase === 'cold' || phase === 'panicked' || phase === 'hot')

  // Every step of this vault in the explorer. The CTV script itself shows up in
  // the input of the transaction that spends each contract (its witness).
  let links = $derived.by(() => {
    const out = [{ label: 'Depósito', txid: outpoint.split(':')[0] }]
    const root = st.map.get(outpoint)
    if (root?.spentBy) out.push({ label: root.spentLeaf === 0 ? 'Inicio del retiro' : 'A la caja fuerte', txid: root.spentBy })
    if (unvault?.spentBy)
      out.push({ label: phase === 'hot' ? (hotByMe ? (isThief ? 'Robo' : 'Retiro') : 'Vaciado') : 'Pánico', txid: unvault.spentBy })
    return out
  })
</script>

<article class="panel card" class:thief={isThief} class:alarm={phase === 'withdrawing'}>
  <header>
    <div>
      <div class="eyebrow">{isThief || isGuardian ? `Vault de ${owner}` : 'Vault'}</div>
      {#if ended}
        <div class="value">Vacío</div>
        <div class="muted small">Tenía {sats(params.amount_sat)}</div>
      {:else}
        <div class="value">{sats(params.amount_sat)}</div>
      {/if}
    </div>
    <div class="meta">
      <span class="badge">espera {params.delay} bloques · {simDuration(params.delay, chain.simBlock)}</span>
      {#if phase === 'locked'}<span class="badge ok">Protegido</span>
      {:else if phase === 'withdrawing'}<span class="badge hot">Retiro en curso</span>
      {:else if phase === 'cold' || phase === 'panicked'}<span class="badge cold">En la caja fuerte</span>
      {:else if phase === 'hot'}<span class="badge">{hotByMe ? 'Retirado' : 'Vaciado'}</span>
      {:else}<span class="badge">Confirmando…</span>{/if}
    </div>
  </header>

  <div class="txs">
    <div class="eyebrow">Transacciones</div>
    <ol>
      {#each links as l, i (l.txid)}
        <li>
          <span class="step">{i + 1}</span>
          <span class="what">{l.label}</span>
          <a class="mono" href={txUrl(l.txid)} target="_blank" rel="noopener" title={l.txid}>{short(l.txid, 8)} ↗</a>
        </li>
      {/each}
    </ol>
  </div>

  {#if phase === 'withdrawing'}
    <div class="timer">
      {#if unvault?.funding !== 'confirmed'}
        Retiro iniciado. Se confirma en el próximo bloque.
      {:else if !ready}
        La llave caliente podrá sacarlo en <b>{blocksLeft} bloques</b> — el {date(simDate(chain.height + 1 + (blocksLeft ?? 0)))}
        <span class="muted">({blocksToTime(blocksLeft ?? 0, chain.miner?.interval_s)} en la sala)</span>
      {:else}
        La espera ha terminado: la llave caliente ya puede sacarlo.
      {/if}
    </div>
  {/if}

  {#if phase === 'loading' || phase === 'funding'}
    <p class="muted">Guardando el dinero en el vault…</p>
  {:else}
    <div class="sides">
      <div class="side hot">
        <div class="eyebrow"><span class="dot hot"></span>Con la llave caliente</div>
        {#if isGuardian && (phase === 'locked' || phase === 'withdrawing')}
          <p class="muted">Solo quien tenga la llave caliente de {owner} puede sacarlo por aquí, y tiene que esperar.</p>
        {:else if phase === 'locked'}
          <p>{isThief ? 'Inicia el retiro. Luego hay que esperar, y la dueña lo verá.' : `Primero se inicia el retiro y luego hay que esperar ${params.delay} bloques.`}</p>
          <button class="hot" onclick={start} disabled={locked}>Iniciar retiro</button>
        {:else if phase === 'withdrawing'}
          <p>{isThief ? 'Cuando acabe la espera, llévatelo a donde quieras.' : 'Cuando acabe la espera, puedes retirarlo.'}</p>
          <label class="dest">
            Destino
            <input bind:value={dest} placeholder="bcrt1…" autocomplete="off" spellcheck="false" disabled={locked} />
            <span class="muted hint">{dest.trim() === destination ? (isThief ? 'Tu cartera' : 'Tu cartera caliente') : 'Otra dirección'}</span>
          </label>
          <button class="hot" onclick={withdrawHot} disabled={locked || !ready || !dest.trim()}>{isThief ? 'Llevármelo' : 'Retirar'}</button>
        {:else if phase === 'hot'}
          <p><b>{sats(params.amount_sat - 2 * params.fee_sat)}</b> salieron por aquí{hotByMe ? '.' : isThief ? ', pero no a tu cartera.' : ' sin que fueras tú.'}</p>
        {:else}
          <p class="muted">Camino cerrado.</p>
        {/if}
      </div>
      <div class="side cold">
        <div class="eyebrow"><span class="dot cold"></span>Hacia la caja fuerte</div>
        {#if isGuardian && phase === 'locked'}
          <p>Eres su caja fuerte. Si alguien inicia un retiro, podrás pulsar el pánico por {owner}: el dinero vendría a tu cartera.</p>
        {:else if isGuardian && phase === 'withdrawing'}
          <p><b>Alguien está sacando el vault de {owner}.</b> El pánico no necesita ninguna firma: púlsalo y el dinero vendrá a tu cartera.</p>
          <button class="cold" onclick={panic} disabled={locked}>Botón del pánico</button>
        {:else if isThief && (phase === 'locked' || phase === 'withdrawing')}
          <p class="muted">Fuera de tu alcance. Si {owner} pulsa el pánico, el dinero acaba aquí.</p>
        {:else if phase === 'locked'}
          <p>Al instante, sin esperas.</p>
          <button class="secondary" onclick={toCold} disabled={locked}>Mandar a la caja fuerte</button>
        {:else if phase === 'withdrawing'}
          <p><b>¿No has sido tú?</b> Alguien tiene tu llave caliente. El botón del pánico lo manda todo a la caja fuerte y nadie puede impedirlo.</p>
          <button class="cold" onclick={panic} disabled={locked}>Botón del pánico</button>
        {:else if phase === 'cold' || phase === 'panicked'}
          <p><b>{sats(params.amount_sat - (phase === 'cold' ? 1 : 2) * params.fee_sat)}</b> a salvo{phase === 'panicked' ? ' gracias al pánico' : ''}.</p>
        {:else}
          <p class="muted">Camino cerrado.</p>
        {/if}
      </div>
    </div>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}

</article>

<style>
  .card { display: flex; flex-direction: column; gap: 0.8rem; }
  .small { font-size: 0.82rem; }
  .dest { width: 100%; font-weight: 550; }
  .dest input { font-family: var(--mono); font-size: 0.8rem; }
  .hint { font-weight: 400; font-size: 0.78rem; }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; flex-wrap: wrap; }
  .value { font-size: 1.3rem; font-weight: 650; font-variant-numeric: tabular-nums; }
  .meta { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .txs ol { list-style: none; margin: 0.35rem 0 0; padding: 0; }
  .txs li { display: grid; grid-template-columns: 1.4rem 1fr auto; gap: 0.5rem; align-items: center; padding: 0.4rem 0; border-top: 1px solid var(--line); font-size: 0.88rem; }
  .txs .step { width: 1.4rem; height: 1.4rem; border-radius: 999px; background: var(--card); color: var(--muted); display: grid; place-items: center; font-size: 0.72rem; font-weight: 600; }
  .txs a { text-decoration: none; font-size: 0.8rem; }
  .timer { background: var(--hot-bg); color: var(--fg); border-radius: 8px; padding: 0.6rem 0.8rem; font-size: 0.92rem; }
  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .side { border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem; display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; }

  .side p { margin: 0; font-size: 0.9rem; }
  @media (max-width: 40rem) { .sides { grid-template-columns: 1fr; } }
</style>
