#!/bin/env bash

# Build the vsmtp message broker, currently using rabbitmq
echo "building vsmtp-broker:dev image"
docker build .. -f ../rabbitmq.Dockerfile \
    --tag vsmtp-broker:dev

# Build the vsmtp-all-in-one image, which contains all the binaries
echo "building vsmtp-all-in-one:dev image"
docker build .. -f ../all-in-one.Dockerfile \
    --build-arg "MODE=release" \
    --tag vsmtp-all-in-one:dev || exit 1

# Copy all binaries from the all-in-one image into their own image.
bins=(receiver working log-dispatcher maildir mbox basic forward)
for bin in "${bins[@]}"; do
    echo "building vsmtp-$bin:dev image"
    docker build --tag vsmtp-$bin:dev --build-arg BIN=$bin - <<'EOF'
FROM vsmtp-all-in-one:dev as all-in-one
FROM debian:buster-slim AS runtime
ARG BIN
COPY --from=all-in-one /app/bin/$BIN /app/bin/$BIN
ARG BIN
RUN mkdir -p /etc/vsmtp/$BIN/plugins
ARG BIN
COPY --from=all-in-one /usr/lib/vsmtp /etc/vsmtp/$BIN/plugins
ENV BIN_=$BIN
ENV PATH="$PATH:/app/bin"
CMD $BIN_
EOF
done
