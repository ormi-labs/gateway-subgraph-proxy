FROM rust:1.97-slim-bookworm AS build
RUN apt-get update && apt-get install -y \
    clang \
    cmake \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .

RUN cargo build --release --locked --color=always

FROM debian:bookworm-slim

RUN apt-get update ;\
    apt-get install --no-install-recommends -y ca-certificates ;\
    apt-get autoremove -y ;\
    rm -rf /var/lib/apt/lists/*

RUN set -x; \
  addgroup -g 1000 -S proxy ;\
  adduser -S -D -H -u 1000 -h /app -s /bin/sh -G proxy -g proxy proxy

COPY --from=build --chown=proxy:proxy /app/target/release/proxy /opt/proxy/proxy

WORKDIR /opt/proxy
USER proxy

ENTRYPOINT [ "./proxy" ]
