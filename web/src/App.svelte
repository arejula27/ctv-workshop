<script lang="ts">
  import { onMount } from 'svelte'
  import { ready } from './lib/core'
  import { chain, initWallet, me, refreshBalances, simDate, startChain } from './lib/store.svelte'
  import { date } from './lib/format'
  import { navigate, route } from './lib/router.svelte'
  import Onboarding from './routes/Onboarding.svelte'
  import WalletPage from './routes/Wallet.svelte'
  import VaultPage from './routes/Vault.svelte'
  import Screen from './routes/Screen.svelte'
  import Herencia from './routes/Herencia.svelte'
  import Suscripcion from './routes/Suscripcion.svelte'
  import PoolPage from './routes/Pool.svelte'
  import Admin from './routes/Admin.svelte'
  import Faucet from './routes/Faucet.svelte'

  let loaded = $state(false)
  let loadError = $state('')
  let now = $state(Date.now())

  onMount(() => {
    startChain()
    ready.then(() => (loaded = true)).catch((e) => (loadError = String(e)))
    const t = setInterval(() => (now = Date.now()), 500)
    // The indexer lags a moment behind each block/broadcast, so also poll.
    const poll = setInterval(() => me.wallet && refreshBalances().catch(() => {}), 5000)
    return () => {
      clearInterval(t)
      clearInterval(poll)
    }
  })

  // Wallet keys come from wasm, so derive them once it's loaded.
  $effect(() => {
    if (loaded && me.id) initWallet()
  })

  // Balances follow the chain.
  $effect(() => {
    void chain.tick
    if (me.wallet) refreshBalances().catch(() => {})
  })

  let countdown = $derived(
    chain.nextBlockAt == null ? null : Math.max(0, Math.ceil((chain.nextBlockAt - now) / 1000)),
  )

  const links = [
    { path: '/wallet', label: 'Cartera' },
    { path: '/vault', label: 'Vault', demo: 'vault' },
    { path: '/herencia', label: 'Herencia', demo: 'inheritance' },
    { path: '/suscripcion', label: 'Suscripción', demo: 'subscription' },
    { path: '/pool', label: 'Pool', demo: 'pool' },
  ]
</script>

{#if route.path !== '/screen'}
<header>
  <a class="brand" href="/wallet" onclick={(e) => { e.preventDefault(); navigate('/wallet') }}>CTV Workshop</a>
  <div class="clock" class:off={!chain.connected} title="Altura de la cadena">
    <span class="date" title="Calendario simulado: 10 minutos por bloque, como en Bitcoin">{date(simDate(chain.height))}</span>
    <span class="block">Bloque {chain.height || '—'}</span>
    <span class="muted">
      {#if !chain.connected}desconectado{:else if chain.miner?.paused}en pausa{:else if countdown != null}{countdown}s{/if}
    </span>
  </div>
</header>
{/if}

{#if loadError}
  <main><p class="error">No se pudo cargar la app: {loadError}</p></main>
{:else if !loaded}
  <main><p class="muted">Cargando…</p></main>
{:else if route.path === '/screen'}
  <Screen />
{:else if route.path === '/admin'}
  <main><Admin /></main>
{:else if route.path === '/faucet'}
  <Faucet />
{:else if !me.id}
  <main><Onboarding /></main>
{:else}
  <nav>
    {#each links as l}
      <a href={l.path} class:active={route.path === l.path || (route.path === '/' && l.path === '/wallet')}
        class:now={l.demo === chain.demo} title={l.demo === chain.demo ? 'La demo que estamos jugando ahora' : undefined}
        onclick={(e) => { e.preventDefault(); navigate(l.path) }}>{l.label}</a>
    {/each}
  </nav>
  <main>
    {#if route.path === '/vault'}
      <VaultPage />
    {:else if route.path === '/herencia'}
      <Herencia />
    {:else if route.path === '/suscripcion'}
      <Suscripcion />
    {:else if route.path === '/pool'}
      <PoolPage />
    {:else}
      <WalletPage />
    {/if}
  </main>
{/if}

<style>
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 1.25rem;
    height: 3.25rem;
    border-bottom: 1px solid var(--line);
    position: sticky;
    top: 0;
    background: var(--surface);
    z-index: 2;
  }
  .brand { font-weight: 650; color: var(--fg); text-decoration: none; letter-spacing: -0.01em; }
  .clock { font-variant-numeric: tabular-nums; font-size: 0.85rem; display: flex; flex-wrap: wrap; gap: 0 0.9rem; justify-content: flex-end; align-items: baseline; }
  .date { font-weight: 600; }
  .block { color: var(--muted); }
  .clock.off { color: var(--danger); }
  nav {
    display: flex;
    gap: 1.25rem;
    padding: 0 1.25rem;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
    overflow-x: auto;
  }
  nav a {
    white-space: nowrap;
    padding: 0.7rem 0 0.6rem;
    color: var(--muted);
    text-decoration: none;
    font-weight: 550;
    font-size: 0.92rem;
    border-bottom: 2px solid transparent;
  }
  nav a.active { color: var(--fg); border-bottom-color: var(--fg); }
  /* the demo the room is playing now */
  nav a.now::after { content: ''; display: inline-block; width: 6px; height: 6px; margin-left: 0.4rem; border-radius: 999px; background: var(--hot); vertical-align: middle; }
  main { max-width: 56rem; margin: 0 auto; padding: 1.5rem 1.25rem; }
</style>
