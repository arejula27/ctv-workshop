// The room as seen from the browser: participants and registered contracts,
// refreshed when the feed says something changed.
import { listContracts, participants, type ContractEntry, type Participant } from './api'
import { chain } from './store.svelte'

export const room = $state<{ people: Participant[]; contracts: ContractEntry[] }>({
  people: [],
  contracts: [],
})

let lastJoin = -1
let lastContracts = -1

/** Call from a component's $effect: refetches when someone joins or a contract changes. */
export function syncRoom() {
  const joins = chain.feed.filter((e) => e.type === 'participant_joined').length
  if (joins !== lastJoin) {
    lastJoin = joins
    participants().then((p) => (room.people = p)).catch(() => {})
  }
  if (chain.contractsTick !== lastContracts) {
    lastContracts = chain.contractsTick
    listContracts().then((c) => (room.contracts = c)).catch(() => {})
  }
}

export const nameOf = (address: string) => room.people.find((p) => p.address === address)?.name ?? 'alguien'
