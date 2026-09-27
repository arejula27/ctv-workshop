# Código y artefactos reutilizados

| Origen | Licencia | Qué usamos | Dónde |
|---|---|---|---|
| [bitcoin-inquisition/bitcoin](https://github.com/bitcoin-inquisition/bitcoin) v29.4-inq | MIT | Binarios oficiales `bitcoind`/`bitcoin-cli` (SHA256 fijado) | `infra/bitcoind/Dockerfile` |
| [vulpemventures/nigiri](https://github.com/vulpemventures/nigiri) | MIT | Imágenes `ghcr.io/vulpemventures/electrs` y `esplora` y sus flags. **No** su bitcoind (Core v30, sin CTV) | `infra/docker-compose.yml` |
| [stutxo/op_ctv_payment_pool](https://github.com/stutxo/op_ctv_payment_pool) | MIT | (F3/F7) `ctv_scripts.rs`, `pools.rs` | `crates/ctv-core` |
| [stutxo/simple_ctv](https://github.com/stutxo/simple_ctv) | MIT | Referencia P2A (no usado por ahora) | — |
| jamesob/simple-ctv-vault, stutxo/Op_SecureTheSandwich, jamesob/opvault-demo | **sin licencia** | Solo ideas y diseño; nada de código | — |

## Notas de validación (F2, 2026-09-27)

- Ejecutado `op_ctv_payment_pool` (commit de 2026-02-05, 10 usuarios, modo regtest) contra
  nuestra Inquisition v29.4: las salidas CTV (`<hash32> OP_CTV`) se confirman correctamente.
- Upstream no compila tal cual: falta `use bitcoin::hashes::HashEngine;` en `ctv_scripts.rs`.
- Falla en la 7.ª salida con `bad-txns-inputs-duplicate`: la tx hija de CPFP (P2A) gasta dos veces
  la misma entrada. Es un bug de su camino de fees, no de CTV. Nosotros usamos fee fija en v2.
- Su camino de salida no exige firma (TODO en su código): lo añadimos al portarlo.
