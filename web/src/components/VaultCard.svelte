<script lang="ts">
  import { onMount } from 'svelte'
  import { hotSecret, vault as core, type SignedTx } from '../lib/core'
  import * as esplora from '../lib/esplora'
  import { addVault, chain, me, refreshBalances, removeVault, simDate, type SavedVault } from '../lib/store.svelte'
  import { treeStatus, type ContractStatus } from '../lib/treeStatus'
  import { blocksToTime, date, sats, simDuration } from '../lib/format'
  import TreeView from './TreeView.svelte'
  import { blocksLeft as blocksLeftOf, unvaultOutpoint, vaultPhase } from '../lib/vaultPhase'
  import { reportActivity, type ActivityKind } from '../lib/api'

  let { v }: { v: SavedVault } = $props()

  let view = $derived(core.view(v.params, v.outpoint))
  let statuses = $state(new Map<string, ContractStatus>())
  let busy = $state(false)
  let error = $state('')
  let showTree = $state(false)
  let showKit = $state(false)

  let unvault = $derived(statuses.get(unvaultOutpoint(view)))
  let phase = $derived(vaultPhase(view, statuses))
  let blocksLeft = $derived(blocksLeftOf(unvault, v.params.delay, chain.height))
  let hotByMe = $derived(unvault?.spentBy != null && unvault.spentBy === v.hotTxid)
  let isThief = $derived(v.role === 'thief')

  async function refresh() {
    try {
      statuses = await treeStatus(view)
    } catch (e) {
      error = (e as Error).message
    }
  }

  onMount(() => {
    refresh()
    const t = setInterval(refresh, 4000)
    return () => clearInterval(t)
  })

  $effect(() => {
    void chain.tick
    refresh()
  })

  async function act(kind: ActivityKind, build: () => SignedTx, after?: (tx: SignedTx) => void) {
    busy = true
    error = ''
    try {
      const tx = build()
      await esplora.broadcast(tx.hex)
      after?.(tx)
      // Tell the room (feeds /screen); never block the action on it.
      if (me.id) reportActivity(me.id.name, kind, v.outpoint, tx.txid).catch(() => {})
      await refresh()
      refreshBalances().catch(() => {})
    } catch (e) {
      error = (e as Error).message
    } finally {
      busy = false
    }
  }

  const start = () =>
    act(isThief ? 'theft_start' : 'vault_start', () => core.start(v.params, v.outpoint))
  const toCold = () => act('vault_to_cold', () => core.toCold(v.params, v.outpoint))
  const panic = () => act('vault_panic', () => core.panic(v.params, v.outpoint))
  function withdrawHot() {
    if (!me.wallet) return
    const secret = isThief ? v.hotSecret! : hotSecretOfMine()
    act(
      isThief ? 'theft_success' : 'vault_withdraw',
      () => core.withdrawHot(v.params, v.outpoint, me.wallet!.main_address, secret),
      (tx) => addVault({ ...v, hotTxid: tx.txid }),
    )
  }

  // The owner's hot key comes from their seed.
  const hotSecretOfMine = () => hotSecret(me.id!.seed)

  // What the owner hands to the thief: just the hot key. The thief's app finds
  // the vault through the room registry.
  let kit = $derived(v.role === 'owner' && me.id ? hotSecretOfMine() : '')

  const toneOf = (label: string) =>
    /caja fuerte|pánico/i.test(label) ? 'cold' : /caliente/i.test(label) ? 'hot' : undefined
</script>

