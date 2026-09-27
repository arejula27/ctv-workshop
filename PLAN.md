# CTV Workshop — Plan

Workshop para enseñar CTV (BIP-119) a público no técnico. Cada participante usa desde el navegador
una webapp (núcleo Rust→WASM) sobre una regtest compartida que controla el presentador.

Demos: **Vault**, **Payment pool**, **Herencia**, **Suscripción cancelable**.
Las cuatro son la misma pieza: un árbol de transacciones CTV con una salida alternativa en otra hoja taproot.

---

## 1. Decisiones

### Tomadas
| Tema | Decisión | Motivo |
|---|---|---|
| Red | **regtest** con Bitcoin Inquisition | Control total del reloj (minado, saltos, reset) |
| Timelocks | **Siempre por altura** (CSV/CLTV en bloques) | "Saltar en el tiempo" = minar N bloques |
| Comisiones | **Fee fija dentro de la plantilla**, tx v2 | Sin mercado de fees en regtest; P2A/TRUC añade complejidad y esperas |
| Claves | En el navegador (localStorage), exportables | Son monedas de test |
| Identidad | Nombre + pubkey, sin login | Cero fricción |
| App | **Una sola webapp con rutas** | Un saldo, un despliegue, visualizador común |
| Reutilización | **Máxima** (ver §6) | Tiempo |
| Reloj | Compartido, lo controla el presentador | Cadena única para toda la sala |
| Pool | **Mesas de 4–6 personas** | El árbol del pool es O(2ⁿ); 21 usuarios = 43 min nativo |
| rust-bitcoin | **0.32** | Lo usa todo el código reutilizable; compila a WASM |
| Nodo | **Inquisition v29.4**, binarios oficiales con SHA256 fijado | CTV, CSFS, CAT, APO activos en regtest por defecto (verificado); sin compilar |
| Indexador/explorador | Imágenes de electrs y esplora de Nigiri (Vulpem) | Solo leen bloques por RPC; el bitcoind de Nigiri (Core v30) **no** se usa |
| RPC en el server | Cliente JSON-RPC async propio (~80 líneas) | `bitcoincore-rpc` es bloqueante y sus tipos se rompen con Core ≥ 28 |
| Reset | Script `infra/scripts/reset.sh` (borra volúmenes) | Vaciar mempool/cadena desde dentro de bitcoind no es fiable |

### Abiertas (con valor por defecto)
| Tema | Por defecto | Pendiente de |
|---|---|---|
| Nº asistentes | **≤ 20** (diseñar para 20, probar con 30) | Confirmado |
| Hosting | VPS + portátil con el compose como plan B | Confirmar |
| Framework UI | Svelte + Vite con el core vía wasm-pack | Alternativa: Leptos (todo Rust, UI más lenta de hacer) |
| Wallet base | `bdk_wallet` + `bdk_esplora` (async, WASM) para la wallet normal; construcción manual para gastos CTV | Validar en F4 que compila a WASM sin fricción |

---

## 2. Arquitectura

```
                 ┌──────────────────────────── navegador (participante) ──────────────────────────┐
                 │  webapp (Svelte)  ── ctv-wasm (ctv-core) ── claves en localStorage             │
                 └───────┬──────────────────────┬──────────────────────────┬─────────────────────┘
                         │ /esplora/*           │ /api/*                   │ /ws
                 ┌───────▼──────────────────────▼──────────────────────────▼─────────────────────┐
                 │                          Caddy (mismo origen, TLS)                             │
                 └───────┬──────────────────────┬──────────────────────────┬─────────────────────┘
                         │                      │                          │
                 ┌───────▼───────┐     ┌────────▼─────────────────────────▼──────┐
                 │ esplora/electrs│     │ workshop-server (Rust, axum)            │
                 │ REST: utxos,   │     │  · faucet, mine, auto-miner, jump, reset│
                 │ tx, broadcast  │     │  · coordinador de pools (salas WS)      │
                 └───────┬───────┘     │  · bus de eventos → /screen             │
                         │             └────────┬────────────────────────────────┘
                         │   RPC/P2P            │ RPC (credenciales solo aquí)
                 ┌───────▼──────────────────────▼──────┐
                 │ bitcoind (Inquisition, regtest)      │      esplora (explorador, :5000)   
                 └──────────────────────────────────────┘
```

