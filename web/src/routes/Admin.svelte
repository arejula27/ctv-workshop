<script lang="ts">
  import { admin, listVaults, participants } from '../lib/api'
  import { chain } from '../lib/store.svelte'
  import { blocksToTime } from '../lib/format'
  import { DEMOS } from '../lib/feed'
  import type { Demo } from '../lib/events'

  const KEY = 'ctv.admin'
  let token = $state(readToken())
  let busy = $state(false)
  let error = $state('')
  let people = $state<{ name: string }[]>([])
  let vaultCount = $state(0)
  let custom = $state(144)

  function readToken() {
    try {
      return localStorage.getItem(KEY) ?? ''
    } catch {
      return ''
    }
  }
  function saveToken() {
    try {
      localStorage.setItem(KEY, token)
    } catch {
      // not persisted; still works this session
    }
  }

  $effect(() => {
    void chain.feed.length
    participants().then((p) => (people = p)).catch(() => {})
    listVaults().then((v) => (vaultCount = v.length)).catch(() => {})
  })

  async function run(fn: () => Promise<unknown>) {
    busy = true
    error = ''
    try {
      saveToken()
      await fn()
    } catch (e) {
      error = (e as Error).message
    } finally {
      busy = false
    }
  }

  const intervals = [5, 10, 30, 60]
  const jumps = [1, 5, 10, 50]
</script>

<h1>Panel del presentador</h1>

<label>
  Token de admin
  <input type="password" bind:value={token} placeholder="ADMIN_TOKEN de infra/.env" autocomplete="off" />
</label>

<section>
  <h2>Demo que se juega ahora</h2>
  <p class="muted">Cambia lo que enseña <code>/screen</code> y marca la pestaña en la app de todo el mundo.</p>
  <div class="row">
    {#each Object.entries(DEMOS) as [demo, d]}
      <button class="secondary" class:on={chain.demo === demo}
        onclick={() => run(() => admin.demo(token, demo as Demo))} disabled={busy || !token}>{d.icon} {d.title}</button>
    {/each}
  </div>
</section>

<section>
  <h2>Reloj</h2>
  <p>
    Bloque <b>{chain.height}</b> ·
    {#if chain.miner?.paused}⏸ en pausa{:else if chain.miner}un bloque cada {chain.miner.interval_s} s{/if}
  </p>
  <div class="row">
    {#if chain.miner?.paused}
      <button onclick={() => run(() => admin.miner(token, { paused: false }))} disabled={busy || !token}>▶ Reanudar</button>
    {:else}
      <button onclick={() => run(() => admin.miner(token, { paused: true }))} disabled={busy || !token}>⏸ Pausar</button>
    {/if}
    {#each intervals as s}
      <button class="secondary" class:on={chain.miner?.interval_s === s}
        onclick={() => run(() => admin.miner(token, { interval_s: s }))} disabled={busy || !token}>{s} s</button>
    {/each}
  </div>
</section>

<section>
  <h2>Saltar en el tiempo</h2>
  <div class="row">
    {#each jumps as n}
      <button class="secondary" onclick={() => run(() => admin.jump(token, n))} disabled={busy || !token}>
        +{n} {n === 1 ? 'bloque' : 'bloques'}
      </button>
    {/each}
  </div>
  <div class="row">
    <input type="number" bind:value={custom} min="1" max="1000" style="max-width: 8rem" />
    <button onclick={() => run(() => admin.jump(token, custom))} disabled={busy || !token}>
      Saltar {custom} bloques
    </button>
    <span class="muted">{blocksToTime(custom, chain.miner?.interval_s)} a ritmo normal</span>
  </div>
</section>

{#if error}<p class="error">{error}</p>{/if}

<section>
  <h2>Sala</h2>
  <p>{people.length} participantes · {vaultCount} vaults · <a href="/screen" target="_blank">abrir /screen ↗</a></p>
  <p class="muted">{people.map((p) => p.name).join(', ')}</p>
</section>

<section>
  <h2>Reiniciar</h2>
  <p class="muted">Desde el host: <code>infra/scripts/reset.sh</code> (borra la cadena y la sala).</p>
</section>

<style>
  section { margin: 1.5rem 0; }
  label { display: flex; flex-direction: column; gap: 0.3rem; font-weight: 600; }
  .row { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; margin-top: 0.5rem; }
  .on { outline: 2px solid var(--fg); }
</style>
