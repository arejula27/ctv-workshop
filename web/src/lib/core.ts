// Typed wrappers over the ctv-wasm bindings (crates/ctv-wasm/src/lib.rs).
import init, * as wasm from '../wasm/ctv_wasm.js'

export const ready: Promise<unknown> = init()

export type WalletInfo = {
  main_address: string
  hot_address: string
  cold_address: string
  main_pubkey: string
  hot_pubkey: string
  cold_pubkey: string
}

export type SignedTx = { hex: string; txid: string }

export type Utxo = { txid: string; vout: number; value: number; address: string }

export type VaultParams = {
  amount_sat: number
  hot_pubkey: string
  cold_pubkey: string
  delay: number
  fee_sat: number
}

// Mirror of ctv_core::tree views
export type ContractView = {
  label: string
  value_sat: number
  address: string
  outpoint: string | null
  leaves: LeafView[]
}
export type LeafView = {
  label: string
  kind: 'ctv' | 'key'
  csv: number | null
  signer: string | null
  tx: TxView | null
}
export type TxView = { label: string; txid: string | null; fee_sat: number; outputs: OutputView[] }
export type OutputView = { label: string; value_sat: number; contract: ContractView | null }

export function newSeed(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32))
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('')
}

export const walletInfo = (seed: string): WalletInfo => wasm.wallet_info(seed)
export const hotSecret = (seed: string): string => wasm.hot_secret(seed)
/** Secret of a wallet's main key (signs a vault's hot withdrawal; what a thief gets from a leaked seed). */
export const walletSecret = (seed: string): string => wasm.wallet_secret(seed)
/** Everything the wallet owns in `utxos`, to `to`, minus the fee. */
export const walletSweep = (seed: string, utxos: Utxo[], to: string): SignedTx => wasm.wallet_sweep(seed, utxos, to)
export const secretPubkey = (secret: string): string => wasm.secret_pubkey(secret)
export const walletSend = (seed: string, utxos: Utxo[], to: string, sats: number): SignedTx =>
  wasm.wallet_send(seed, utxos, to, BigInt(sats))

export const vault = {
  address: (p: VaultParams): string => wasm.vault_address(p),
  view: (p: VaultParams, outpoint?: string): ContractView => wasm.vault_view(p, outpoint ?? null),
  unvaultOutpoint: (p: VaultParams, op: string): string => wasm.vault_unvault_outpoint(p, op),
  start: (p: VaultParams, op: string): SignedTx => wasm.vault_start(p, op),
  toCold: (p: VaultParams, op: string): SignedTx => wasm.vault_to_cold(p, op),
  panic: (p: VaultParams, op: string): SignedTx => wasm.vault_panic(p, op),
  withdrawHot: (p: VaultParams, op: string, to: string, hotSecret: string): SignedTx =>
    wasm.vault_withdraw_hot(p, op, to, hotSecret),
}

// ---------- inheritance ----------

export type Heir = { label: string; address: string; share: number }
export type InheritanceParams = {
  amount_sat: number
  owner_pubkey: string
  heirs: Heir[]
  delay: number
  fee_sat: number
}

export const inheritance = {
  address: (p: InheritanceParams): string => wasm.inheritance_address(p),
  view: (p: InheritanceParams, op?: string): ContractView => wasm.inheritance_view(p, op ?? null),
  /** "I'm alive": the renewed inheritance sits at `${txid}:0` with `next_params`. */
  keepAlive: (p: InheritanceParams, op: string, seed: string): SignedTx & { next_params: InheritanceParams } =>
    wasm.inheritance_keep_alive(p, op, seed),
  withdraw: (p: InheritanceParams, op: string, seed: string, to: string): SignedTx =>
    wasm.inheritance_withdraw(p, op, seed, to),
  claim: (p: InheritanceParams, op: string): SignedTx => wasm.inheritance_claim(p, op),
}

// ---------- subscription ----------

export type SubscriptionParams = {
  payment_sat: number
  count: number
  every: number
  receiver_address: string
  payer_pubkey: string
  fee_sat: number
}

export const subscription = {
  amount: (p: SubscriptionParams): number => Number(wasm.subscription_amount(p)),
  address: (p: SubscriptionParams): string => wasm.subscription_address(p),
  view: (p: SubscriptionParams, funding?: string): ContractView => wasm.subscription_view(p, funding ?? null),
  pay: (p: SubscriptionParams, k: number, funding: string): SignedTx => wasm.subscription_pay(p, k, funding),
  cancel: (p: SubscriptionParams, k: number, funding: string, seed: string, to: string): SignedTx =>
    wasm.subscription_cancel(p, k, funding, seed, to),
}

// ---------- payment pool ----------

export type PoolMember = { label: string; pubkey: string; address: string }
export type PoolParams = { members: PoolMember[]; share_sat: number; fee_sat: number }

export const pool = {
  address: (p: PoolParams): string => wasm.pool_address(p),
  /** Every exit order: prune it before drawing (see treeStatus.prune). */
  view: (p: PoolParams, funding?: string): ContractView => wasm.pool_view(p, funding ?? null),
  /** `who` leaves the pool left after `exited`, which sits at `current`. */
  exit: (p: PoolParams, exited: number[], who: number, current: string, seed: string): SignedTx =>
    wasm.pool_exit(p, new Uint32Array(exited), who, current, seed),
}
