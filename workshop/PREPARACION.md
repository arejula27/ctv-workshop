# Preparación del workshop

Lo que hay que hacer antes de empezar, en orden. Los comandos se lanzan desde la raíz del repo, en WSL.

## La víspera

1. **Docker Desktop** abierto y con la integración WSL activa
   (Settings → Resources → WSL integration). Comprueba con `docker version`.
2. **Código y web al día:**
   ```bash
   git pull
   cd web && npm install && npm run build && cd ..
   ```
3. **`infra/.env` existe** y tiene `RPC_PASS` y `ADMIN_TOKEN` propios (si no: `cp infra/.env.example infra/.env`
   y cámbialos). **Apunta el `ADMIN_TOKEN`**: lo necesitas en `/admin`.
4. Opcional, para quedarte tranquilo: `infra/scripts/test-regtest.sh` e `infra/scripts/test-ui.sh`.
   Ojo: los tests llenan la sala de participantes de prueba, así que haz después el paso 2 del día.

## El día, 30 minutos antes

1. **Levantar el stack:**
   ```bash
   docker compose -f infra/docker-compose.yml --env-file infra/.env up -d --build
   ```

2. **Vaciar la sala:** ejecuta `infra/scripts/reset.sh`. El tablero y la lista de participantes tienen ahora
   muchas entradas de los tests ("Ana kqnw", "Mesa 3 jog"…) y el reset deja la sala vacía.
   - Borra la cadena, los índices (esplora y mempool) y el registro de la sala, y arranca una regtest nueva (el servidor
     vuelve a minar los 200 bloques iniciales; tarda unos segundos). El calendario simulado empieza de nuevo.
   - ⚠️ Todavía no se ha ejecutado nunca: hazlo la víspera también, para ver que funciona.
   - Los navegadores detectan la cadena nueva y olvidan solos la partida anterior (se conserva el nombre).

3. **Comprobar que todo responde:**
   ```bash
   set -a; . infra/.env; set +a; infra/scripts/smoke.sh
   ```
   Tiene que acabar en `all good`. Deja en el feed un pedido al faucet y un salto de 10 bloques: es normal.

4. **Tu portátil: abre `/admin`** (`http://localhost:8080/admin`), pega el `ADMIN_TOKEN` y:
   - en **Demo que se juega ahora**, elige la primera (🔒 Vault);
   - en **Reloj**, elige el ritmo. Con 30 s por bloque, una espera de 10 bloques son 5 minutos: hay tiempo para
     que la ladrona actúe y para que la víctima reaccione.

5. **Proyector: abre `/screen`** (`http://localhost:8080/screen`) a pantalla completa (F11). No necesita cartera.

6. **La dirección para la gente:** `http://<IP de tu portátil>:8080`. Debajo del reloj, `/screen` muestra
   "Entra en…" con esa dirección, para quien no la tenga.
   - Docker Desktop publica el puerto 8080 en Windows: la IP es la de Windows en la wifi del evento
     (`ipconfig` en PowerShell → "Dirección IPv4").
   - El firewall de Windows puede bloquear el 8080: pruébalo desde tu móvil antes de que llegue nadie.
   - El explorador (mempool) está en `http://<IP>:5000`. Muestra el diagrama de cada transacción y, en las
     entradas que gastan un contrato, la fila **P2TR tapscript** con el script (`… OP_NOP4` = OP_CHECKTEMPLATEVERIFY).

## Durante

- **Fondos**: no hay botón de faucet en la app. Cada persona pulsa **Recibir** en su cartera, copia la dirección y la
  pega en `http://<IP>:8080/faucet`.
- **Vault**: al entrar, cada persona elige papel (usuaria o ladrona). Conviene que primero creen su cartera caliente
  todas las usuarias, así pueden elegirse como caja fuerte entre ellas.
- **Al pasar a otra demo**, cámbiala en `/admin`. El proyector cambia de tablero y en la app de cada persona
  aparece un ● en la pestaña de la demo nueva.
- **Saltos de tiempo** desde `/admin`: `+144` bloques = 1 día en el calendario.
- **Pausa** el reloj mientras explicas, para que nada caduque a medias.

## Si algo falla

| Síntoma | Qué hacer |
|---|---|
| La web no carga, o "connection reset", tras reiniciar Docker Desktop | `docker compose -f infra/docker-compose.yml --env-file infra/.env up -d --force-recreate` (la cadena se conserva) |
| El faucet dice "wait …s" | Cada dirección puede pedir una vez por minuto (`FAUCET_COOLDOWN_S` en `infra/.env`) |
| "No tienes saldo suficiente" justo después del faucet | Espera al siguiente bloque, o pulsa `+1 bloque` en `/admin` |
| Hay que empezar de cero | `infra/scripts/reset.sh` y que todo el mundo recargue la página |
