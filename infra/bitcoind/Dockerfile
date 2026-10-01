# Official Bitcoin Inquisition release binaries (no compiling), checksum pinned.
FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a

ARG INQ_VERSION=29.4
ARG INQ_SHA256=7ad3cb745575f7c21916cf3c294a95f59e11e5bf6e37c4a47b92697cf4723e22

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && curl -fsSL -o /tmp/inq.tar.gz \
    "https://github.com/bitcoin-inquisition/bitcoin/releases/download/v${INQ_VERSION}-inq/bitcoin-${INQ_VERSION}-inq-x86_64-linux-gnu.tar.gz" \
 && echo "${INQ_SHA256}  /tmp/inq.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/inq.tar.gz -C /tmp \
 && install -m 0755 /tmp/bitcoin-${INQ_VERSION}-inq/bin/bitcoind /tmp/bitcoin-${INQ_VERSION}-inq/bin/bitcoin-cli /usr/local/bin/ \
 && rm -rf /tmp/* \
 && apt-get purge -y curl && apt-get autoremove -y && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 bitcoin && mkdir -p /home/bitcoin/.bitcoin && chown bitcoin:bitcoin /home/bitcoin/.bitcoin
COPY bitcoin.conf /etc/bitcoin/bitcoin.conf
USER bitcoin
VOLUME /home/bitcoin/.bitcoin
EXPOSE 18443 18444

ENTRYPOINT ["bitcoind", "-conf=/etc/bitcoin/bitcoin.conf", "-datadir=/home/bitcoin/.bitcoin"]
