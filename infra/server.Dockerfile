FROM rust:1.98-slim-trixie AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p workshop-server

FROM debian:trixie-slim
COPY --from=build /src/target/release/workshop-server /usr/local/bin/workshop-server
USER 1000
EXPOSE 3000
ENTRYPOINT ["workshop-server"]
