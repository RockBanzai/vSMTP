##
FROM rust:1.72.0-slim-buster AS chef
USER root
RUN cargo install cargo-chef@0.1.61
WORKDIR /app

##
FROM chef AS planner
# note: see dockerignore for more details ;)
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

##
FROM chef AS builder
RUN apt-get update && apt-get install -y \
    build-essential \
    protobuf-compiler \
    libprotobuf-dev
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --workspace --all-features --recipe-path recipe.json
COPY . .
ARG MODE
RUN if ["$MODE" = "release"]; then cargo build --workspace --all-features --release; else cargo build --workspace --all-features; fi

FROM debian:buster-slim AS runtime
COPY --from=builder /app/target/debug/vsmtp-receiver /app/bin/vsmtp-receiver
COPY --from=builder /app/target/debug/vsmtp-working /app/bin/vsmtp-working
COPY --from=builder /app/target/debug/vsmtp-log-dispatcher /app/bin/vsmtp-log-dispatcher
COPY --from=builder /app/target/debug/vsmtp-maildir /app/bin/vsmtp-maildir
COPY --from=builder /app/target/debug/vsmtp-mbox /app/bin/vsmtp-mbox
COPY --from=builder /app/target/debug/vsmtp-basic /app/bin/vsmtp-basic
COPY --from=builder /app/target/debug/vsmtp-forward /app/bin/vsmtp-forward
RUN mkdir /usr/lib/vsmtp
COPY --from=builder /app/target/debug/*.so /usr/lib/vsmtp/

ENV PATH="$PATH:/app/bin"
