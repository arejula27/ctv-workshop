// workshop-server API.
import type { VaultParams } from './core'
import type { MinerSettings } from './events'

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

export const joinRoom = (name: string, address: string) =>
  call('POST', '/api/participants', { name, address })
export const participants = () => call<{ name: string }[]>('GET', '/api/participants')
export const registerVault = (owner: string, params: VaultParams, outpoint: string) =>
  call('POST', '/api/vaults', { owner, params, outpoint })
export const listVaults = () => call<VaultEntry[]>('GET', '/api/vaults')
export const reportActivity = (who: string, kind: ActivityKind, outpoint: string, txid: string) =>
  call('POST', '/api/activity', { who, kind, outpoint, txid })

// ---------- admin ----------

export const admin = {
  mine: (token: string, blocks: number) => call<{ height: number }>('POST', '/api/admin/mine', { blocks }, token),
  jump: (token: string, blocks: number) => call<{ height: number }>('POST', '/api/admin/jump', { blocks }, token),
  miner: (token: string, settings: Partial<MinerSettings>) =>
    call<MinerSettings>('PUT', '/api/admin/miner', settings, token),
}
