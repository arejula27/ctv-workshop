// UI test: run with infra/scripts/test-ui.sh (Playwright in Docker, on the stack's network).
// Two browsers play the vault game through the real UI: Ana owns a vault, Bea steals her hot key.
import { chromium } from 'playwright'
const BASE = 'http://caddy'
const shot = (p, name) => p.screenshot({ path: `/work/e2e/shots/${name}.png`, fullPage: true })
const mine = async (n = 1) => {
  const r = await fetch(`${BASE}/api/admin/jump`, { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }, body: JSON.stringify({ blocks: n }) })
  return (await r.json()).height
}
// Unique names: the room feed keeps history across runs.
const tag = Math.random().toString(36).slice(2, 6)
const ANA = `Ana ${tag}`, BEA = `Bea ${tag}`
const browser = await chromium.launch()
const ctx = () => browser.newContext({ viewport: { width: 1100, height: 900 } })
const ana = await (await ctx()).newPage(), bea = await (await ctx()).newPage()
for (const p of [ana, bea]) p.on('pageerror', (e) => console.log('PAGE ERROR', e.message))

async function onboard(p, name) {
  await p.goto(BASE + '/')
  await p.getByPlaceholder('Tu nombre').fill(name)
  await p.getByRole('button', { name: 'Crear mi cartera' }).click()
  await p.getByRole('heading', { name: `Hola, ${name}` }).waitFor()
}
await onboard(ana, ANA); await onboard(bea, BEA)
await ana.getByRole('button', { name: 'Pedir sats al faucet' }).click()
await ana.getByText('¡Recibidos').waitFor()
await mine(1)
await ana.getByText('1.000.000 sats').first().waitFor({ timeout: 15000 })
await shot(ana, '1-wallet')
console.log('✓ onboarding + faucet')

await ana.getByRole('link', { name: 'Vault' }).click()
await ana.getByLabel('Guardar en un vault (sats)').fill('500000')
await ana.getByLabel('Espera para volver aquí (bloques)').fill('5')
await ana.getByRole('button', { name: 'Crear vault' }).click()
await ana.getByText('Guardando el dinero').waitFor({ timeout: 15000 })
await mine(1)
await ana.getByText('Solo puede salir por uno de estos dos caminos').waitFor({ timeout: 15000 })
await shot(ana, '2-vault-locked')
console.log('✓ vault created and confirmed')

await ana.getByRole('button', { name: /Dejar que me roben/ }).click()
const hotKey = await ana.locator('.kit textarea').inputValue()
await bea.getByRole('link', { name: 'Vault' }).click()
// Only the hot key: Bea's app finds Ana's vault through the room registry.
await bea.getByPlaceholder('La llave caliente robada (64 caracteres)').fill(hotKey)
await bea.getByRole('button', { name: 'Usar la llave robada' }).click()
await bea.getByText(`Vault de ${ANA}`).waitFor()
await bea.getByText(`Tienes la llave caliente de ${ANA}`).waitFor({ timeout: 15000 })
await shot(bea, '3-thief-ready')
await bea.locator('article.thief').getByRole('button', { name: 'Iniciar retiro' }).click()
await mine(1)
await bea.getByText(/Faltan \d+ bloques/).waitFor({ timeout: 15000 })
const stealBtn = bea.getByRole('button', { name: 'Llevármelo' })
console.log('✓ thief started withdrawal; steal button disabled:', await stealBtn.isDisabled())

await ana.getByText(/Faltan \d+ bloques/).waitFor({ timeout: 15000 })
await ana.getByRole('button', { name: /Ver árbol/ }).click()
await shot(ana, '4-owner-alarm')
await ana.getByRole('button', { name: '🚨 Botón del pánico' }).click()
await mine(1)
await ana.getByText('Pánico pulsado a tiempo').waitFor({ timeout: 15000 })
await shot(ana, '5-owner-safe')
await bea.getByText('pulsó el pánico a tiempo').waitFor({ timeout: 15000 })
await shot(bea, '6-thief-failed')
console.log('✓ owner panicked in time; thief sees it failed')

