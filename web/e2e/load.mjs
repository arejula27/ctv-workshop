// Load test: N simulated participants hit the stack at once, through the real
// wasm core, the REST API, esplora and the WebSocket — like a full room would.
//
//   ADMIN_TOKEN=... node web/e2e/load.mjs            (from the repo root, stack running)
//   N=30 BASE_URL=http://127.0.0.1:8080 node web/e2e/load.mjs
import { readFileSync } from 'node:fs'

const BASE = process.env.BASE_URL ?? 'http://127.0.0.1:8080'
const N = Number(process.env.N ?? 25)
const ADMIN = process.env.ADMIN_TOKEN
if (!ADMIN) throw new Error('set ADMIN_TOKEN')

const W = new URL('../src/wasm/', import.meta.url)
const wasm = await import(new URL('ctv_wasm.js', W))
wasm.initSync({ module: readFileSync(new URL('ctv_wasm_bg.wasm', W)) })

// ---------- plumbing with timing ----------
const timings = {}
const errors = []
async function timed(label, fn) {
  const t0 = performance.now()
  try {
    return await fn()
  } finally {
    ;(timings[label] ??= []).push(performance.now() - t0)
  }
}
async function http(method, path, body, headers = {}) {
  const r = await fetch(BASE + path, {
    method,
    headers: body === undefined ? headers : { 'Content-Type': 'application/json', ...headers },
    body: body === undefined ? undefined : typeof body === 'string' ? body : JSON.stringify(body),
  })
  const text = await r.text()
  if (!r.ok) throw new Error(`${method} ${path}: ${r.status} ${text.slice(0, 200)}`)
  try {
    return JSON.parse(text)
  } catch {
    return text
  }
}
const api = (m, p, b) => timed(`api ${m} ${p.replace(/\/[0-9a-f]{5}\//, '/:id/')}`, () => http(m, p, b))
const esplora = (p) => timed('esplora GET', () => http('GET', '/esplora' + p))
const broadcast = (hex) => timed('esplora broadcast', () => http('POST', '/esplora/tx', hex))
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const seed = () => [...crypto.getRandomValues(new Uint8Array(32))].map((b) => b.toString(16).padStart(2, '0')).join('')

/** Polls until `check` returns something truthy. */
async function until(label, check, timeoutMs = 90_000) {
  const t0 = Date.now()
  while (Date.now() - t0 < timeoutMs) {
    const v = await check().catch(() => null)
    if (v) return v
    await sleep(1000)
  }
  throw new Error(`timeout: ${label}`)
}
const utxos = async (address) => (await esplora(`/address/${address}/utxo`)).map((u) => ({ ...u, address }))
const confirmed = async (txid) => (await esplora(`/tx/${txid}/status`))?.confirmed

// ---------- the room ----------
const tag = Math.random().toString(36).slice(2, 5)
const people = Array.from({ length: N }, (_, i) => {
  const s = seed()
  return { name: `P${i} ${tag}`, seed: s, w: wasm.wallet_info(s) }
})

// A fast clock: one block every 3 s, like a presenter mashing "+1".
let mining = true
const clock = (async () => {
  while (mining) {
    await http('POST', '/api/admin/mine', { blocks: 1 }, { Authorization: `Bearer ${ADMIN}` }).catch((e) => errors.push(`mine: ${e.message}`))
    await sleep(3000)
  }
})()

// One WebSocket per participant plus a projector, like real browsers.
let wsMessages = 0
const sockets = Array.from({ length: N + 1 }, () => {
  const ws = new WebSocket(BASE.replace(/^http/, 'ws') + '/ws')
  ws.onmessage = () => wsMessages++
  ws.onerror = () => errors.push('websocket error')
  return ws
})

