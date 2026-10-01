<script lang="ts">
  // One tile of the projector board. Tones follow the app: hot = orange, cold = blue, thief = gray.
  import type { Snippet } from 'svelte'

  let {
    title,
    subtitle,
    tone,
    pulse = false,
    children,
  }: { title: string; subtitle: string; tone?: 'hot' | 'cold' | 'thief'; pulse?: boolean; children: Snippet } =
    $props()
</script>

<div class="tile {tone ?? ''}" class:pulse>
  <div class="title">{title}</div>
  <div class="subtitle">{subtitle}</div>
  <div class="state">{@render children()}</div>
</div>

<style>
  .tile { border-radius: 14px; padding: 0.8rem 1rem; background: var(--card); border: 3px solid var(--line); }
  .title { font-weight: 700; font-size: 1.3rem; }
  .subtitle { color: var(--muted); font-variant-numeric: tabular-nums; }
  .state { font-weight: 600; margin-top: 0.3rem; }
  .hot { border-color: var(--hot); background: var(--hot-bg); }
  .cold { border-color: var(--cold); background: var(--cold-bg); }
  .thief { border-color: var(--thief); background: var(--thief-bg); }
  .pulse { animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { transform: scale(1.03); } }
  @media (prefers-reduced-motion: reduce) { .pulse { animation: none; } }
</style>
