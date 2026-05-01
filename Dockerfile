FROM lukemathwalker/cargo-chef:latest-rust-1.75 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release -p clawdb -p clawdb-cli -p clawdb-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 grep \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -u 1001 -m -d /home/clawdb clawdb
WORKDIR /home/clawdb

COPY --from=builder /app/target/release/clawdb /usr/local/bin/clawdb
COPY --from=builder /app/target/release/clawdb-cli /usr/local/bin/clawdb-cli
COPY --from=builder /app/target/release/clawdb-server /usr/local/bin/clawdb-server

RUN chown -R clawdb:clawdb /home/clawdb
USER clawdb

EXPOSE 50050 8080 9090
VOLUME ["/home/clawdb/.clawdb"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD clawdb-cli status --json 2>/dev/null | grep -q '"ok":true'

ENTRYPOINT ["clawdb-server"]
