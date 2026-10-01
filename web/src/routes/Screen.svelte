<script lang="ts">
  import { onMount } from 'svelte'
  import { listPools, listVaults, type PoolTable, type VaultEntry } from '../lib/api'
  import { chain, simDate } from '../lib/store.svelte'
  import { room, syncRoom, nameOf } from '../lib/room.svelte'
  import { DEMOS, demoOf, describe } from '../lib/feed'
  import { date } from '../lib/format'
  import ScreenVault from '../components/ScreenVault.svelte'
  import ScreenInheritance from '../components/ScreenInheritance.svelte'
  import ScreenSubscription from '../components/ScreenSubscription.svelte'
  import ScreenPool from '../components/ScreenPool.svelte'
  import Tile from '../components/Tile.svelte'

  let vaults = $state<VaultEntry[]>([])
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
  let inheritances = $derived(room.contracts.filter((c) => c.kind === 'inheritance'))
  let subscriptions = $derived(room.contracts.filter((c) => c.kind === 'subscription'))
  let count = $derived(
    { vault: vaults.length, inheritance: inheritances.length, subscription: subscriptions.length, pool: pools.length }[chain.demo],
  )
</script>

<div class="screen">
  <section class="clock">
    <div class="label">Bloque</div>
    <div class="height">{chain.height || '—'}</div>
    <div class="simdate">📅 {date(simDate(chain.height))}</div>
    <div class="next">
      {#if !chain.connected}
        desconectado
      {:else if chain.miner?.paused}
        ⏸ reloj en pausa
      {:else if countdown != null}
        siguiente en <b>{countdown}s</b>
      {/if}
    </div>
    <div class="url"><span class="muted">Entra en</span><br />{location.host}{demo.path}</div>
  </section>

  <section class="board">
    <h1>{demo.icon} {demo.title} <span class="muted">({count})</span></h1>
    {#if count === 0}
      <p class="muted">Todavía no hay nada. ¡Empezad!</p>
    {/if}
    <div class="grid">
      {#if chain.demo === 'vault'}
        {#each vaults as v (v.outpoint)}<ScreenVault {v} stolen={stolen.has(v.outpoint)} />{/each}
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
          {#if t.state === 'funded'}
            <ScreenPool table={t} />
          {:else}
            <Tile title="🪑 {t.name}" subtitle="{t.members.length}/6 sentadas">
              {t.members.map((m) => m.label).join(', ') || 'esperando gente…'}
            </Tile>
          {/if}
        {/each}
      {/if}
    </div>
  </section>

  <section class="feed">
    <h2>Qué está pasando</h2>
    <ul>
      {#each feed as e, i (i + JSON.stringify(e))}
        {@const d = describe(e)}
        <li class={d.tone ?? ''}><span class="icon">{d.icon}</span>{d.text}</li>
      {/each}
    </ul>
  </section>
</div>

<style>
  .screen {
    display: grid;
    grid-template-columns: 18rem 1fr 24rem;
    gap: 1.5rem;
    padding: 1.5rem;
    height: 100vh;
    overflow: hidden;
  }
  /* Newest on top; older items just fall off the bottom of the projector. */
  .feed, .board { overflow: hidden; }
  .clock { text-align: center; }
  .label { text-transform: uppercase; letter-spacing: 0.1em; color: var(--muted); }
  .height { font-size: 6rem; font-weight: 800; font-variant-numeric: tabular-nums; line-height: 1; }
  .simdate { font-size: 1.5rem; font-weight: 700; margin-top: 0.5rem; }
  .next { font-size: 1.4rem; margin-top: 0.5rem; }
  .url { margin-top: 2rem; font-size: 1.2rem; font-weight: 700; word-break: break-all; }
  h1 { margin-top: 0; font-size: 2.2rem; }
  h2 { margin-top: 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr)); gap: 0.8rem; }
  ul { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.5rem; }
  li { font-size: 1.1rem; padding: 0.5rem 0.7rem; border-radius: 10px; background: var(--card); display: flex; gap: 0.5rem; }
  li.hot { background: var(--hot-bg); }
  li.cold { background: var(--cold-bg); }
  li.thief { background: var(--thief-bg); border: 2px dashed var(--thief); }
  @media (max-width: 70rem) {
    .screen { grid-template-columns: 1fr; height: auto; overflow: visible; }
  }
</style>
