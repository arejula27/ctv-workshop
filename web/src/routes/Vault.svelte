<script lang="ts">
  import { saveVaultGame, vaultGame } from '../lib/store.svelte'
  import VaultOwner from '../components/VaultOwner.svelte'
  import VaultThief from '../components/VaultThief.svelte'
</script>

<div class="head">
  <div>
    <div class="eyebrow">Demo · Vault</div>
    <h1>{vaultGame.role === 'thief' ? 'Ladrona' : vaultGame.role === 'owner' ? 'Tus carteras' : 'Elige tu papel'}</h1>
  </div>
  {#if vaultGame.role}
    <button class="ghost" onclick={() => saveVaultGame({ role: null })}>Cambiar de papel</button>
  {/if}
</div>

{#if !vaultGame.role}
  <div class="roles">
    <button class="role panel hot" onclick={() => saveVaultGame({ role: 'owner' })}>
      <span class="eyebrow">Usuaria</span>
      <b>Protege tu dinero</b>
      <span class="muted">Tienes una cartera en el móvil y guardas tus ahorros en un vault. Si alguien te roba la llave, tendrás tiempo de reaccionar.</span>
    </button>
    <button class="role panel thief" onclick={() => saveVaultGame({ role: 'thief' })}>
      <span class="eyebrow">Ladrona</span>
      <b>Roba lo que puedas</b>
      <span class="muted">Tienes las semillas filtradas de las carteras calientes de la sala. Vacíalas y ataca sus vaults… si te dejan.</span>
    </button>
  </div>
{:else if vaultGame.role === 'owner'}
  <VaultOwner />
{:else}
  <VaultThief />
{/if}

<style>
  .head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1rem; }
  .roles { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .role {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    padding: 1.25rem;
  }
  .role.hot { border-top: 3px solid var(--hot); }
  .role.thief { border-top: 3px solid var(--thief); }
  .role:hover { border-color: var(--fg); }
  .role b { font-size: 1.15rem; }
  .role .muted { font-size: 0.9rem; }
  @media (max-width: 40rem) { .roles { grid-template-columns: 1fr; } }
</style>
