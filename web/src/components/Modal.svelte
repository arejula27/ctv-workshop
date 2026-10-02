<script lang="ts">
  import type { Snippet } from 'svelte'

  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props()

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}>
  <div class="dialog panel" role="dialog" tabindex="-1" aria-modal="true" aria-label={title} onclick={(e) => e.stopPropagation()}>
    <header>
      <h2>{title}</h2>
      <button class="ghost" onclick={onclose} aria-label="Cerrar">✕</button>
    </header>
    {@render children()}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.4);
    display: grid;
    place-items: center;
    padding: 1rem;
    z-index: 10;
  }
  .dialog { width: min(28rem, 100%); display: flex; flex-direction: column; gap: 0.9rem; }
  header { display: flex; justify-content: space-between; align-items: center; }
</style>
