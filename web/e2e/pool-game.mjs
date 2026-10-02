// UI test: the payment pool as a wallet with an on-chain and an instant account.
// Run with infra/scripts/test-ui.sh.
import { chromium } from 'playwright'
// Same host and ports as a participant's browser (the explorer reads the API from :8080)
const BASE = process.env.BASE_URL ?? 'http://localhost:8080'
const H = { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }
const shot = (p, name) => p.screenshot({ path: `/work/e2e/shots/${name}.png`, fullPage: true })
const mine = (blocks = 1) => fetch(`${BASE}/api/admin/jump`, { method: 'POST', headers: H, body: JSON.stringify({ blocks }) })
await fetch(`${BASE}/api/admin/demo`, { method: 'PUT', headers: H, body: JSON.stringify({ demo: 'pool' }) })

const tag = Math.random().toString(36).slice(2, 6)
const ANA = `Ana ${tag}`, BEA = `Bea ${tag}`, CARLA = `Carla ${tag}`, TABLE = `Mesa ${tag}`
const browser = await chromium.launch()
const page = async (w = 1200, h = 900) => (await browser.newContext({ viewport: { width: w, height: h } })).newPage()
const [ana, bea, carla, screen] = [await page(), await page(), await page(), await page(1600, 900)]
const all = { ana, bea, carla, screen }
for (const p of Object.values(all)) p.on('pageerror', (e) => console.log('PAGE ERROR', e.message))

const onchain = (p) => p.locator('section.wallet', { hasText: 'Cuenta on-chain' })
const instant = (p) => p.locator('section.instant')
const balance = (p) => instant(p).locator('.amount')

async function enter(p, name) {
  await p.goto(BASE + '/')
  await p.getByPlaceholder('Tu nombre').fill(name)
  await p.getByRole('button', { name: 'Entrar' }).click()
  await p.getByRole('link', { name: 'Pool' }).click()
  await p.getByRole('button', { name: 'Crear cartera' }).click()
}

/** Whoever proposes, wait until every app has signed and it's on chain. */
async function settle(text) {
  for (const p of [ana, bea, carla]) await instant(p).locator('.pending').waitFor({ state: 'detached', timeout: 20000 })
  await mine(1)
  await screen.getByText(text).first().waitFor({ timeout: 20000 })
}

