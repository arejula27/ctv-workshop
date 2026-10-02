<script lang="ts">
  import { vault as core, type SignedTx, type VaultParams } from '../lib/core'
  import { broadcastAndReport, friendly } from '../lib/act'
  import type { ActivityKind } from '../lib/api'
  import { chain, isMine, markMine, simDate } from '../lib/store.svelte'
  import { useStatuses } from '../lib/useStatuses.svelte'
  import { blocksLeft as blocksLeftOf, unvaultOutpoint, vaultPhase } from '../lib/vaultPhase'
  import { blocksToTime, date, sats, simDuration } from '../lib/format'
  import TreeView from './TreeView.svelte'

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
    role: 'owner' | 'thief'
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
  let hotByMe = $derived(isMine(unvault?.spentBy))
  let busy = $state(false)
  let error = $state('')
  let showTree = $state(false)
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
  const withdrawHot = () =>
    act(isThief ? 'theft_success' : 'vault_withdraw', () => core.withdrawHot(params, outpoint, destination, secret))

  const toneOf = (label: string) =>
    /caja fuerte|pánico/i.test(label) ? 'cold' : /caliente/i.test(label) ? 'hot' : undefined
  const ready = $derived(blocksLeft != null && blocksLeft <= 0)
</script>

<article class="panel card" class:thief={isThief} class:alarm={phase === 'withdrawing'}>
  <header>
    <div>
      <div class="eyebrow">{isThief ? `Vault de ${owner}` : 'Vault'}</div>
      <div class="value">{sats(params.amount_sat)}</div>
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
        <div class="eyebrow">Hacia la cartera caliente</div>
        {#if phase === 'locked'}
          <p>{isThief ? 'Inicia el retiro. Luego hay que esperar, y la dueña lo verá.' : `Primero se inicia el retiro y luego hay que esperar ${params.delay} bloques.`}</p>
          <button class="hot" onclick={start} disabled={locked}>Iniciar retiro</button>
        {:else if phase === 'withdrawing'}
          <p>{isThief ? 'Cuando acabe la espera, llévatelo a tu cartera.' : 'Cuando acabe la espera, puedes sacarlo.'}</p>
          <button class="hot" onclick={withdrawHot} disabled={locked || !ready}>{isThief ? 'Llevármelo' : 'Retirar a la cartera caliente'}</button>
        {:else if phase === 'hot'}
          <p><b>{sats(params.amount_sat - 2 * params.fee_sat)}</b> salieron por aquí{hotByMe ? '.' : isThief ? ', pero no a tu cartera.' : ' sin que fueras tú.'}</p>
        {:else}
          <p class="muted">Camino cerrado.</p>
        {/if}
      </div>
      <div class="side cold">
        <div class="eyebrow">Hacia la caja fuerte</div>
        {#if isThief && (phase === 'locked' || phase === 'withdrawing')}
          <p class="muted">Fuera de tu alcance. Si {owner} pulsa el pánico, el dinero acaba aquí.</p>
        {:else if phase === 'locked'}
          <p>Al instante, sin esperas.</p>
          <button class="secondary" onclick={toCold} disabled={locked}>Mandar a la caja fuerte</button>
        {:else if phase === 'withdrawing'}
          <p><b>¿No has sido tú?</b> Alguien tiene tu llave caliente. El pánico lo manda todo a la caja fuerte y nadie puede impedirlo.</p>
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

  <footer>
    <button class="ghost" onclick={() => (showTree = !showTree)}>{showTree ? 'Ocultar' : 'Ver'} transacciones del contrato</button>
  </footer>
  {#if showTree}
    <div class="tree"><TreeView contract={view} statuses={st.map} {toneOf} /></div>
  {/if}
</article>

<style>
  .card { display: flex; flex-direction: column; gap: 0.8rem; }
  .card.alarm { border-color: var(--hot); box-shadow: 0 0 0 3px var(--hot-bg); }
  .card.thief { border-top: 3px solid var(--thief); }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; flex-wrap: wrap; }
  .value { font-size: 1.3rem; font-weight: 650; font-variant-numeric: tabular-nums; }
  .meta { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .timer { background: var(--hot-bg); color: var(--fg); border-radius: 8px; padding: 0.6rem 0.8rem; font-size: 0.92rem; }
  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .side { border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem; display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; }
  .side.hot { border-left: 3px solid var(--hot); }
  .side.cold { border-left: 3px solid var(--cold); }
  .side p { margin: 0; font-size: 0.9rem; }
  footer { display: flex; }
  .tree { overflow-x: auto; }
  @media (max-width: 40rem) { .sides { grid-template-columns: 1fr; } }
</style>
