// workshop-server API.
import type { InheritanceParams, PoolMember, SubscriptionParams, VaultParams } from './core'
import type { Demo, MinerSettings } from './events'

async function call<T>(method: string, path: string, body?: unknown, token?: string): Promise<T> {
  const headers: Record<string, string> = {}
  if (body !== undefined) headers['Content-Type'] = 'application/json'
  if (token) headers.Authorization = `Bearer ${token}`
  const r = await fetch(path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) })
  const data = await r.json().catch(() => ({}))
  if (!r.ok) throw new Error(data.error ?? `${path}: ${r.status}`)
  return data as T
}

export const faucet = (address: string) =>
  call<{ txid: string; amount_sat: number }>('POST', '/api/faucet', { address })

// ---------- room ----------

export type VaultEntry = { owner: string; params: VaultParams; outpoint: string; registered_at: number }

export type ActivityKind =
  | 'vault_start'
  | 'vault_to_cold'
  | 'vault_panic'
  | 'vault_withdraw'
  | 'theft_start'
  | 'theft_success'
  | 'inheritance_keep_alive'
  | 'inheritance_withdraw'
  | 'inheritance_claim'
  | 'subscription_pay'
  | 'subscription_cancel'
  | 'pool_exit'

export const joinRoom = (name: string, address: string) =>
  call('POST', '/api/participants', { name, address })
export type Participant = { name: string; address: string }
export const participants = () => call<Participant[]>('GET', '/api/participants')
export const registerVault = (owner: string, params: VaultParams, outpoint: string) =>
  call('POST', '/api/vaults', { owner, params, outpoint })
export const listVaults = () => call<VaultEntry[]>('GET', '/api/vaults')
export type ContractKind = 'inheritance' | 'subscription'
export type ContractEntry =
  | { id: string; kind: 'inheritance'; owner: string; params: InheritanceParams; outpoint: string; registered_at: number }
  | { id: string; kind: 'subscription'; owner: string; params: SubscriptionParams; outpoint: string; registered_at: number }

/** Registers (or moves, for the same `id`) an inheritance or subscription. */
export const registerContract = (
  id: string,
  kind: ContractKind,
  owner: string,
  params: InheritanceParams | SubscriptionParams,
  outpoint: string,
) => call('POST', '/api/contracts', { id, kind, owner, params, outpoint })
export const listContracts = () => call<ContractEntry[]>('GET', '/api/contracts')

/** `outpoint`: a vault's funding outpoint, or a contract's id. */
export const reportActivity = (who: string, kind: ActivityKind, outpoint: string, txid: string) =>
  call('POST', '/api/activity', { who, kind, outpoint, txid })

// ---------- pool tables ----------

export type PoolTable = {
  id: string
  name: string
  host: string
  members: PoolMember[]
  share_sat: number
  fee_sat: number
  state: 'open' | 'funding' | 'funded'
  outpoint: string | null
  created_at: number
}

export const listPools = () => call<PoolTable[]>('GET', '/api/pools')
export const createPool = (name: string, host: string) => call<PoolTable>('POST', '/api/pools', { name, host })
export const joinPool = (id: string, member: PoolMember) => call<PoolTable>('POST', `/api/pools/${id}/join`, member)
export const startPool = (id: string) => call<PoolTable>('POST', `/api/pools/${id}/start`)

// ---------- admin ----------

export const admin = {
  mine: (token: string, blocks: number) => call<{ height: number }>('POST', '/api/admin/mine', { blocks }, token),
  jump: (token: string, blocks: number) => call<{ height: number }>('POST', '/api/admin/jump', { blocks }, token),
  miner: (token: string, settings: Partial<MinerSettings>) =>
    call<MinerSettings>('PUT', '/api/admin/miner', settings, token),
  demo: (token: string, demo: Demo) => call<{ demo: Demo }>('PUT', '/api/admin/demo', { demo }, token),
}