try {
  await screen.goto(BASE + '/screen')
  await enter(ana, ANA); await enter(bea, BEA); await enter(carla, CARLA)

  await ana.getByPlaceholder('Nombre de una mesa nueva').fill(TABLE)
  await ana.getByRole('button', { name: 'Abrir mesa' }).click()
  const table = (p) => p.locator('.table', { hasText: TABLE })
  for (const p of [bea, carla]) await table(p).getByRole('button', { name: 'Sentarme' }).click()
  await table(ana).getByText(`${ANA}, ${BEA}, ${CARLA}`).waitFor({ timeout: 15000 })
  await table(bea).getByRole('button', { name: 'Empezar' }).click()
  for (const p of [ana, bea, carla]) await balance(p).getByText('100.000 sats').waitFor({ timeout: 20000 })
  await mine(1)
  console.log('✓ table started: everyone has 100.000 sats in the instant account')

  // Ana pays Bea inside the pool; Bea and Carla's apps check and sign by themselves.
  await instant(ana).getByRole('button', { name: 'Enviar' }).click()
  await ana.getByLabel('A quién').selectOption({ label: `${BEA} (dentro del pool)` })
  await ana.getByLabel('Cantidad (sats)').fill('20000')
  await ana.locator('form').getByRole('button', { name: 'Enviar' }).click()
  await settle(`${ANA} ha pagado 20.000 sats a ${BEA}`)
  await balance(ana).getByText('79.500 sats').waitFor({ timeout: 15000 })
  await balance(bea).getByText('120.000 sats').waitFor({ timeout: 15000 })
  await shot(ana, 'p1-paid')
  console.log('✓ instant payment: all three apps signed; Ana 79.500, Bea 120.000')

  // Each state of the pool links to the explorer.
  const links = await instant(ana).locator('.history a').evaluateAll((as) => as.map((a) => a.getAttribute('href')))
  if (links.length < 2 || !links.every((h) => /\/tx\/[0-9a-f]{64}$/.test(h))) throw new Error(`pool history links: ${links}`)
  // …and the explorer really opens it (it reads the API cross-origin).
  const explorer = await page()
  const txid = links[0].split('/tx/')[1]
  // A just-broadcast tx can take a moment to reach the index: reload like a person would.
  let found = false
  for (let n = 0; n < 10 && !found; n++) {
    await explorer.goto(links[0])
    found = await explorer
      .waitForFunction((t) => document.body.innerText.includes(t), txid, { timeout: 3000 })
      .then(() => true, () => false)
  }
  if (!found) throw new Error(`the explorer doesn't open ${txid}: ${(await explorer.locator('body').innerText()).slice(0, 200)}`)
  await explorer.close()
  console.log('✓ every state of the pool links to the explorer, and it opens')

  // Carla funds her on-chain account and deposits 50.000 into the pool.
  await onchain(carla).getByRole('button', { name: 'Recibir' }).click()
  const addr = (await carla.locator('.address').innerText()).trim()
  await carla.getByRole('button', { name: 'Cerrar' }).click()
  await fetch(`${BASE}/api/faucet`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ address: addr }) })
  await mine(1)
  await onchain(carla).locator('.amount', { hasText: '1.000.000 sats' }).waitFor({ timeout: 20000 })
  await onchain(carla).getByRole('button', { name: 'Meter en el pool' }).click()
  await carla.getByLabel('Cantidad (sats)').fill('50000')
  await carla.locator('form').getByRole('button', { name: 'Meter en el pool' }).click()
  await settle(`${CARLA} ha metido 50.000 sats`)
  await balance(carla).getByText('150.000 sats').waitFor({ timeout: 15000 })
  console.log('✓ deposit from the on-chain account: Carla 150.000 in the pool')

  // Bea sends 30.000 from the pool to her own on-chain account.
  await onchain(bea).getByRole('button', { name: 'Recibir' }).click()
  const beaAddr = (await bea.locator('.address').innerText()).trim()
  await bea.getByRole('button', { name: 'Cerrar' }).click()
  await instant(bea).getByRole('button', { name: 'Enviar' }).click()
  await bea.getByLabel('A quién').selectOption({ label: 'Una dirección on-chain…' })
  await bea.locator('.dialog').getByLabel('Dirección', { exact: true }).fill(beaAddr)
  await bea.getByLabel('Cantidad (sats)').fill('30000')
  await bea.locator('form').getByRole('button', { name: 'Enviar' }).click()
  await settle(`${BEA} ha enviado 30.000 sats desde el pool`)
  await balance(bea).getByText('89.500 sats').waitFor({ timeout: 15000 })
  await onchain(bea).locator('.amount', { hasText: '30.000 sats' }).waitFor({ timeout: 20000 })
  console.log('✓ withdrawal to an on-chain address: Bea 89.500 in the pool, 30.000 on-chain')

  await screen.locator('.tile', { hasText: TABLE }).getByText('3 movimientos').waitFor({ timeout: 15000 })
  await shot(screen, 'p2-screen')

  // Bea leaves on her own, with her current balance (not the initial 100.000).
  await instant(bea).getByRole('button', { name: 'Salir del pool' }).click()
  await bea.getByText('Te llevas 89.000 sats').waitFor()
  await bea.locator('.dialog').getByRole('button', { name: 'Salir del pool' }).click()
  await mine(1)
  await onchain(bea).locator('.amount', { hasText: '119.000 sats' }).waitFor({ timeout: 20000 })
  await instant(ana).locator('.member', { hasText: BEA }).waitFor({ state: 'detached', timeout: 15000 })
  console.log('✓ Bea left without asking anyone, with 89.000 (her current balance minus the fee)')

  // Ana leaves: the pool dissolves and Carla, last, gets her full balance.
  await instant(ana).getByRole('button', { name: 'Salir del pool' }).click()
  await ana.locator('.dialog').getByRole('button', { name: 'Salir del pool' }).click()
  await mine(1)
  await screen.getByText(`El pool «${TABLE}» se ha disuelto`).waitFor({ timeout: 20000 })
  await onchain(carla).locator('.amount', { hasText: /1\.099\.\d{3} sats/ }).waitFor({ timeout: 20000 })
  await shot(carla, 'p3-dissolved')
  console.log('✓ Ana left: the pool dissolved and Carla got her 150.000 on-chain')
} catch (e) {
  for (const [n, p] of Object.entries(all)) await shot(p, `FAIL-pool-${n}`).catch(() => {})
  throw e
}
await browser.close()