// Projector: the board and the feed tell the story.
const screen = await (await browser.newContext({ viewport: { width: 1600, height: 900 } })).newPage()
await screen.goto(BASE + '/screen')
await screen.getByText(`¡${ANA} ha pulsado el pánico!`).waitFor({ timeout: 15000 })
await screen.getByText(`¡${BEA} intenta robar el vault de ${ANA}!`).first().waitFor()
await screen.locator('.vault', { hasText: ANA }).getByText('salvado con el pánico').waitFor({ timeout: 15000 })
console.log('✓ /screen shows the board and the feed')

// Round 2: Ana doesn't react, Bea gets away with it. The presenter jumps the clock from /admin.
await ana.getByRole('button', { name: 'Archivar' }).click()
await ana.getByLabel('Guardar en un vault (sats)').fill('300000')
await ana.getByLabel('Espera para volver aquí (bloques)').fill('3')
await ana.getByRole('button', { name: 'Crear vault' }).click()
await ana.getByText('Guardando el dinero').waitFor({ timeout: 15000 }) // broadcast done before mining
await mine(1)
await ana.getByText('Solo puede salir por uno de estos dos caminos').waitFor({ timeout: 15000 })
await bea.getByPlaceholder('La llave caliente robada (64 caracteres)').fill(hotKey)
await bea.getByRole('button', { name: 'Usar la llave robada' }).click()
const round2 = bea.locator('article.thief', { hasText: '300.000 sats' })
await round2.getByRole('button', { name: 'Iniciar retiro' }).click()
await mine(1)
await round2.getByText(/Faltan \d+ bloques/).waitFor({ timeout: 15000 })
const adminPage = await (await ctx()).newPage()
await adminPage.goto(BASE + '/admin')
await adminPage.getByPlaceholder('ADMIN_TOKEN de infra/.env').fill(process.env.ADMIN_TOKEN)
await adminPage.getByRole('button', { name: '+5 bloques' }).click()
await round2.getByText('¡Tiempo cumplido!').waitFor({ timeout: 15000 })
await round2.getByRole('button', { name: 'Llevármelo' }).click()
await round2.getByText('¡Robo completado!').waitFor({ timeout: 15000 })
await ana.getByText('Te han robado').waitFor({ timeout: 15000 })
await screen.getByText(`${BEA} ha robado 300.000 sats a ${ANA}`).waitFor({ timeout: 15000 })
await screen.locator('.vault', { hasText: ANA }).getByText('¡robado!').waitFor({ timeout: 15000 })
await shot(screen, '8-screen')
await shot(ana, '9-owner-robbed')
console.log('✓ round 2: admin jump, theft succeeds, owner and screen see it')

// The header date follows the simulated calendar: +144 blocks = +1 day.
const before = await ana.locator('header .date').first().innerText()
await adminPage.getByRole('spinbutton').fill('144')
await adminPage.getByRole('button', { name: 'Saltar 144 bloques' }).click()
await ana.waitForFunction((b) => document.querySelector('header .date')?.textContent !== b, before, { timeout: 15000 })
await ana.waitForTimeout(3000) // the jump lands in batches; let the header settle
const after = await ana.locator('header .date').first().innerText()
await ana.locator('header').first().screenshot({ path: '/work/e2e/shots/10-header-after-jump.png' })
console.log(`✓ calendar jump: ${before}  →  ${after}`)

const phone = await (await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 })).newPage()
await phone.goto(BASE + '/')
await phone.evaluate((s) => Object.entries(s).forEach(([k, v]) => localStorage.setItem(k, v)), await ana.evaluate(() => ({ ...localStorage })))
await phone.goto(BASE + '/vault')
await phone.getByText('Te han robado').waitFor({ timeout: 15000 })
await shot(phone, '7-phone')
console.log('✓ phone layout')
await browser.close()
