// UI test: the vault game as real wallets. Run with infra/scripts/test-ui.sh.
// Ana and Carla protect their money; Bea is the thief with everyone's leaked hot seeds.
import { chromium } from 'playwright'
const BASE = 'http://caddy'
const H = { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }
const shot = (p, name) => p.screenshot({ path: `/work/e2e/shots/${name}.png`, fullPage: true })
const jump = (blocks = 1) => fetch(`${BASE}/api/admin/jump`, { method: 'POST', headers: H, body: JSON.stringify({ blocks }) })
await fetch(`${BASE}/api/admin/demo`, { method: 'PUT', headers: H, body: JSON.stringify({ demo: 'vault' }) })

const tag = Math.random().toString(36).slice(2, 6)
const ANA = `Ana ${tag}`, BEA = `Bea ${tag}`, CARLA = `Carla ${tag}`
const browser = await chromium.launch()
const page = async (w = 1200, h = 900) => (await browser.newContext({ viewport: { width: w, height: h } })).newPage()
const [ana, bea, carla, screen] = [await page(), await page(), await page(), await page(1600, 900)]
const all = { ana, bea, carla, screen }
for (const p of Object.values(all)) p.on('pageerror', (e) => console.log('PAGE ERROR', e.message))

const wallet = (p, title) => p.locator('section.wallet', { hasText: title })

async function enter(p, name, role) {
  await p.goto(BASE + '/')
  await p.getByPlaceholder('Tu nombre').fill(name)
  await p.getByRole('button', { name: 'Entrar' }).click()
  await p.getByRole('link', { name: 'Vault' }).click()
  await p.getByRole('button', { name: role }).click()
}

/** Receive → copy the address → paste it in /faucet (in another tab) → mine. */
async function fund(p, title) {
  await wallet(p, title).getByRole('button', { name: 'Recibir' }).click()
  const address = (await p.locator('.address').innerText()).trim()
  await p.getByRole('button', { name: 'Cerrar' }).click()
  const f = await p.context().newPage()
  await f.goto(BASE + '/faucet')
  await f.getByLabel('Dirección').fill(address)
  await f.getByRole('button', { name: 'Enviarme bitcoin de prueba' }).click()
  await f.getByText(/Enviados 1\.000\.000 sats/).waitFor()
  await f.close()
  await jump(1)
  await wallet(p, title).locator('.amount', { hasText: '1.000.000 sats' }).waitFor({ timeout: 20000 })
}

async function moveToVault(p, sats, blocks) {
  await wallet(p, 'Cartera caliente').getByRole('button', { name: 'Mover al vault' }).click()
  await p.getByLabel('Cantidad (sats)').fill(String(sats))
  await p.getByLabel(/Espera para volver/).fill(String(blocks))
  await p.locator('form').getByRole('button', { name: /al vault/ }).click()
  await p.getByText('Guardando el dinero en el vault').waitFor({ timeout: 15000 })
  await jump(1)
  await p.locator('.card .badge', { hasText: 'Protegido' }).waitFor({ timeout: 20000 })
}

