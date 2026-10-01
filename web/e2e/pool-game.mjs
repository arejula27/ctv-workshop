// UI test: a payment pool with three browsers. Run with infra/scripts/test-ui.sh.
import { chromium } from 'playwright'
const BASE = 'http://caddy'
const shot = (p, name) => p.screenshot({ path: `/work/e2e/shots/${name}.png`, fullPage: true })
const mine = async (n = 1) => {
  const r = await fetch(`${BASE}/api/admin/jump`, { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }, body: JSON.stringify({ blocks: n }) })
  return (await r.json()).height
}
const tag = Math.random().toString(36).slice(2, 6)
const ANA = `Ana ${tag}`, BEA = `Bea ${tag}`, CARLA = `Carla ${tag}`, TABLE = `Mesa ${tag}`
const browser = await chromium.launch()
const ctx = () => browser.newContext({ viewport: { width: 1100, height: 900 } })
const [ana, bea, carla] = await Promise.all([1, 2, 3].map(async () => (await ctx()).newPage()))
const all = [ana, bea, carla]
for (const p of all) p.on('pageerror', (e) => console.log('PAGE ERROR', e.message))

async function onboard(p, name) {
  await p.goto(BASE + '/')
  await p.getByPlaceholder('Tu nombre').fill(name)
  await p.getByRole('button', { name: 'Crear mi cartera' }).click()
  await p.getByRole('heading', { name: `Hola, ${name}` }).waitFor()
  await p.getByRole('link', { name: 'Pool' }).click()
}

try {
  await onboard(ana, ANA); await onboard(bea, BEA); await onboard(carla, CARLA)

  await ana.getByPlaceholder('Nombre de una mesa nueva').fill(TABLE)
  await ana.getByRole('button', { name: 'Abrir mesa' }).click()
  const tableIn = (p) => p.locator('.table', { hasText: TABLE })
  for (const p of [bea, carla]) await tableIn(p).getByRole('button', { name: 'Sentarme' }).click()
  await tableIn(ana).getByText(`${ANA}, ${BEA}, ${CARLA}`).waitFor({ timeout: 15000 })
  console.log('✓ three people at the table')

  await tableIn(bea).getByRole('button', { name: '¡Empezar!' }).click()
  await bea.getByText('Creando el UTXO compartido').waitFor({ timeout: 15000 }) // funded before mining
  await mine(1)
  for (const p of all) await p.getByText('UTXO compartido · 300.000 sats').waitFor({ timeout: 15000 })
  await shot(carla, 'p1-pool-funded')
  console.log('✓ the house funded one shared UTXO; everyone sees it')

  // Carla tries to push Ana out: impossible without Ana's signature.
  await carla.locator('.chip.in', { hasText: ANA }).getByRole('button', { name: 'Sacar' }).click()
  await carla.getByText(`No puedes sacar a ${ANA}`).waitFor()
  console.log('✓ nobody can push someone else out')

  await bea.getByRole('button', { name: '🚪 Salir' }).click()
  await mine(1)
  for (const p of all) await p.getByText('UTXO compartido · 200.000 sats').waitFor({ timeout: 15000 })
  await ana.locator('.chip.gone', { hasText: BEA }).getByText('99.500 sats').waitFor()
  console.log('✓ Bea left without asking; Ana and Carla stay inside')

  await ana.getByRole('button', { name: '🚪 Salir' }).click()
  await mine(1)
  for (const p of all) await p.getByText('El pool se ha disuelto').waitFor({ timeout: 15000 })
  await carla.locator('.chip.gone', { hasText: CARLA }).getByText('100.000 sats').waitFor()
  await carla.getByRole('button', { name: /Ver árbol/ }).click()
  await shot(carla, 'p2-pool-dissolved')
  console.log('✓ Ana left: the pool dissolved and Carla (last) got her full share')
} catch (e) {
  for (const [i, p] of all.entries()) await shot(p, `FAIL-pool-${i}`).catch(() => {})
  throw e
}
await browser.close()