---

## 3. Estructura del monorepo

```
ctv-workshop/
├── PLAN.md
├── README.md
├── Cargo.toml                  # workspace
├── rust-toolchain.toml
├── crates/
│   ├── ctv-core/               # sin I/O; compila a nativo y wasm32
│   │   ├── src/
│   │   │   ├── ctv.rs          # hash CTV, script CTV (de stutxo, MIT)
│   │   │   ├── taproot.rs      # NUMS, builder de hojas, control blocks
│   │   │   ├── tree.rs         # motor genérico de árbol de plantillas + export JSON
│   │   │   ├── spend.rs        # construir/firmar gastos de cada hoja
│   │   │   └── contracts/
│   │   │       ├── vault.rs
│   │   │       ├── pool.rs     # de stutxo (MIT) + firma por hoja
│   │   │       ├── inheritance.rs
│   │   │       └── subscription.rs
│   │   └── tests/
│   │       ├── ctvhash_vectors.rs   # vectores oficiales BIP-119
│   │       └── regtest_*.rs         # integración contra el compose (feature `regtest-it`)
│   ├── ctv-wasm/               # wasm-bindgen: API pequeña, JSON in/out
│   └── workshop-server/        # axum: control API + coordinador + eventos
├── web/                        # Svelte + Vite
│   └── src/
│       ├── lib/                # cliente esplora, cliente ws, store de wallet
│       ├── components/TreeView # visualizador (cytoscape.js)
│       └── routes/             # / wallet vault pool herencia suscripcion admin screen
├── infra/
│   ├── docker-compose.yml
│   ├── bitcoind/               # Dockerfile (base stutxo) + bitcoin.conf
│   ├── esplora/
│   ├── Caddyfile
│   └── scripts/                # reset.sh, smoke.sh
├── workshop/
│   ├── runbook.md              # guion del facilitador (orden, reloj por escena, plan B)
│   ├── slides/
│   └── cheatsheet.md           # hoja para participantes + QR
└── vendor-notes/               # atribuciones y qué se tomó de cada repo
```

---

## 4. Componentes

### 4.1 Infra (`infra/`)
- **bitcoind**: Inquisition regtest, `-txindex=1 -rest=1 -minrelaytxfee=0 -fallbackfee=0.0001`.
  Inquisition v29.4: CTV/CSFS/CAT/APO activos en regtest sin `-vbparams` (verificado con `getdeploymentinfo`).
  Wallet interna `faucet` que recibe la coinbase.
- **esplora/electrs**: indexa bitcoind y expone la API REST que usa la webapp.
- **explorer**: frontend de esplora (imagen de Vulpem) para proyectar, en el puerto 5000.
- **Caddy**: sirve `web/dist`, hace proxy de `/api`, `/ws` y `/esplora`. Al ser mismo origen no hace falta CORS.

### 4.2 `workshop-server`
| Endpoint | Auth | Función |
|---|---|---|
| `POST /api/faucet {address}` | rate-limit por dirección/IP | Envía X sats y mina 1 bloque si el autominado está parado |
| `GET  /api/status` | — | Altura, intervalo de minado, estado (pausado/activo) |
| `POST /api/admin/mine {n}` | token | Mina n bloques |
| `PUT  /api/admin/miner {interval_s \| paused}` | token | Autominado |
| `POST /api/admin/jump {blocks}` | token | Salto temporal = mina n bloques de golpe |
| — | — | Reset: `infra/scripts/reset.sh` desde el host (borra volúmenes de bitcoind y electrs; el server re-siembra 200 bloques al arrancar) |
| `WS   /ws` | — | Eventos (`block`, `faucet`, `vault_unvault`, `pool_created`…) y salas del pool |

