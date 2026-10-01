FROM rust:1.98-slim-trixie@sha256:4cd829461bd5c4d511c32e269da9cb8929223b666519d8004e35fc8d1d771ab7 AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p workshop-server

FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a
COPY --from=build /src/target/release/workshop-server /usr/local/bin/workshop-server
# Room registry lives here (named volume); must be writable by the runtime user.
RUN mkdir /data && chown 1000:1000 /data
USER 1000
EXPOSE 3000
ENTRYPOINT ["workshop-server"]