async function participant(p, i) {
  await api('POST', '/api/participants', { name: p.name, address: p.w.main_address })
  await api('POST', '/api/faucet', { address: p.w.main_address })
  await until(`${p.name} funds`, async () => (await utxos(p.w.main_address)).some((u) => u.status.confirmed))

  // Vault, a thief starts a withdrawal, the owner panics.
  const vp = { amount_sat: 200_000, hot_pubkey: p.w.hot_pubkey, cold_pubkey: p.w.cold_pubkey, delay: 5, fee_sat: 1000 }
  const fund = wasm.wallet_send(p.seed, await utxos(p.w.main_address), wasm.vault_address(vp), 200_000n)
  await broadcast(fund.hex)
  const op = `${fund.txid}:0`
  await until(`${p.name} vault confirms`, () => confirmed(fund.txid))
  await api('POST', '/api/vaults', { owner: p.name, params: vp, outpoint: op })
  const thief = people[(i + 1) % N]
  const start = wasm.vault_start(vp, op)
  await broadcast(start.hex)
  await api('POST', '/api/activity', { who: thief.name, kind: 'theft_start', outpoint: op, txid: start.txid })
  await until(`${p.name} withdrawal confirms`, () => confirmed(start.txid))
  const panic = wasm.vault_panic(vp, op)
  await broadcast(panic.hex)
  await api('POST', '/api/activity', { who: p.name, kind: 'vault_panic', outpoint: op, txid: panic.txid })

  // A subscription to the next person; the receiver collects payment 1.
  const sp = { payment_sat: 10_000, count: 3, every: 2, receiver_address: thief.w.main_address, payer_pubkey: p.w.main_pubkey, fee_sat: 500 }
  const total = BigInt(wasm.subscription_amount(sp))
  const sfund = await until(`${p.name} change for the subscription`, async () => {
    const mine = (await utxos(p.w.main_address)).filter((u) => u.status.confirmed)
    return mine.length ? wasm.wallet_send(p.seed, mine, wasm.subscription_address(sp), total) : null
  })
  await broadcast(sfund.hex)
  await until(`${p.name} subscription confirms`, () => confirmed(sfund.txid))
  await api('POST', '/api/contracts', { id: sfund.txid, kind: 'subscription', owner: p.name, params: sp, outpoint: `${sfund.txid}:0` })
  await until(`${p.name} payment 1`, async () => {
    try {
      await broadcast(wasm.subscription_pay(sp, 1, `${sfund.txid}:0`).hex)
      return true
    } catch (e) {
      if (String(e).includes('non-BIP68-final')) return false
      throw e
    }
  })
}

async function poolTable(members, t) {
  const table = await api('POST', '/api/pools', { name: `Mesa ${t} ${tag}`, host: members[0].name })
  await Promise.all(members.map((p) => api('POST', `/api/pools/${table.id}/join`, { label: p.name, pubkey: p.w.main_pubkey, address: p.w.main_address })))
  const funded = await api('POST', `/api/pools/${table.id}/start`)
  const params = { members: funded.members, share_sat: funded.share_sat, fee_sat: funded.fee_sat }
  let current = funded.outpoint
  await until(`table ${t} funds`, () => confirmed(current.split(':')[0]))
  // Everyone but the last leaves, one per block.
  const exited = []
  for (let k = 0; k < params.members.length - 1; k++) {
    const who = params.members.findIndex((m) => m.pubkey === members[k].w.main_pubkey)
    const tx = wasm.pool_exit(params, new Uint32Array(exited), who, current, members[k].seed)
    await broadcast(tx.hex)
    await until(`table ${t} exit ${k}`, () => confirmed(tx.txid))
    exited.push(who)
    current = `${tx.txid}:0`
  }
}

const t0 = performance.now()
const results = await Promise.allSettled([
  ...people.map((p, i) => participant(p, i)),
  // Tables of 5, everyone seated somewhere
  ...Array.from({ length: Math.floor(N / 5) }, (_, t) => poolTable(people.slice(t * 5, t * 5 + 5), t)),
])
mining = false
await clock
for (const ws of sockets) ws.close()

// ---------- report ----------
const failed = results.filter((r) => r.status === 'rejected')
for (const f of failed) errors.push(f.reason?.message ?? String(f.reason))
console.log(`\n${N} participants, ${Math.floor(N / 5)} pool tables, ${((performance.now() - t0) / 1000).toFixed(0)} s`)
console.log(`WebSocket messages received: ${wsMessages} over ${sockets.length} sockets`)
console.log('\nlatency (ms)              n      p50     p95     max')
for (const [label, list] of Object.entries(timings).sort()) {
  const s = [...list].sort((a, b) => a - b)
  const q = (x) => s[Math.min(s.length - 1, Math.floor(x * s.length))].toFixed(0)
  console.log(`${label.padEnd(24)} ${String(s.length).padStart(4)} ${q(0.5).padStart(8)} ${q(0.95).padStart(7)} ${s[s.length - 1].toFixed(0).padStart(7)}`)
}
console.log(errors.length ? `\n✗ ${errors.length} errors:\n  ${[...new Set(errors)].slice(0, 15).join('\n  ')}` : '\n✓ no errors')
process.exit(errors.length ? 1 : 0)
