// On-chain statuses of a tree, refreshed every few seconds and on every block.
import { onMount } from 'svelte'
import type { ContractView } from './core'
import { chain } from './store.svelte'
import { treeStatus, type ContractStatus } from './treeStatus'

export function useStatuses(view: () => ContractView) {
  const s = $state({ map: new Map<string, ContractStatus>() })
  const refresh = () => treeStatus(view()).then((m) => (s.map = m)).catch(() => {})
  onMount(() => {
    const t = setInterval(refresh, 5000)
    return () => clearInterval(t)
  })
  $effect(() => {
    void chain.tick
    void view()
    refresh()
  })
  return s
}
