FROM rust:latest AS chef

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY src ./src
COPY Cargo.toml Cargo.lock ./
COPY nutrition-units ./nutrition-units

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=chef /app/target/release/nutrition /usr/local/bin/nutrition

ENTRYPOINT ["nutrition"]
CMD ["--help"]