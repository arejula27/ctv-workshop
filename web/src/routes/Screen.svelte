<script lang="ts">
  import { onMount } from 'svelte'
  import { listVaults, type VaultEntry } from '../lib/api'
  import { chain, simDate } from '../lib/store.svelte'
  import { date } from '../lib/format'
  import { describe } from '../lib/feed'
  import ScreenVault from '../components/ScreenVault.svelte'

  let vaults = $state<VaultEntry[]>([])
  let now = $state(Date.now())

  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 250)
    return () => clearInterval(t)
  })

  $effect(() => {
    void chain.vaultsTick
    listVaults().then((l) => (vaults = l)).catch(() => {})
  })

  let countdown = $derived(
    chain.nextBlockAt == null ? null : Math.max(0, Math.ceil((chain.nextBlockAt - now) / 1000)),
  )
  // Vaults emptied by a thief, from the feed (the chain alone can't tell theft from withdrawal).
  let stolen = $derived(
    new Set(chain.feed.flatMap((e) => (e.type === 'activity' && e.kind === 'theft_success' ? [e.outpoint] : []))),
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
    <div class="url"><span class="muted">Entra en</span><br />{location.host}</div>
  </section>

  <section class="board">
    <h2>Vaults de la sala <span class="muted">({vaults.length})</span></h2>
    {#if vaults.length === 0}
      <p class="muted">Todavía nadie ha creado un vault.</p>
    {/if}
    <div class="grid">
      {#each vaults as v (v.outpoint)}
        <ScreenVault {v} stolen={stolen.has(v.outpoint)} />
      {/each}
    </div>
  </section>

  <section class="feed">
    <h2>Qué está pasando</h2>
    <ul>
      {#each chain.feed as e, i (i + JSON.stringify(e))}
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
  .feed { overflow: hidden; }
  .board { overflow: hidden; }
  .clock { text-align: center; }
  .label { text-transform: uppercase; letter-spacing: 0.1em; color: var(--muted); }
  .height { font-size: 6rem; font-weight: 800; font-variant-numeric: tabular-nums; line-height: 1; }
  .simdate { font-size: 1.5rem; font-weight: 700; margin-top: 0.5rem; }
  .next { font-size: 1.4rem; margin-top: 0.5rem; }
  .url { margin-top: 2rem; font-size: 1.2rem; font-weight: 700; word-break: break-all; }
  h2 { margin-top: 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr)); gap: 0.8rem; }
  ul { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.5rem; }
  li { font-size: 1.1rem; padding: 0.5rem 0.7rem; border-radius: 10px; background: var(--card); display: flex; gap: 0.5rem; }
  li.hot { background: var(--hot-bg); }
  li.cold { background: var(--cold-bg); }
  li.thief { background: var(--thief-bg); border: 2px dashed var(--thief); }
  @media (max-width: 70rem) {
    .screen { grid-template-columns: 1fr; height: auto; overflow: visible; }
  }
</style>
