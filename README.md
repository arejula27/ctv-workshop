# CTV Workshop

Webapp (Rust→WASM) para enseñar OP_CHECKTEMPLATEVERIFY a público no técnico sobre una regtest
de Bitcoin Inquisition controlada por el presentador. Plan completo en [PLAN.md](PLAN.md).

## Requisitos

- Rust (se instala solo el toolchain de `rust-toolchain.toml` con `rustup`)
- Node ≥ 22, `wasm-pack`, `clang` (para compilar secp256k1 a wasm32)
- Docker con Compose (en WSL: activar la integración WSL de Docker Desktop)

## Levantar el stack

```bash
cd web && npm install && npm run build && cd ..   # compila el wasm (wasm-pack) y la web; Caddy sirve web/dist
cp infra/.env.example infra/.env                   # cambia RPC_PASS y ADMIN_TOKEN
docker compose -f infra/docker-compose.yml --env-file infra/.env up -d --build
ADMIN_TOKEN=... infra/scripts/smoke.sh             # comprobación end-to-end
```

| URL | Qué es |
|---|---|
| http://localhost:8080 | Webapp |
| http://localhost:8080/api/status | Estado de la cadena |
| http://localhost:8080/esplora/blocks/tip/height | API Esplora |
| http://localhost:5000 | Explorador (para proyectar) |

Reiniciar la cadena desde cero: `infra/scripts/reset.sh`.

Prueba de carga (25 participantes simulados a la vez): `ADMIN_TOKEN=... node web/e2e/load.mjs`.

Tests contra el stack: `infra/scripts/test-regtest.sh` (contratos) e `infra/scripts/test-ui.sh`
(las cuatro demos con navegadores headless; capturas en `web/e2e/shots/`). Sin Docker:
`infra/scripts/native-regtest.sh start|test|stop`.

**Si Docker Desktop se reinicia** y los puertos publicados dan "connection reset" (los contenedores
siguen vivos pero el port-forwarding queda colgado):
`docker compose -f infra/docker-compose.yml --env-file infra/.env up -d --force-recreate`.
La cadena sobrevive a un cierre brusco: el server fuerza a bitcoind a escribir a disco tras cada bloque.

## API del servidor

| Método | Ruta | Auth | |
|---|---|---|---|
| GET | `/api/status` | — | altura, reloj, faucet |
| POST | `/api/faucet` `{address}` | cooldown por dirección | envía sats (mina 1 bloque si el reloj está en pausa) |
| POST | `/api/admin/mine` `{blocks}` | `Bearer ADMIN_TOKEN` | mina N bloques |
| POST | `/api/admin/jump` `{blocks}` | `Bearer ADMIN_TOKEN` | salto temporal (igual que mine, pero se anuncia en `/screen`) |
| PUT | `/api/admin/miner` `{paused?, interval_s?}` | `Bearer ADMIN_TOKEN` | reloj automático |
| GET/POST | `/api/participants` | — | la sala (nombre + dirección) |
| GET/POST | `/api/vaults` | — | vaults (validados con `ctv-core` y en cadena) |
| GET/POST | `/api/contracts` | — | herencias y suscripciones (id estable, outpoint móvil) |
| GET/POST | `/api/pools`, `/api/pools/{id}/join`, `/api/pools/{id}/start` | — | mesas del payment pool; `start` lo financia el faucet |
| POST | `/api/activity` | — | lo que hace la gente, para el feed (debe apuntar a una tx real) |
| WS | `/ws` | — | `hello` (con los últimos eventos), `block`, `faucet`, `miner`, `jump`, `participant_joined`, `vault_registered`, `contract_registered`, `pool_updated`, `activity` |

## Desarrollo sin Docker

```bash
# nodo Inquisition nativo (binarios de la release v29.4-inq)
bitcoind -conf=$PWD/infra/bitcoind/bitcoin.conf -rpcuser=u -rpcpassword=p
RPC_USER=u RPC_PASS=p ADMIN_TOKEN=dev-admin-token cargo run -p workshop-server
BACKEND_URL=http://localhost:3000 npm --prefix web run dev
```
