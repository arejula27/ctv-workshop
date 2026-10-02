// Mirror of workshop-server's `Event` (crates/workshop-server/src/events.rs).
export type MinerSettings = { paused: boolean; interval_s: number }

/** The demo the room is playing (they run one after another). */
export type Demo = 'vault' | 'inheritance' | 'subscription' | 'pool'

export type Status = {
  network: string
  height: number
  miner: MinerSettings
  next_block_in_ms: number | null
  faucet_amount_sat: number
  /** Simulated calendar: block h happens at sim_epoch_s + (h - 1) * sim_block_s. */
  sim_epoch_s: number
  sim_block_s: number
  demo: Demo
  explorer_url: string
}

export type FeedEvent =
  | { type: 'faucet'; address: string; amount_sat: number; txid: string }
  | { type: 'miner'; settings: MinerSettings }
  | { type: 'jump'; blocks: number; height: number }
  | { type: 'participant_joined'; name: string }
  | { type: 'demo_changed'; demo: Demo }
  | { type: 'hot_wallet_created'; owner: string }
  | { type: 'vault_registered'; owner: string; amount_sat: number; delay: number; outpoint: string }
  | {
      type: 'contract_registered'
      kind: import('./api').ContractKind
      owner: string
      amount_sat: number
      id: string
      renewed: boolean
    }
  | {
      type: 'pool_updated'
      id: string
      name: string
      members: string[]
      state: 'open' | 'funding' | 'funded'
      change: 'created' | 'joined' | 'funded'
      amount_sat: number
    }
  | {
      type: 'activity'
      who: string
      kind: import('./api').ActivityKind
      owner: string
      amount_sat: number
      outpoint: string
      txid: string
    }

export type ServerEvent =
  | { type: 'hello'; status: Status; recent: FeedEvent[] }
  | { type: 'block'; height: number; hash: string }
  | FeedEvent

/** Connects to /ws and reconnects forever. Returns a function that closes it. */
export function subscribe(onEvent: (e: ServerEvent) => void, onOpen?: (open: boolean) => void) {
  let ws: WebSocket | undefined
  let closed = false
  const connect = () => {
    const proto = location.protocol === 'https:' ? 'wss' : 'ws'
    ws = new WebSocket(`${proto}://${location.host}/ws`)
    ws.onopen = () => onOpen?.(true)
    ws.onmessage = (m) => onEvent(JSON.parse(m.data))
    ws.onclose = () => {
      onOpen?.(false)
      if (!closed) setTimeout(connect, 1000)
    }
  }
  connect()
  return () => {
    closed = true
    ws?.close()
  }
}
