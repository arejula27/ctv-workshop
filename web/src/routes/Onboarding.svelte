<script lang="ts">
  import { newSeed } from '../lib/core'
  import { setIdentity } from '../lib/store.svelte'

  let name = $state('')

  function start(e: SubmitEvent) {
    e.preventDefault()
    // Only your name identifies you in the room. Each wallet makes its own seed;
    // this one backs the general "Cartera" tab.
    setIdentity({ name: name.trim(), seed: newSeed() })
  }
</script>

<div class="page">
  <section class="panel">
    <div class="eyebrow">CTV Workshop</div>
    <h1>¿Cómo te llamas?</h1>
    <p class="muted">Es el nombre con el que te verá el resto de la sala. Todo el bitcoin de este taller es de prueba.</p>
    <form onsubmit={start}>
      <input bind:value={name} required maxlength="24" placeholder="Tu nombre" aria-label="Tu nombre" />
      <button type="submit">Entrar</button>
    </form>
  </section>
</div>

<style>
  .page { max-width: 28rem; margin: 3rem auto; }
  .panel { display: flex; flex-direction: column; gap: 0.75rem; }
  form { display: flex; gap: 0.5rem; }
</style>
