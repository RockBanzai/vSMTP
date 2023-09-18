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
ARG MODE

RUN apt-get update && apt-get install -y build-essential
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --workspace --all-features --recipe-path recipe.json --profile $MODE
COPY . .
RUN cargo build --workspace --all-features --profile $MODE

FROM debian:buster-slim AS runtime
ARG MODE

COPY --from=builder /app/target/$MODE/vsmtp-receiver /app/bin/vsmtp-receiver
COPY --from=builder /app/target/$MODE/vsmtp-working /app/bin/vsmtp-working
COPY --from=builder /app/target/$MODE/vsmtp-log-dispatcher /app/bin/vsmtp-log-dispatcher
COPY --from=builder /app/target/$MODE/vsmtp-maildir /app/bin/vsmtp-maildir
COPY --from=builder /app/target/$MODE/vsmtp-mbox /app/bin/vsmtp-mbox
COPY --from=builder /app/target/$MODE/vsmtp-basic /app/bin/vsmtp-basic
COPY --from=builder /app/target/$MODE/vsmtp-forward /app/bin/vsmtp-forward
RUN mkdir /usr/lib/vsmtp
COPY --from=builder /app/target/$MODE/*.so /usr/lib/vsmtp/

ENV PATH="$PATH:/app/bin"
