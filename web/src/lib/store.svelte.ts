// App state shared by every page. Persisted bits live in localStorage (test coins only).
import { subscribe, type Demo, type FeedEvent, type MinerSettings } from './events'
import { joinRoom, registerContract, registerHotWallet, registerVault, type ContractKind } from './api'
import {
  newSeed,
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
    // The vault game's hot wallet too, so thieves see it even after a server reset.
    .then(() => vaultGame.hot && registerHotWallet(name, vaultGame.hot.seed))
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
  /** Bumped when a pool table changes. */
  poolsTick: 0,
  /** Bumped when someone publishes a hot wallet (vault game). */
  walletsTick: 0,
  /** Block explorer base URL for transaction links. */
  explorerUrl: '',
  /** The demo the presenter has on screen now. */
  demo: 'vault' as Demo,
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
          forgetOldChain(e.status.chain_id)
          chain.height = e.status.height
          chain.miner = e.status.miner
          chain.nextBlockAt =
            e.status.next_block_in_ms == null ? null : Date.now() + e.status.next_block_in_ms
          chain.feed = [...e.recent].reverse()
          chain.simEpoch = e.status.sim_epoch_s
          chain.simBlock = e.status.sim_block_s
          chain.demo = e.status.demo
          chain.explorerUrl = e.status.explorer_url
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
        case 'demo_changed':
          chain.demo = e.demo
          break
      }
      // Signature bookkeeping only refreshes the pool, it isn't news for the feed.
      const quiet = e.type === 'pool_updated' && ['signature', 'failed', 'proposal'].includes(e.change)
      if (quiet) chain.poolsTick++
      if (e.type !== 'hello' && e.type !== 'block' && !quiet) {
        chain.feed = [e, ...chain.feed].slice(0, FEED_LEN)
        if (e.type === 'vault_registered') chain.vaultsTick++
        if (e.type === 'contract_registered') chain.contractsTick++
        if (e.type === 'pool_updated') chain.poolsTick++
        if (e.type === 'hot_wallet_created') chain.walletsTick++
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

// ---------- vault game ----------

/** The safe box: a wallet of our own, or a friend's hot wallet. */
export type SafeBox =
  | { kind: 'own'; seed: string }
  | { kind: 'friend'; owner: string; address: string; pubkey: string }

export type VaultGame = {
  role: 'owner' | 'thief' | null
  /** Owner: the everyday wallet that can be stolen. */
  hot?: { seed: string }
  safe?: SafeBox
  /** Thief: where the loot goes. */
  thief?: { seed: string }
}

export const vaultGame = $state<VaultGame>(load<VaultGame>('ctv.vaultgame', { role: null }))

export function saveVaultGame(patch: Partial<VaultGame>) {
  Object.assign(vaultGame, patch)
  save('ctv.vaultgame', $state.snapshot(vaultGame))
}

// Transactions we broadcast ourselves, to tell our own withdrawal from a theft.
const mineTxids = $state<{ list: string[] }>({ list: load<string[]>('ctv.mytxids', []) })
export const isMine = (txid: string | undefined) => !!txid && mineTxids.list.includes(txid)
export function markMine(txid: string) {
  mineTxids.list = [...mineTxids.list, txid]
  save('ctv.mytxids', mineTxids.list)
}

// ---------- pool game ----------

/** The pool demo's wallet: its on-chain account and its key inside the pool. */
export const poolGame = $state<{ seed?: string }>(load<{ seed?: string }>('ctv.poolgame', {}))
export function savePoolGame(patch: { seed?: string }) {
  Object.assign(poolGame, patch)
  save('ctv.poolgame', $state.snapshot(poolGame))
}

// ---------- chain resets ----------

/** After a reset the server runs a new chain: whatever this browser saved from
 *  the old one (vaults, contracts, each demo's wallets, the role) points to
 *  coins that no longer exist. Drop it and keep only the name. */
function forgetOldChain(chainId: string) {
  if (!chainId) return
  let known: string | null = null
  try {
    known = localStorage.getItem('ctv.chain')
  } catch {
    return
  }
  if (known === chainId) return
  try {
    localStorage.setItem('ctv.chain', chainId)
  } catch {
    // storage unavailable: nothing saved to forget either
  }
  // First visit ever: nothing to forget.
  if (known === null && vaults.list.length === 0 && !vaultGame.hot && !poolGame.seed && contracts.list.length === 0) return
  vaults.list = []
  contracts.list = []
  mineTxids.list = []
  for (const k of Object.keys(vaultGame) as (keyof VaultGame)[]) delete vaultGame[k]
  vaultGame.role = null
  delete poolGame.seed
  for (const key of ['ctv.vaults', 'ctv.contracts', 'ctv.mytxids', 'ctv.vaultgame', 'ctv.poolgame']) {
    try {
      localStorage.removeItem(key)
    } catch {
      // ignore
    }
  }
  if (me.id) {
    // A fresh general wallet too: the old one's coins are gone with the old chain.
    setIdentity({ name: me.id.name, seed: newSeed() })
  }
}
