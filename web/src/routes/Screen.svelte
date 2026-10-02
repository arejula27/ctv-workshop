<script lang="ts">
  import { onMount } from 'svelte'
  import { listHotWallets, listPools, listVaults, type HotWallet, type PoolTable, type VaultEntry } from '../lib/api'
  import { chain, simDate } from '../lib/store.svelte'
  import { room, syncRoom, nameOf } from '../lib/room.svelte'
  import { DEMOS, demoOf, describe } from '../lib/feed'
  import { date } from '../lib/format'
  import ScreenPerson from '../components/ScreenPerson.svelte'
  import ScreenInheritance from '../components/ScreenInheritance.svelte'
  import ScreenSubscription from '../components/ScreenSubscription.svelte'
  import ScreenPool from '../components/ScreenPool.svelte'
  import Tile from '../components/Tile.svelte'

  let vaults = $state<VaultEntry[]>([])
  let wallets = $state<HotWallet[]>([])
  let pools = $state<PoolTable[]>([])
  let now = $state(Date.now())

  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 250)
    return () => clearInterval(t)
  })

  // Only fetch what the current demo needs.
  $effect(() => {
    void chain.vaultsTick
    if (chain.demo === 'vault') listVaults().then((l) => (vaults = l)).catch(() => {})
  })
  $effect(() => {
    void chain.walletsTick
    if (chain.demo === 'vault') listHotWallets().then((l) => (wallets = l)).catch(() => {})
  })
  $effect(() => {
    void chain.poolsTick
    if (chain.demo === 'pool') listPools().then((l) => (pools = l)).catch(() => {})
  })
  $effect(() => {
    void chain.feed.length
    void chain.contractsTick
    syncRoom()
  })

  let demo = $derived(DEMOS[chain.demo])
  let countdown = $derived(
    chain.nextBlockAt == null ? null : Math.max(0, Math.ceil((chain.nextBlockAt - now) / 1000)),
  )
  // Room-wide events plus the ones of the current demo.
  let feed = $derived(chain.feed.filter((e) => {
    const d = demoOf(e)
    return d == null || d === chain.demo
  }))
  // Vaults emptied by a thief, from the feed (the chain alone can't tell theft from withdrawal).
  let stolen = $derived(
    new Set(chain.feed.flatMap((e) => (e.type === 'activity' && e.kind === 'theft_success' ? [e.outpoint] : []))),
  )
  // Vault demo: one card per person, with their hot wallet and their vaults.
  let people = $derived.by(() => {
    const names = [...new Set([...wallets.map((w) => w.owner), ...vaults.map((v) => v.owner)])]
    return names.map((owner) => ({
      owner,
      wallets: wallets.filter((w) => w.owner === owner),
      vaults: vaults.filter((v) => v.owner === owner),
    }))
  })
  let inheritances = $derived(room.contracts.filter((c) => c.kind === 'inheritance'))
  let subscriptions = $derived(room.contracts.filter((c) => c.kind === 'subscription'))
  let count = $derived(
    { vault: people.length, inheritance: inheritances.length, subscription: subscriptions.length, pool: pools.length }[chain.demo],
  )
</script>

<div class="screen">
  <aside class="side">
    <section class="panel clock">
      <div class="eyebrow">Bloque</div>
      <div class="height">{chain.height || '—'}</div>
      <div class="simdate">{date(simDate(chain.height))}</div>
      <div class="next muted">
        {#if !chain.connected}
          Desconectado
        {:else if chain.miner?.paused}
          Reloj en pausa
        {:else if countdown != null}
          Siguiente bloque en <b>{countdown} s</b>
        {/if}
      </div>
    </section>
    <section class="panel join">
      <div class="eyebrow">Entra en</div>
      <div class="url mono">{location.host}{demo.path}</div>
    </section>
  </aside>

  <section class="board">
    <div class="eyebrow">Demo</div>
    <h1>{demo.title} <span class="badge">{count}</span></h1>
    {#if count === 0}
      <p class="muted">Todavía no hay nada. ¡Empezad!</p>
    {/if}
    <div class="grid">
      {#if chain.demo === 'vault'}
        {#each people as p (p.owner)}<ScreenPerson owner={p.owner} wallets={p.wallets} vaults={p.vaults} {stolen} />{/each}
      {:else if chain.demo === 'inheritance'}
        {#each inheritances as c (c.id)}
          {#if c.kind === 'inheritance'}<ScreenInheritance owner={c.owner} params={c.params} outpoint={c.outpoint} />{/if}
        {/each}
      {:else if chain.demo === 'subscription'}
        {#each subscriptions as c (c.id)}
          {#if c.kind === 'subscription'}
            <ScreenSubscription owner={c.owner} receiver={nameOf(c.params.receiver_address)} params={c.params} outpoint={c.outpoint} />
          {/if}
        {/each}
      {:else}
        {#each pools as t (t.id)}
          {#if t.state === 'funded' || t.state === 'dissolved'}
            <ScreenPool table={t} />
          {:else}
            <Tile title={t.name} subtitle="{t.members.length}/6 personas">
              <span class="badge">Esperando</span>
              <div class="muted names">{t.members.map((m) => m.label).join(', ') || 'Nadie todavía'}</div>
            </Tile>
          {/if}
        {/each}
      {/if}
    </div>
  </section>

  <section class="panel feed">
    <div class="eyebrow">Actividad</div>
    <ul>
      {#each feed as e, i (i + JSON.stringify(e))}
        {@const d = describe(e)}
        <li><span class="dot {d.tone ?? ''}"></span><span>{d.text}</span></li>
      {/each}
    </ul>
  </section>
</div>

<style>
  .screen {
    display: grid;
    grid-template-columns: 17rem 1fr 24rem;
    gap: 1.25rem;
    padding: 1.25rem;
    height: 100vh;
    overflow: hidden;
    background: var(--bg);
  }
  /* Newest on top; older items just fall off the bottom of the projector. */
  .feed, .board { overflow: hidden; }
  .side { display: flex; flex-direction: column; gap: 1rem; }
  .clock { display: flex; flex-direction: column; gap: 0.25rem; }
  .height { font-size: 4.5rem; font-weight: 650; letter-spacing: -0.03em; font-variant-numeric: tabular-nums; line-height: 1.05; }
  .simdate { font-size: 1.15rem; font-weight: 600; }
  .next { font-size: 1rem; }
  .url { font-size: 1.05rem; font-weight: 600; margin-top: 0.3rem; }
  .board h1 { font-size: 2rem; display: flex; align-items: center; gap: 0.6rem; margin: 0.2rem 0 1rem; }
  .board h1 .badge { font-size: 0.9rem; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr)); gap: 0.9rem; align-items: start; }
  .names { font-size: 0.9rem; margin-top: 0.4rem; }
  ul { list-style: none; padding: 0; margin: 0.5rem 0 0; }
  li { display: flex; gap: 0.6rem; align-items: baseline; font-size: 1rem; padding: 0.6rem 0; border-top: 1px solid var(--line); }
  li .dot { flex-shrink: 0; position: relative; top: -1px; }
  @media (max-width: 70rem) {
    .screen { grid-template-columns: 1fr; height: auto; overflow: visible; }
  }
</style>
