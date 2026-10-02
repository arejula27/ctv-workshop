// UI test: inheritance and subscription with two browsers. Run with infra/scripts/test-ui.sh.
import { chromium } from 'playwright'
// Same host and ports as a participant's browser (the explorer reads the API from :8080)
const BASE = process.env.BASE_URL ?? 'http://localhost:8080'
const shot = (p, name) => p.screenshot({ path: `/work/e2e/shots/${name}.png`, fullPage: true })
const mine = async (n = 1) => {
  const r = await fetch(`${BASE}/api/admin/jump`, { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }, body: JSON.stringify({ blocks: n }) })
  return (await r.json()).height
}
const tag = Math.random().toString(36).slice(2, 6)
const ANA = `Ana ${tag}`, BEA = `Bea ${tag}`
const setDemo = (demo) =>
  fetch(`${BASE}/api/admin/demo`, { method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }, body: JSON.stringify({ demo }) })
// The test counts blocks exactly: pause the automatic clock and mine by hand.
const miner = (paused) =>
  fetch(`${BASE}/api/admin/miner`, { method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }, body: JSON.stringify({ paused }) })
await miner(true)
const browser = await chromium.launch()
const ctx = () => browser.newContext({ viewport: { width: 1100, height: 900 } })
const ana = await (await ctx()).newPage(), bea = await (await ctx()).newPage()
for (const p of [ana, bea]) p.on('pageerror', (e) => console.log('PAGE ERROR', e.message))

// On failure, keep a screenshot of every browser to see what went wrong.
try {

  async function onboard(p, name) {
    await p.goto(BASE + '/')
    await p.getByPlaceholder('Tu nombre').fill(name)
    await p.getByRole('button', { name: 'Entrar' }).click()
    await p.getByRole('heading', { name: `Hola, ${name}` }).waitFor()
  }
  await onboard(ana, ANA); await onboard(bea, BEA)
  // Receive → /faucet, like a real wallet
  await ana.getByRole('button', { name: 'Recibir' }).click()
  const address = (await ana.locator('.address').innerText()).trim()
  await ana.getByRole('button', { name: 'Cerrar' }).click()
  const f = await ana.context().newPage()
  await f.goto(BASE + '/faucet')
  await f.getByLabel('Dirección').fill(address)
  await f.getByRole('button', { name: 'Enviarme bitcoin de prueba' }).click()
  await f.getByText(/Enviados/).waitFor()
  await f.close()
  await mine(1)
  await ana.locator('section.wallet .amount', { hasText: '1.000.000 sats' }).waitFor({ timeout: 20000 }) // indexed and spendable

  // ---------- inheritance ----------
  await setDemo('inheritance')
  const screen = await (await browser.newContext({ viewport: { width: 1600, height: 900 } })).newPage()
  await screen.goto(BASE + '/screen')
  await screen.getByRole('heading', { name: /Herencia/ }).waitFor({ timeout: 15000 })
  await ana.getByRole('link', { name: 'Herencia' }).click()
  await ana.getByLabel('Dejar en herencia (sats)').fill('300000')
  await ana.getByLabel('Se reparte si no la mueves en (bloques)').fill('5')
  await ana.getByLabel(BEA).check()
  await ana.getByRole('button', { name: 'Dejar herencia' }).click()
  await ana.getByText('Guardando la herencia').waitFor({ timeout: 15000 })
  await mine(1)
  await ana.getByText(/Si no das señales de vida en \d+ bloques/).waitFor({ timeout: 15000 })
  await bea.getByRole('link', { name: 'Herencia' }).click()
  await bea.getByText(`Herencia de ${ANA}`).waitFor({ timeout: 15000 })
  console.log('✓ inheritance created; the heir sees it')

  await mine(3)
  await ana.getByRole('button', { name: '💓 Sigo viva' }).click()
  // the renewed inheritance (300.000 minus one fee) must be broadcast before the block
  await ana.getByText('299.000 sats · se reparte').waitFor({ timeout: 15000 })
  await mine(1)
  await ana.getByText(/Si no das señales de vida en 4 bloques/).waitFor({ timeout: 15000 })
  await bea.getByText(new RegExp(`Si ${ANA} no da señales de vida en 4 bloques`)).waitFor({ timeout: 15000 })
  await shot(ana, 'c1-inheritance-alive')
  console.log('✓ "I\'m alive" restarts the clock, for the heir too')

  await mine(4)
  await bea.getByText('¡Plazo cumplido!').waitFor({ timeout: 15000 })
  await bea.getByRole('button', { name: '📜 Ejecutar el reparto' }).click()
  await mine(1)
  await bea.getByText('Herencia repartida').waitFor({ timeout: 15000 })
  await ana.getByText('Herencia repartida').waitFor({ timeout: 15000 })
  await shot(bea, 'c2-inheritance-heir')
  await screen.locator('.tile', { hasText: ANA }).getByText('repartida').waitFor({ timeout: 15000 })
  await shot(screen, 'c2b-screen-inheritance')
  console.log('✓ deadline passed; the heir executed the split')

  // ---------- subscription ----------
  await setDemo('subscription')
  await screen.getByRole('heading', { name: /Suscripción/ }).waitFor({ timeout: 15000 })
  await ana.getByRole('link', { name: 'Suscripción' }).click()
  await ana.getByLabel('¿A quién?').selectOption({ label: BEA })
  await ana.getByLabel('Cada pago (sats)').fill('10000')
  await ana.getByLabel('Número de pagos').fill('3')
  await ana.getByLabel('Cada (bloques)').fill('2')
  await ana.getByRole('button', { name: 'Crear suscripción' }).click()
  await ana.getByText('Reservando el dinero').waitFor({ timeout: 15000 })
  await mine(1)
  await bea.getByRole('link', { name: 'Suscripción' }).click()
  await bea.getByText(`${ANA} te paga`).waitFor({ timeout: 15000 })
  await mine(1)
  // Bea's app collects payment 1 by itself once it's due.
  await bea.getByText(/Cobrados 10\.000 sats/).waitFor({ timeout: 40000 })
  await mine(1)
  await ana.getByText(/Cobrados 10\.000 sats/).waitFor({ timeout: 15000 })
  console.log('✓ subscription: payment 1 collected automatically')

  await ana.getByRole('button', { name: '✋ Cancelar suscripción' }).click()
  await mine(1)
  await ana.getByText('Suscripción cancelada tras 1 pago').waitFor({ timeout: 15000 })
  await bea.getByText('Suscripción cancelada tras 1 pago').waitFor({ timeout: 15000 })
  await shot(bea, 'c3-subscription-cancelled')
  await screen.locator('.tile', { hasText: `${ANA} → ${BEA}` }).getByText('cancelada').waitFor({ timeout: 15000 })
  await shot(screen, 'c3b-screen-subscription')
  console.log('✓ payer cancelled; both see it')
} catch (e) {
  console.log('ANA_STORAGE', JSON.stringify(await ana.evaluate(() => ({ ...localStorage })).catch(() => ({}))))
  await shot(ana, 'FAIL-ana').catch(() => {})
  await shot(bea, 'FAIL-bea').catch(() => {})
  throw e
} finally {
  await miner(false)
}
await browser.close()
