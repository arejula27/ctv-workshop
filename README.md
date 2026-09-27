# CTV Workshop

Webapp (Rust→WASM) para enseñar OP_CHECKTEMPLATEVERIFY a público no técnico sobre una regtest
de Bitcoin Inquisition controlada por el presentador. Plan completo en [PLAN.md](PLAN.md).

## Requisitos

- Rust (se instala solo el toolchain de `rust-toolchain.toml` con `rustup`)
- Node ≥ 22
- Docker con Compose (en WSL: activar la integración WSL de Docker Desktop)

## Levantar el stack

```bash
cd web && npm install && npm run build && cd ..   # Caddy sirve web/dist
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

## API del servidor

| Método | Ruta | Auth | |
|---|---|---|---|
| GET | `/api/status` | — | altura, reloj, faucet |
| POST | `/api/faucet` `{address}` | cooldown por dirección | envía sats (mina 1 bloque si el reloj está en pausa) |
| POST | `/api/admin/mine` `{blocks}` | `Bearer ADMIN_TOKEN` | mina N bloques |
| POST | `/api/admin/jump` `{blocks}` | `Bearer ADMIN_TOKEN` | salto temporal (igual que mine, pero se anuncia en `/screen`) |
| PUT | `/api/admin/miner` `{paused?, interval_s?}` | `Bearer ADMIN_TOKEN` | reloj automático |
| WS | `/ws` | — | eventos `hello`, `block`, `faucet`, `miner`, `jump` |

## Desarrollo sin Docker

```bash
# nodo Inquisition nativo (binarios de la release v29.4-inq)
bitcoind -conf=$PWD/infra/bitcoind/bitcoin.conf -rpcuser=u -rpcpassword=p
RPC_USER=u RPC_PASS=p ADMIN_TOKEN=dev-admin-token cargo run -p workshop-server
BACKEND_URL=http://localhost:3000 npm --prefix web run dev
```
