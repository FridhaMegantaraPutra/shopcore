# ---- build ----
FROM rust:1-slim-bookworm AS build
WORKDIR /app

# Cache dependencies separately from source: build a dummy crate first.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && echo 'fn main() {}' > src/main.rs && echo 'fn main() {}' > src/bin/bank_sim.rs \
    && touch src/lib.rs && cargo build --release --locked && rm -rf src

COPY migrations ./migrations
COPY src ./src
# Bump mtimes so cargo rebuilds our crate rather than reusing the dummy.
RUN touch src/main.rs src/lib.rs src/bin/bank_sim.rs && cargo build --release --locked

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 shopcore
COPY --from=build /app/target/release/shopcore /app/target/release/bank-sim /usr/local/bin/
USER shopcore
EXPOSE 8000
ENTRYPOINT ["shopcore"]
CMD ["serve"]