try {
  await screen.goto(BASE + '/screen')

  // Carla creates her hot wallet first, so Ana can pick it as her safe box.
  await enter(carla, CARLA, /Protege tu dinero/)
  await carla.getByRole('button', { name: 'Crear cartera' }).click()
  await wallet(carla, 'Cartera caliente').waitFor()

  await enter(ana, ANA, /Protege tu dinero/)
  await ana.getByRole('button', { name: 'Crear cartera' }).click()
  await ana.getByLabel('Caja fuerte').selectOption({ label: `La cartera de ${CARLA}` })
  await ana.getByRole('button', { name: 'Configurar caja fuerte' }).click()
  await ana.getByRole('heading', { name: `La cartera de ${CARLA}` }).waitFor()
  await fund(ana, 'Cartera caliente')
  console.log('✓ wallets created; funded from /faucet with the address from "Recibir"')

  await moveToVault(ana, 500000, 5)
  await shot(ana, '1-owner')
  console.log('✓ moved 500.000 sats to a vault whose safe box is Carla\'s wallet')

  await enter(bea, BEA, /Roba lo que puedas/)
  await bea.getByRole('button', { name: 'Crear cartera' }).click()
  const victim = bea.locator('section.victim', { hasText: `Cartera caliente de ${ANA}` })
  await victim.getByRole('button', { name: 'Vaciar cartera' }).click()
  await jump(1)
  await wallet(ana, 'Cartera caliente').locator('.amount', { hasText: /^0 sats/ }).waitFor({ timeout: 20000 })
  await wallet(bea, 'Tu cartera').locator('.amount', { hasText: /\d{3}\.\d{3} sats/ }).waitFor({ timeout: 20000 })
  await shot(bea, '2-thief')
  console.log('✓ the thief emptied Ana\'s hot wallet with the leaked seed')

  // The history links each transaction to the explorer.
  const link = await wallet(ana, 'Cartera caliente').locator('li a').first().getAttribute('href')
  if (!/\/tx\/[0-9a-f]{64}$/.test(link ?? '')) throw new Error(`bad explorer link: ${link}`)
  console.log('✓ history entries link to the explorer')

  await victim.locator('.card').getByRole('button', { name: 'Iniciar retiro' }).click()
  await jump(1)
  await ana.locator('.card .badge', { hasText: 'Retiro en curso' }).waitFor({ timeout: 20000 })
  const panicBtn = ana.getByRole('button', { name: 'Botón del pánico' })
  await panicBtn.click()
  await panicBtn.click({ force: true, timeout: 2000 }).catch(() => {}) // a second click must change nothing
  await jump(1)
  await ana.locator('.card .badge', { hasText: 'En la caja fuerte' }).waitFor({ timeout: 20000 })
  await wallet(carla, 'Cartera caliente').locator('.amount', { hasText: '498.000 sats' }).waitFor({ timeout: 20000 })
  await victim.getByText('a salvo gracias al pánico').waitFor({ timeout: 20000 })
  console.log('✓ Ana panicked: the money went to Carla\'s wallet; Bea sees it failed')

  await screen.locator('.tile', { hasText: ANA }).getByText('salvado con el pánico').waitFor({ timeout: 20000 })
  await screen.getByText(`${BEA} ha vaciado la cartera caliente de ${ANA}`).waitFor()
  const panics = await screen.getByText(`¡${ANA} ha pulsado el pánico!`).count()
  if (panics !== 1) throw new Error(`the panic shows ${panics} times on /screen`)
  await shot(screen, '3-screen')
  console.log('✓ /screen shows each action exactly once')

  // Round 2: Carla (now with 498.000 sats) uses a new safe box of her own, and doesn't react.
  await carla.getByLabel('Caja fuerte').selectOption({ label: 'Crear una caja fuerte nueva' })
  await carla.getByRole('button', { name: 'Configurar caja fuerte' }).click()
  await moveToVault(carla, 300000, 3)
  const victim2 = bea.locator('section.victim', { hasText: `Cartera caliente de ${CARLA}` })
  await victim2.locator('.card').getByRole('button', { name: 'Iniciar retiro' }).click()
  await jump(1)
  await victim2.locator('.card .badge', { hasText: 'Retiro en curso' }).waitFor({ timeout: 20000 })
  await jump(3)
  await victim2.getByRole('button', { name: 'Llevármelo' }).click()
  await jump(1)
  await carla.locator('.card .badge', { hasText: 'Vaciado' }).waitFor({ timeout: 20000 })
  await carla.getByText('sin que fueras tú').waitFor()
  await screen.locator('.tile', { hasText: CARLA }).getByText('¡robado!').waitFor({ timeout: 20000 })
  await shot(carla, '4-robbed')
  console.log('✓ round 2: Carla didn\'t react and Bea took the vault')

  const phone = await page(390, 844)
  await phone.goto(BASE + '/')
  await phone.evaluate((s) => Object.entries(s).forEach(([k, v]) => localStorage.setItem(k, v)), await ana.evaluate(() => ({ ...localStorage })))
  await phone.goto(BASE + '/vault')
  await phone.locator('.card .badge', { hasText: 'En la caja fuerte' }).waitFor({ timeout: 20000 })
  await shot(phone, '5-phone')
  console.log('✓ phone layout')
} catch (e) {
  for (const [n, p] of Object.entries(all)) await shot(p, `FAIL-vault-${n}`).catch(() => {})
  throw e
}
await browser.close()
