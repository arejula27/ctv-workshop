// UI test: run with infra/scripts/test-ui.sh (Playwright in Docker, on the stack's network).
// Two browsers play the vault game through the real UI: Ana owns a vault, Bea steals her hot key.
import { chromium } from 'playwright'
const BASE = 'http://caddy'
const shot = (p, name) => p.screenshot({ path: `/work/e2e/shots/${name}.png`, fullPage: true })
const mine = async (n = 1) => {
  const r = await fetch(`${BASE}/api/admin/jump`, { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.ADMIN_TOKEN}` }, body: JSON.stringify({ blocks: n }) })
  return (await r.json()).height
}
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
await onboard(ana, 'Ana'); await onboard(bea, 'Bea')
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
const kit = await ana.locator('.kit textarea').inputValue()
await bea.getByRole('link', { name: 'Vault' }).click()
await bea.getByPlaceholder('ctvkit:…').fill(kit)
await bea.getByRole('button', { name: 'Usar la llave robada' }).click()
await bea.getByText('Vault de Ana').waitFor()
await bea.getByText('Tienes la llave caliente de Ana').waitFor({ timeout: 15000 })
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

const phone = await (await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 })).newPage()
await phone.goto(BASE + '/')
await phone.evaluate((s) => Object.entries(s).forEach(([k, v]) => localStorage.setItem(k, v)), await ana.evaluate(() => ({ ...localStorage })))
await phone.goto(BASE + '/vault')
await phone.getByText('Pánico pulsado a tiempo').waitFor({ timeout: 15000 })
await shot(phone, '7-phone')
console.log('✓ phone layout')
await browser.close()
