// Feed events as short sentences for the room (and the projector).
import type { FeedEvent } from './events'
import { sats } from './format'

export function describe(e: FeedEvent): { icon: string; text: string; tone?: 'hot' | 'cold' | 'thief' } {
  switch (e.type) {
    case 'participant_joined':
      return { icon: '👋', text: `${e.name} se ha unido` }
    case 'faucet':
      return { icon: '🚰', text: `Alguien ha pedido ${sats(e.amount_sat)} al faucet` }
    case 'vault_registered':
      return { icon: '🔒', text: `${e.owner} ha guardado ${sats(e.amount_sat)} en un vault (espera de ${e.delay} bloques)` }
    case 'jump':
      return { icon: '⏩', text: `Salto en el tiempo: +${e.blocks} ${e.blocks === 1 ? 'bloque' : 'bloques'}` }
    case 'miner':
      return e.settings.paused
        ? { icon: '⏸', text: 'Reloj en pausa' }
        : { icon: '▶', text: `Un bloque cada ${e.settings.interval_s} s` }
    case 'contract_registered':
      if (e.kind === 'subscription')
        return { icon: '📆', text: `${e.owner} ha creado una suscripción de ${sats(e.amount_sat)}` }
      return e.renewed
        ? { icon: '💓', text: `${e.owner} sigue viva: su herencia vuelve a empezar` }
        : { icon: '📜', text: `${e.owner} ha dejado una herencia de ${sats(e.amount_sat)}` }
    case 'pool_updated':
      if (e.change === 'created') return { icon: '🏊', text: `Nueva mesa de pool: «${e.name}»` }
      if (e.change === 'joined')
        return { icon: '🪑', text: `${e.members[e.members.length - 1]} se ha sentado en la mesa «${e.name}»` }
      return { icon: '🏊', text: `La mesa «${e.name}» comparte ahora un solo UTXO de ${sats(e.amount_sat)} entre ${e.members.length}` }
    case 'activity':
      switch (e.kind) {
        case 'pool_exit':
          return { icon: '🚪', text: `${e.who} ha salido de su pool sin pedir permiso a nadie`, tone: 'hot' }
        case 'inheritance_keep_alive':
          return { icon: '💓', text: `${e.who} ha dado señales de vida` }
        case 'inheritance_withdraw':
          return { icon: '↩', text: `${e.who} ha recuperado su herencia` }
        case 'inheritance_claim':
          return { icon: '📜', text: `Se ha repartido la herencia de ${e.owner}`, tone: 'cold' }
        case 'subscription_pay':
          return { icon: '💸', text: `Pago de ${sats(e.amount_sat)} de la suscripción de ${e.owner}`, tone: 'cold' }
        case 'subscription_cancel':
          return { icon: '✋', text: `${e.who} ha cancelado su suscripción`, tone: 'hot' }
        case 'vault_start':
          return { icon: '⏱', text: `${e.who} ha iniciado un retiro de su vault`, tone: 'hot' }
        case 'vault_to_cold':
          return { icon: '🧊', text: `${e.who} ha mandado su vault a la caja fuerte`, tone: 'cold' }
        case 'vault_panic':
          return { icon: '🚨', text: `¡${e.who} ha pulsado el pánico!`, tone: 'cold' }
        case 'vault_withdraw':
          return { icon: '🔥', text: `${e.who} ha sacado su vault a la cartera caliente`, tone: 'hot' }
        case 'theft_start':
          return { icon: '🦹', text: `¡${e.who} intenta robar el vault de ${e.owner}!`, tone: 'thief' }
        case 'theft_success':
          return { icon: '💰', text: `${e.who} ha robado ${sats(e.amount_sat)} a ${e.owner}`, tone: 'thief' }
      }
  }
}
