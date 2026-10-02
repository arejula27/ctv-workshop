<script lang="ts">
  import { me, setIdentity } from '../lib/store.svelte'
  import WalletPanel from '../components/WalletPanel.svelte'

  let showSeed = $state(false)

  function logout() {
    if (confirm('¿Salir? Se olvidarán tus carteras de este navegador.')) setIdentity(null)
  }
</script>

<h1>Hola, {me.id?.name}</h1>

{#if me.id}
  <WalletPanel title="Mi cartera" subtitle="Para la herencia, la suscripción y el pool" seed={me.id.seed} />
{/if}

<details>
  <summary class="muted">Ajustes</summary>
  <p>
    <button class="secondary" onclick={() => (showSeed = !showSeed)}>{showSeed ? 'Ocultar' : 'Ver'} semilla</button>
    {#if showSeed}<br /><code>{me.id?.seed}</code>{/if}
  </p>
  <button class="secondary" onclick={logout}>Salir de esta cartera</button>
</details>

<style>
  details { margin-top: 1.5rem; }
  details p { display: flex; flex-direction: column; gap: 0.3rem; align-items: flex-start; }
</style>
