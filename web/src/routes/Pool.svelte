<script lang="ts">
  import { createPool, joinPool, listPools, startPool, type PoolTable } from '../lib/api'
  import { friendly } from '../lib/act'
  import { chain, me } from '../lib/store.svelte'
  import { sats } from '../lib/format'
  import PoolCard from '../components/PoolCard.svelte'

  let tables = $state<PoolTable[]>([])
  let name = $state('')
  let busy = $state(false)
  let error = $state('')

  $effect(() => {
    void chain.poolsTick
    listPools().then((t) => (tables = t)).catch(() => {})
  })

  const seated = (t: PoolTable) => t.members.some((m) => m.pubkey === me.wallet?.main_pubkey)
  let open = $derived(tables.filter((t) => t.state !== 'funded'))
  let mine = $derived(tables.filter((t) => t.state === 'funded' && seated(t)).reverse())

  async function run(fn: () => Promise<unknown>) {
    busy = true
    error = ''
    try {
      await fn()
      tables = await listPools()
    } catch (e) {
      error = friendly(e)
    } finally {
      busy = false
    }
  }

  const me_member = () => ({ label: me.id!.name, pubkey: me.wallet!.main_pubkey, address: me.wallet!.main_address })

  function create(e: SubmitEvent) {
    e.preventDefault()
    run(async () => {
      const t = await createPool(name.trim(), me.id!.name)
      await joinPool(t.id, me_member())
      name = ''
    })
  }
</script>

<h1>Payment pool</h1>
<p class="intro">Varias personas comparten un único UTXO. Cualquiera puede salir cuando quiera con su parte, sin pedir permiso, y nadie puede echar a nadie.</p>

<div class="split">
  <section class="half hot">
    <div class="icon">🪑</div>
    <h2>Mesas</h2>
    <p class="hint">Sentaos 2–6 personas en la misma mesa y pulsad «Empezar». El pool lo pone la casa: {sats(100_000)} por persona.</p>

    {#each open as t (t.id)}
      <div class="table">
        <div><b>«{t.name}»</b> <span class="muted">· {t.members.length}/6</span></div>
        <div class="muted">{t.members.map((m) => m.label).join(', ') || 'vacía'}</div>
        <div class="row">
          {#if !seated(t)}
            <button class="secondary" onclick={() => run(() => joinPool(t.id, me_member()))} disabled={busy || t.members.length >= 6 || t.state !== 'open'}>Sentarme</button>
          {:else}
            <span class="muted">Ya estás en esta mesa</span>
          {/if}
          <button class="hot" onclick={() => run(() => startPool(t.id))} disabled={busy || t.members.length < 2 || !seated(t) || t.state !== 'open'}>
            {t.state === 'funding' ? 'Empezando…' : '¡Empezar!'}
          </button>
        </div>
      </div>
    {/each}

    <form onsubmit={create}>
      <input bind:value={name} placeholder="Nombre de una mesa nueva" maxlength="24" required />
      <button type="submit" class="secondary" disabled={busy || !name.trim()}>Abrir mesa</button>
    </form>
    {#if error}<p class="error">{error}</p>{/if}
  </section>

  <section class="half cold">
    <div class="icon">🏊</div>
    <h2>¿Por qué un pool?</h2>
    <p class="hint">Un solo UTXO para muchas personas ocupa menos espacio en la cadena y, desde fuera, no se sabe cuánto es de cada una: privacidad tipo coinjoin.</p>
    <p class="hint">CTV fija de antemano todas las formas de salir. Cada salida exige la firma de quien sale, así que nadie puede sacar a otra persona.</p>
  </section>
</div>

{#each mine as t (t.id)}
  <PoolCard table={t} />
{/each}

<style>
  .intro { margin-top: 0; }
  .split { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin: 1rem 0; }
  .half { border-radius: 16px; padding: 1rem; display: flex; flex-direction: column; gap: 0.6rem; }
  .half.hot { background: var(--hot-bg); border: 2px solid var(--hot); }
  .half.cold { background: var(--cold-bg); border: 2px solid var(--cold); }
  h2 { margin: 0; }
  .icon { font-size: 2rem; }
  .hint { font-size: 0.85rem; color: var(--muted); margin: 0; }
  .table { background: var(--bg); border-radius: 12px; padding: 0.6rem 0.8rem; display: flex; flex-direction: column; gap: 0.3rem; }
  .row, form { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; }
  form input { flex: 1; min-width: 10rem; }
  @media (max-width: 40rem) { .split { grid-template-columns: 1fr; } }
</style>
