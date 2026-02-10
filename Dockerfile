FROM rust:1.76-slim AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates \
  openssh-client \
  rsync \
  git \
  tar \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/rbazel /usr/local/bin/rbazel
WORKDIR /work
ENTRYPOINT ["rbazel"]
