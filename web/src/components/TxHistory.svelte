<script lang="ts">
  import type { HistoryEntry } from '../lib/history'
  import { txUrl } from '../lib/explorer'
  import { simDate } from '../lib/store.svelte'
  import { date, sats, short } from '../lib/format'

  let {
    entries,
    labelOf = () => undefined,
  }: { entries: HistoryEntry[]; labelOf?: (address: string) => string | undefined } = $props()
</script>

{#if entries.length === 0}
  <p class="muted empty">Todavía no hay movimientos.</p>
{:else}
  <ul>
    {#each entries as e (e.txid)}
      {@const label = e.counterparty ? (labelOf(e.counterparty) ?? short(e.counterparty, 6)) : '—'}
      <li>
        <span class="dir" class:in={e.delta > 0}>{e.delta > 0 ? '↓' : '↑'}</span>
        <div class="what">
          <div>{e.delta > 0 ? 'Recibido' : 'Enviado'} <span class="muted">{e.delta > 0 ? 'de' : 'a'} {label}</span></div>
          <div class="muted small">
            {#if e.confirmed}{date(simDate(e.height ?? 0))}{:else}<span class="badge hot">Pendiente</span>{/if}
          </div>
        </div>
        <div class="value" class:in={e.delta > 0}>{e.delta > 0 ? '+' : '−'}{sats(Math.abs(e.delta))}</div>
        <a href={txUrl(e.txid)} target="_blank" rel="noopener" title="Ver en el explorador">↗</a>
      </li>
    {/each}
  </ul>
{/if}

<style>
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: grid; grid-template-columns: 2rem 1fr auto 1.5rem; gap: 0.6rem; align-items: center; padding: 0.55rem 0; border-top: 1px solid var(--line); }
  .dir { width: 2rem; height: 2rem; border-radius: 999px; display: grid; place-items: center; background: var(--card); color: var(--muted); font-weight: 700; }
  .dir.in { background: var(--ok-bg); color: var(--ok); }
  .what { min-width: 0; font-size: 0.9rem; }
  .small { font-size: 0.78rem; }
  .value { font-variant-numeric: tabular-nums; font-weight: 600; font-size: 0.92rem; }
  .value.in { color: var(--ok); }
  a { text-decoration: none; color: var(--muted); font-weight: 600; text-align: center; }
  a:hover { color: var(--cold); }
  .empty { font-size: 0.88rem; margin: 0.5rem 0 0; }
</style>
