<script lang="ts">
  // One tile of the projector board. Tones follow the app: hot = orange, cold = blue, thief = gray.
  import type { Snippet } from 'svelte'

  let {
    title,
    subtitle,
    tone,
    pulse = false,
    href,
    children,
  }: {
    title: string
    subtitle: string
    tone?: 'hot' | 'cold' | 'thief'
    pulse?: boolean
    /** Explorer link for the tile's current transaction. */
    href?: string
    children: Snippet
  } = $props()
</script>

<div class="tile {tone ?? ''}" class:pulse>
  <div class="title">
    <span><span class="dot {tone ?? ''}"></span>{title}</span>
    {#if href}<a {href} target="_blank" rel="noopener" title="Ver en el explorador">↗</a>{/if}
  </div>
  <div class="subtitle">{subtitle}</div>
  <div class="state">{@render children()}</div>
</div>

<style>
  .tile { border-radius: 12px; padding: 0.8rem 1rem; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow); }
  .title { font-weight: 650; font-size: 1.15rem; display: flex; justify-content: space-between; gap: 0.5rem; }
  .title a { text-decoration: none; color: var(--muted); font-size: 1rem; }
  .title a:hover { color: var(--cold); }
  .subtitle { color: var(--muted); font-variant-numeric: tabular-nums; }
  .state { margin-top: 0.6rem; }

  /* something is happening: a soft ring, not a jumping card */
  .pulse { animation: ring 1.6s ease-in-out infinite; }
  @keyframes ring { 50% { box-shadow: 0 0 0 4px var(--hot-bg), var(--shadow); } }
  @media (prefers-reduced-motion: reduce) { .pulse { animation: none; } }
</style>