Coordinador de pool: crear sala → unirse (nombre + pubkey + dirección de salida) → el host cierra la sala
→ el servidor difunde la lista ordenada → cada cliente calcula **localmente** el mismo árbol y la
misma dirección → cada miembro aporta su UTXO (tx de funding colaborativa, PSBT) o, para simplificar,
el faucet financia el pool en nombre de todos (v1).

### 4.3 `ctv-core`
- `tree.rs`: `Node { outputs, locktime, sequence, leaves: Vec<Leaf> }` con
  `Leaf::Ctv { csv: Option<u16>, signer: Option<XOnly>, child: Node }` y `Leaf::Key { csv, pk }`.
  Se compila de las hojas hacia la raíz y exporta JSON `{nodes, edges, status}` para el visualizador.
- Clave interna siempre NUMS (no hay camino por keypath en v1).
- Fee fija por transacción descontada del valor.

### 4.4 Contratos

**Vault** (diseño de jamesob pasado a taproot):
```
vault_out:   [ <H(unvault)> CTV ]  |  [ <H(tocold)> CTV ]
unvault_out: [ <delay> CSV DROP <hot_pk> CHECKSIG ]  |  [ <H(tocold)> CTV ]
```
Iniciar el retiro y el botón del pánico **no requieren firma**: cualquiera, incluido un vigilante, puede
publicarlos. Lo único que el ladrón con la clave hot puede hacer es esperar `delay` bloques.
Juego: se le "roba" la clave hot a un compañero y tiene `delay` bloques para reaccionar.

**Pool** (stutxo + firma):
```
pool(S) = { para cada i∈S: [ <pk_i> CHECKSIGVERIFY <H(exit_i → pool(S∖i))> CTV ] }
```
Mesas de hasta 6 → ≤ 64 árboles, se calcula al instante en WASM.

**Herencia**:
```
out: [ <owner_pk> CHECKSIG ]  |  [ <N> CSV DROP <H(reparto)> CTV ]
```
"Seguir vivo" = el dueño gasta a la misma estructura y el CSV vuelve a empezar.

**Suscripción** (K pagos, se construye del último al primero):
```
tramo_k: [ <N> CSV DROP <H(pago_k: X→receptor, resto→tramo_{k+1})> CTV ]  |  [ <payer_pk> CHECKSIG ]
```
Cualquiera puede empujar el pago cuando vence el plazo (el receptor, o un botón automático en la app).
Cancelar = el pagador firma y recupera lo pendiente.

### 4.5 `ctv-wasm`
API mínima y estable (JSON): `wallet_new/restore`, `vault_create/unvault/panic/withdraw`,
`pool_build/exit`, `inheritance_create/refresh/claim`, `subscription_create/pay/cancel`,
`tree_json`. Devuelve tx en hex; el broadcast lo hace JS contra esplora.

### 4.6 `web`
- `/` onboarding (nombre, generar/restaurar seed, pedir sats)
- `/wallet` saldo, recibir, enviar a un participante por nombre
- `/vault` `/pool` `/herencia` `/suscripcion`: narración paso a paso + `TreeView`
- `/admin` (token): reloj, salto, pausa, reset, participantes
- `/screen` proyector: altura, cuenta atrás al próximo bloque, feed de eventos, progreso de la sala

`TreeView`: nodos = transacciones (pendiente / en mempool / confirmada / bloqueada hasta bloque X),
aristas = hojas (etiqueta "espera N bloques", "firma de X", "cualquiera").

---

## 5. Fases

