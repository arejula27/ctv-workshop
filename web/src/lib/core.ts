// Typed wrappers over the ctv-wasm bindings (crates/ctv-wasm/src/lib.rs).
import init, * as wasm from '../wasm/ctv_wasm.js'

export const ready: Promise<unknown> = init()

export type WalletInfo = {
  main_address: string
  hot_address: string
  cold_address: string
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
