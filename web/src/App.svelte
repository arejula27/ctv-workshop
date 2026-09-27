<script lang="ts">
  import { onMount } from 'svelte'
  import { subscribe, type MinerSettings } from './lib/events'

  let connected = $state(false)
  let height = $state<number | null>(null)
  let miner = $state<MinerSettings | null>(null)
  let nextBlockAt = $state<number | null>(null)
  let now = $state(Date.now())
  let log = $state<string[]>([])

  const push = (line: string) => (log = [line, ...log].slice(0, 8))
  const resetCountdown = () =>
    (nextBlockAt = miner && !miner.paused ? Date.now() + miner.interval_s * 1000 : null)

  onMount(() => {
    const tick = setInterval(() => (now = Date.now()), 250)
    const close = subscribe((e) => {
      switch (e.type) {
        case 'hello':
          height = e.status.height
          miner = e.status.miner
          nextBlockAt = e.status.next_block_in_ms == null ? null : Date.now() + e.status.next_block_in_ms
          break
        case 'block':
          height = e.height
          resetCountdown()
          push(`Bloque ${e.height}`)
          break
        case 'jump':
          push(`⏩ Salto de ${e.blocks} bloques`)
          break
        case 'miner':
          miner = e.settings
          resetCountdown()
          push(e.settings.paused ? 'Reloj en pausa' : `Un bloque cada ${e.settings.interval_s}s`)
          break
        case 'faucet':
          push(`Faucet: ${e.amount_sat.toLocaleString()} sats`)
          break
      }
    }, (open) => (connected = open))
    return () => {
      clearInterval(tick)
      close()
    }
  })

  let countdown = $derived(nextBlockAt == null ? null : Math.max(0, Math.ceil((nextBlockAt - now) / 1000)))
</script>

<main>
  <h1>CTV Workshop</h1>
  <p class="conn" class:ok={connected}>{connected ? 'conectado' : 'conectando…'}</p>

  <section class="clock">
    <div>
      <span class="label">Bloque</span>
      <span class="value">{height ?? '—'}</span>
    </div>
    <div>
      <span class="label">Siguiente bloque</span>
      <span class="value">{miner?.paused ? 'pausa' : countdown == null ? '—' : `${countdown}s`}</span>
    </div>
  </section>

  <ul>
    {#each log as line}
      <li>{line}</li>
    {/each}
  </ul>
</main>

<style>
  main { max-width: 32rem; margin: 0 auto; padding: 2rem 1rem; }
  .conn { color: var(--muted); font-size: 0.9rem; }
  .conn.ok { color: var(--ok); }
  .clock { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin: 1.5rem 0; }
  .clock div { background: var(--card); border-radius: 12px; padding: 1rem; }
  .label { display: block; font-size: 0.8rem; color: var(--muted); }
  .value { font-size: 2.5rem; font-variant-numeric: tabular-nums; font-weight: 600; }
  ul { list-style: none; padding: 0; color: var(--muted); }
</style>
