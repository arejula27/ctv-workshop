// workshop-server public API.
export async function faucet(address: string): Promise<{ txid: string; amount_sat: number }> {
  const r = await fetch('/api/faucet', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ address }),
  })
  const body = await r.json()
  if (!r.ok) throw new Error(body.error ?? `faucet: ${r.status}`)
  return body
}
