// App state shared by every page. Persisted bits live in localStorage (test coins only).
import { subscribe, type FeedEvent, type MinerSettings } from './events'
import { joinRoom, registerContract, registerVault, type ContractKind } from './api'
import {
  walletInfo,
  type InheritanceParams,
  type SubscriptionParams,
  type VaultParams,
  type WalletInfo,
} from './core'
import * as esplora from './esplora'

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    return raw ? (JSON.parse(raw) as T) : fallback
  } catch {
    return fallback
  }
}

function save(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // private mode / storage full: the app keeps working for this session
  }
}

// ---------- identity ----------

export type Identity = { name: string; seed: string }

export const me = $state<{ id: Identity | null; wallet: WalletInfo | null }>({
  id: load<Identity | null>('ctv.identity', null),
  wallet: null,
})

export function setIdentity(id: Identity | null) {
  me.id = id
  me.wallet = null
  save('ctv.identity', id)
}

/** Call after the wasm module is ready. Also (re)announces us and our vaults
 *  to the room, which is idempotent and survives a server reset. */
export function initWallet() {
  if (!me.id || me.wallet) return
  me.wallet = walletInfo(me.id.seed)
  const { name } = me.id
  joinRoom(name, me.wallet.main_address)
    .then(() =>
      Promise.allSettled(
        vaults.list
          .filter((v) => v.role === 'owner')
          .map((v) => registerVault(name, v.params, v.outpoint)),
      ),
    )
    .then(() =>
      Promise.allSettled(
        contracts.list.map((c) => registerContract(c.id, c.kind, name, c.params, c.outpoint)),
      ),
    )
    .catch(() => {})
}

// ---------- chain clock (workshop-server /ws) ----------

export const chain = $state({
  connected: false,
  height: 0,
  miner: null as MinerSettings | null,
  nextBlockAt: null as number | null,
  /** Bumped on every block so pages can refetch. */
  tick: 0,
  /** Newest first. */
  feed: [] as FeedEvent[],
  /** Bumped when someone registers a vault. */
  vaultsTick: 0,
  /** Bumped when an inheritance or subscription is registered or moves. */
  contractsTick: 0,
  /** Simulated calendar (see Status). */
  simEpoch: 0,
  simBlock: 600,
})

/** Simulated date of block `height`: 10 minutes per block, like real Bitcoin, so a
 *  jump of 144 blocks moves the calendar one day forward. */
export function simDate(height: number): Date | null {
  if (!chain.simEpoch) return null
  return new Date((chain.simEpoch + (height - 1) * chain.simBlock) * 1000)
}

const FEED_LEN = 30

let started = false
export function startChain() {
  if (started) return
  started = true
  const resetCountdown = () =>
    (chain.nextBlockAt =
      chain.miner && !chain.miner.paused ? Date.now() + chain.miner.interval_s * 1000 : null)
  subscribe(
    (e) => {
      switch (e.type) {
        case 'hello':
          chain.height = e.status.height
          chain.miner = e.status.miner
          chain.nextBlockAt =
            e.status.next_block_in_ms == null ? null : Date.now() + e.status.next_block_in_ms
          chain.feed = [...e.recent].reverse()
          chain.simEpoch = e.status.sim_epoch_s
          chain.simBlock = e.status.sim_block_s
          chain.tick++
          break
        case 'block':
          chain.height = e.height
          resetCountdown()
          chain.tick++
          break
        case 'miner':
          chain.miner = e.settings
          resetCountdown()
          break
      }
      if (e.type !== 'hello' && e.type !== 'block') {
        chain.feed = [e, ...chain.feed].slice(0, FEED_LEN)
        if (e.type === 'vault_registered') chain.vaultsTick++
        if (e.type === 'contract_registered') chain.contractsTick++
      }
    },
    (open) => (chain.connected = open),
  )
}

// ---------- balances ----------

export const balances = $state({ main: 0, cold: 0, pending: 0, loaded: false })

export async function refreshBalances() {
  if (!me.wallet) return
  const [main, cold] = await Promise.all([
    esplora.utxos(me.wallet.main_address),
    esplora.utxos(me.wallet.cold_address),
  ])
  const sum = (l: { value: number }[]) => l.reduce((a, u) => a + u.value, 0)
  balances.main = sum(main)
  balances.cold = sum(cold)
  balances.pending = sum([...main, ...cold].filter((u) => !u.status.confirmed))
  balances.loaded = true
}

/** Spendable coins: everyday wallet first; the cold box only if asked. */
export async function spendableUtxos(includeCold = false) {
  if (!me.wallet) return []
  const lists = [esplora.utxos(me.wallet.main_address)]
  if (includeCold) lists.push(esplora.utxos(me.wallet.cold_address))
  return (await Promise.all(lists)).flat()
}

// ---------- vaults ----------

export type SavedVault = {
  id: string
  role: 'owner' | 'thief'
  params: VaultParams
  /** Funding outpoint, `txid:vout`. */
  outpoint: string
  /** Owner's name (for the thief's view). */
  owner?: string
  /** Only for the thief: the stolen hot key. */
  hotSecret?: string
  /** Hot withdrawal we broadcast ourselves, to tell it apart from a theft. */
  hotTxid?: string
  createdAt: number
}

export const vaults = $state<{ list: SavedVault[] }>({ list: load<SavedVault[]>('ctv.vaults', []) })

export function addVault(v: SavedVault) {
  vaults.list = [v, ...vaults.list.filter((x) => x.id !== v.id)]
  save('ctv.vaults', vaults.list)
}

export function removeVault(id: string) {
  vaults.list = vaults.list.filter((v) => v.id !== id)
  save('ctv.vaults', vaults.list)
}

// ---------- inheritances and subscriptions we own ----------

export type SavedContract =
  | { id: string; kind: 'inheritance'; params: InheritanceParams; outpoint: string; createdAt: number }
  | { id: string; kind: 'subscription'; params: SubscriptionParams; outpoint: string; createdAt: number }

export const contracts = $state<{ list: SavedContract[] }>({
  list: load<SavedContract[]>('ctv.contracts', []),
})

/** Insert or update by id (an inheritance's "I'm alive" moves it). */
export function saveContract(c: SavedContract) {
  contracts.list = [c, ...contracts.list.filter((x) => x.id !== c.id)]
  save('ctv.contracts', contracts.list)
}

export function removeContract(id: string) {
  contracts.list = contracts.list.filter((c) => c.id !== id)
  save('ctv.contracts', contracts.list)
}

export type { ContractKind }