| Fase | Entregable | Criterio de aceptación |
|---|---|---|
| **F0** Entorno | Toolchain en WSL (rustup + `wasm32-unknown-unknown`, wasm-pack, node, docker), esqueleto del workspace | `cargo build` y `npm run dev` funcionan |
| **F1** Infra | `docker-compose` con bitcoind, electrs, explorer, Caddy y `workshop-server` (faucet, mine, miner, jump) | `infra/scripts/smoke.sh`: faucet → tx visible en esplora → jump 10 → altura +10 |
| **F2** Validación | Ejecutar el pool de stutxo (sin modificar) contra nuestra regtest | Pool de 4 creado y las 4 salidas confirmadas |
| **F3** ctv-core + vault | Hash CTV con vectores, motor de árbol, contrato vault | Vectores BIP-119 pasan; test de integración: depósito → retiro → pánico, y depósito → retiro → espera → hot |
| **F4** WASM + web mínima | Onboarding, wallet, `/vault`, `TreeView` | Dos navegadores: A roba la clave hot de B, B pulsa el pánico a tiempo |
| **F5** Presentador | `/admin`, `/screen`, eventos WS | Ensayo del vault de principio a fin controlando el reloj desde `/admin` |
| **F6** Herencia + suscripción | Contratos + pantallas | Tests de integración de todos los caminos; demo con saltos de tiempo |
| **F7** Pool | `pool.rs` con firma, coordinador, pantalla | Mesa de 5 navegadores: crear pool, salidas en cualquier orden, sin poder sacar a otro |
| **F8** Endurecimiento | Carga simulada (N clientes headless), `reset.sh`, runbook, slides, cheatsheet | 30 clientes simulados sin errores; ensayo general con 5–10 personas |

Orden de valor: F1–F4 ya permiten un workshop solo con el vault. F6 y F7 amplían.

---

## 6. Reutilización y licencias

| Origen | Licencia | Uso |
|---|---|---|
| stutxo/op_ctv_payment_pool | MIT | Copiar `ctv_scripts.rs` y `pools.rs`, Dockerfile de Inquisition, imagen `ghcr.io/stutxo/grugpool-regtest` |
| stutxo/simple_ctv | MIT | Referencia para P2A si algún día se añade |
| Nigiri: electrs + esplora (Vulpem) | MIT | Imágenes sin modificar |
| Bitcoin Inquisition v29.4 | MIT | Binarios oficiales |
| Vectores CTV de BIP-119 | Dominio del BIP | Tests |
| jamesob/simple-ctv-vault | **Sin licencia** | Solo el diseño; reimplementación propia |
| stutxo/Op_SecureTheSandwich | **Sin licencia** | Solo la idea narrativa |
| jamesob/opvault-demo | **Sin licencia** (y es OP_VAULT) | Solo la idea de UX (alertas) |
| ursuscamp/ctvlib | **Sin licencia**, rust-bitcoin 0.31 | No se usa |
| bdk_wallet / bdk_esplora | MIT/Apache | Wallet del navegador (a validar en F4) |
| cytoscape.js | MIT | Visualizador |

Atribución en `vendor-notes/` y en la cabecera de cada archivo copiado.

---

## 7. Riesgos

| Riesgo | Mitigación |
|---|---|
| Esplora se retrasa indexando con bloques rápidos | Intervalo mínimo de ~10 s; la UI usa la altura de `/api/status` como verdad |
| Un salto temporal rompe la demo de otro grupo | El runbook fija qué reloj corresponde a cada escena; `/screen` avisa antes de un salto |
| Carrera cancelar vs. pago (suscripción) o pánico vs. hot (vault) | Se usa como momento didáctico; la UI muestra quién ganó |
| Pérdida de claves (limpiar el navegador) | Exportar la seed como texto/QR en el onboarding; el faucet permite recargar |
| Wifi del evento | VPS + datos móviles; plan B: portátil + router propio |
| Tamaño del WASM / tiempo de carga | `wasm-opt -Oz`, sin `bitcoincore-rpc` en el core |
| bdk no encaja en WASM | Plan B: wallet mínima propia (una clave P2TR + UTXOs de esplora) |
| Imágenes `:latest` de Vulpem cambian | Fijar digest en F8 |

---

## 8. Estado (2026-09-27)

| Fase | Estado |
|---|---|
| F0 | ✅ Rust 1.98 + wasm32, Node 24 (en `~/.local/node`), workspace, web Svelte. `wasm-pack` se instala en F4 |
| F1 | ✅ Compose levantado (bitcoind, electrs, explorer, server, Caddy). `smoke.sh` completo pasa, esplora incluido; web y WS funcionan a través de Caddy |
| F2 | ✅ Pool de stutxo contra Inquisition v29.4: salidas CTV confirmadas (ver `vendor-notes/`) |