<article class="card {phase}" class:thief={isThief}>
  <header>
    <strong>{isThief ? `🦹 Vault de ${v.owner ?? 'alguien'}` : '🔒 Tu vault'}</strong>
    <span class="muted">{sats(v.params.amount_sat)} · espera de {v.params.delay} bloques ({simDuration(v.params.delay, chain.simBlock)})</span>
  </header>

  <!-- What is happening, in plain words -->
  <div class="story">
    {#if phase === 'loading'}
      <p class="muted">Buscando el vault en la cadena…</p>
    {:else if phase === 'funding'}
      <p>Guardando el dinero en el vault. Se confirma en el próximo bloque.</p>
    {:else if phase === 'locked'}
      <p>{isThief
        ? `Tienes la llave caliente de ${v.owner ?? 'alguien'}. Para llevártelo hay que iniciar un retiro y esperar ${v.params.delay} bloques… y todo el mundo lo verá.`
        : 'El dinero está en el vault. Solo puede salir por uno de estos dos caminos.'}</p>
    {:else if phase === 'withdrawing'}
      <div class="timer">
        {#if unvault?.funding !== 'confirmed'}
          ⏱ Retiro iniciado, esperando al próximo bloque…
        {:else if blocksLeft != null && blocksLeft > 0}
          ⏱ Faltan <b>{blocksLeft}</b> bloques para que la llave caliente pueda llevárselo:
          se abre el <b>{date(simDate(chain.height + 1 + blocksLeft))}</b>
          <span class="muted">({blocksToTime(blocksLeft, chain.miner?.interval_s)} en la sala)</span>
        {:else}
          ⏱ ¡Tiempo cumplido! La llave caliente ya puede llevárselo
        {/if}
      </div>
    {:else if phase === 'cold'}
      <p>🧊 El dinero está en la caja fuerte{isThief ? ` de ${v.owner ?? 'su dueña'}` : ''}.</p>
    {:else if phase === 'panicked'}
      <p>{isThief
        ? `🚨 ${v.owner ?? 'Su dueña'} pulsó el pánico a tiempo. No hay nada que robar.`
        : '🚨 Pánico pulsado a tiempo. El dinero está a salvo en tu caja fuerte.'}</p>
    {:else if phase === 'hot'}
      <p>{hotByMe
        ? isThief ? '💰 ¡Robo completado! Revisa tu cartera caliente.' : '🔥 Retirado a tu cartera caliente.'
        : isThief ? 'Otra persona con la llave se lo ha llevado.' : '😱 Te han robado: alguien con tu llave caliente se lo llevó cuando terminó la espera.'}</p>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>

  <!-- Left: the hot side. Right: the safe side. Same split as the page. -->
  {#if phase !== 'loading' && phase !== 'funding'}
    <div class="sides">
      <div class="side hot" class:winner={phase === 'hot'}>
        {#if phase === 'locked'}
          <p>{isThief ? 'Empieza el retiro. Luego, a esperar.' : `Sacarlo a tu cartera caliente: primero se inicia el retiro y luego hay que esperar ${v.params.delay} bloques.`}</p>
          <button class="hot" onclick={start} disabled={busy}>Iniciar retiro</button>
        {:else if phase === 'withdrawing'}
          <p>{isThief ? 'Cuando acabe la espera, llévatelo.' : 'Cuando acabe la espera, puedes sacarlo.'}</p>
          <button class="hot" onclick={withdrawHot} disabled={busy || blocksLeft == null || blocksLeft > 0}>
            {isThief ? 'Llevármelo' : 'Retirar a mi cartera caliente'}
          </button>
        {:else if phase === 'hot'}
          <p class="landed">🔥 {sats(v.params.amount_sat - 2 * v.params.fee_sat)} salieron por aquí</p>
        {:else}
          <p class="muted">Camino cerrado.</p>
        {/if}
      </div>
      <div class="side cold" class:winner={phase === 'cold' || phase === 'panicked'}>
        {#if isThief && (phase === 'locked' || phase === 'withdrawing')}
          <p class="muted">🔒 La caja fuerte de {v.owner ?? 'su dueña'}: fuera de tu alcance. Si pulsa el pánico, el dinero acabará aquí.</p>
        {:else if phase === 'locked'}
          <p>Mandarlo ya a la caja fuerte. Al instante, sin esperas.</p>
          <button class="cold" onclick={toCold} disabled={busy}>Mandar a la caja fuerte</button>
        {:else if phase === 'withdrawing'}
          <p><b>¿No has sido tú?</b> Alguien tiene tu llave caliente. El pánico lo manda todo a la caja fuerte y nadie puede impedirlo.</p>
          <button class="cold big" onclick={panic} disabled={busy}>🚨 Botón del pánico</button>
        {:else if phase === 'cold' || phase === 'panicked'}
          <p class="landed">🧊 {sats(v.params.amount_sat - (phase === 'cold' ? 1 : 2) * v.params.fee_sat)} a salvo aquí</p>
        {:else}
          <p class="muted">Camino cerrado.</p>
        {/if}
      </div>
    </div>
  {/if}

  <footer>
    <button class="secondary" onclick={() => (showTree = !showTree)}>
      {showTree ? 'Ocultar' : 'Ver'} árbol de transacciones
    </button>
    {#if !isThief && (phase === 'locked' || phase === 'withdrawing')}
      <button class="secondary" onclick={() => (showKit = !showKit)}>🦹 Dejar que me roben la llave caliente</button>
    {/if}
    {#if phase === 'cold' || phase === 'panicked' || phase === 'hot'}
      <button class="secondary" onclick={() => removeVault(v.id)}>Archivar</button>
    {/if}
  </footer>

  {#if showKit}
    <div class="kit">
      <p>Pasa esta llave a quien haga de ladrona. Es tu <b class="t-hot">llave caliente</b>, y nada más: la caja fuerte sigue siendo solo tuya.</p>
      <textarea readonly rows="2" onfocus={(e) => e.currentTarget.select()}>{kit}</textarea>
      <button class="secondary" onclick={() => navigator.clipboard?.writeText(kit)}>Copiar</button>
    </div>
  {/if}

  {#if showTree}
    <div class="tree"><TreeView contract={view} {statuses} {toneOf} /></div>
  {/if}
</article>

<style>
  .card {
    border: 2px solid var(--line);
    border-radius: 16px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    background: var(--bg);
  }
  .card.withdrawing { border-color: var(--hot); }
  .card.thief { background: var(--thief-bg); border-color: var(--thief); }
  header { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: baseline; justify-content: center; text-align: center; }
  .story { text-align: center; }
  .story p { margin: 0.25rem 0; }
  .timer { background: var(--hot-bg); border-radius: 10px; padding: 0.6rem 0.8rem; font-weight: 600; }
  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .side {
    border-radius: 12px;
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    align-items: flex-start;
    border: 2px solid transparent;
  }
  .side p { margin: 0; font-size: 0.92rem; }
  .side.hot { background: var(--hot-bg); }
  .side.cold { background: var(--cold-bg); }
  .side.hot.winner { border-color: var(--hot); }
  .side.cold.winner { border-color: var(--cold); }
  .landed { font-weight: 700; }
  .big { font-size: 1.1rem; padding: 0.85rem 1.2rem; }
  footer { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .t-hot { color: var(--hot); }
  .kit { display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; }
  .kit textarea { font-family: monospace; font-size: 0.75rem; }
  .tree { overflow-x: auto; }
  @media (max-width: 40rem) {
    .sides { grid-template-columns: 1fr; }
  }
</style>
