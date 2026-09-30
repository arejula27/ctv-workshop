<script lang="ts">
  import { newSeed } from '../lib/core'
  import { setIdentity } from '../lib/store.svelte'

  let name = $state('')
  let restoring = $state(false)
  let seed = $state('')
  let error = $state('')

  function start(e: SubmitEvent) {
    e.preventDefault()
    const s = restoring ? seed.trim().toLowerCase() : newSeed()
    if (!/^[0-9a-f]{64}$/.test(s)) {
      error = 'La semilla son 64 caracteres (0-9, a-f).'
      return
    }
    setIdentity({ name: name.trim(), seed: s })
  }
</script>

<h1>Bienvenida, bienvenido</h1>
<p>
  Vas a tener una cartera de bitcoin de pruebas en este navegador. El dinero no es real: juega sin
  miedo.
</p>

<form onsubmit={start}>
  <label>
    ¿Cómo te llamas?
    <input bind:value={name} required maxlength="24" placeholder="Tu nombre" />
  </label>
  {#if restoring}
    <label>
      Tu semilla
      <input bind:value={seed} placeholder="64 caracteres" />
    </label>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
  <button type="submit">{restoring ? 'Recuperar cartera' : 'Crear mi cartera'}</button>
  <button type="button" class="secondary" onclick={() => (restoring = !restoring)}>
    {restoring ? 'Crear una nueva' : 'Ya tengo una semilla'}
  </button>
</form>

<style>
  form { display: flex; flex-direction: column; gap: 0.75rem; margin-top: 1.5rem; }
  label { display: flex; flex-direction: column; gap: 0.3rem; font-weight: 600; }
</style>
