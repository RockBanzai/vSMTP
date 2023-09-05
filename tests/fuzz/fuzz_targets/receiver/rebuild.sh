#!/bin/env bash

### Build the vsmtp message broker.
echo "build image vsmtp3-broker:dev"
docker build ../../../../ -f ../../../../rabbitmq.Dockerfile \
    --tag vsmtp3-broker:dev

### Build the vsmtp3-all-in-one image, which contains all the binaries
echo "build image vsmtp3-all-in-one:dev"
docker build ../../../../ -f ../../../../all-in-one.Dockerfile --tag vsmtp3-all-in-one:dev || exit 1

### Copy the binary we are interested in from the all-in-one image.
### Plugins are not copied because fuzzing do not require them.
bins=(receiver log-dispatcher)
for i in "${bins[@]}"; do
    echo "build image vsmtp3-$i:dev"
    docker build --tag vsmtp3-$i:dev --build-arg BIN=$i - <<'EOF'
FROM vsmtp3-all-in-one:dev as all-in-one
FROM debian:buster-slim AS runtime
ARG BIN
COPY --from=all-in-one /app/bin/$BIN /app/bin/$BIN
COPY --from=all-in-one /usr/lib/vsmtp /usr/lib/vsmtp
ENV BIN_=$BIN
ENV PATH="$PATH:/app/bin"
CMD $BIN_
EOF
done
