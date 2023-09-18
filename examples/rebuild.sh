#!/bin/env bash

### Build the vsmtp message broker, currently using rabbitmq
build_broker() {
    echo "build image vsmtp-broker:dev"
    docker build .. -f ../rabbitmq.Dockerfile \
        --tag vsmtp-broker:dev
}

### Build the vsmtp3-all-in-one image, which contains all the binaries
build_all_in_one() {
    echo "build image vsmtp-all-in-one:dev"
    docker build .. -f ../all-in-one.Dockerfile --tag vsmtp-all-in-one:dev || exit 1
}

### Copy the binary we are interested in from the all-in-one image
build_service() {
    echo "build image vsmtp3-$i:dev"
    docker build --tag $i:dev --build-arg BIN=$i - <<'EOF'
FROM vsmtp-all-in-one:dev as all-in-one
FROM debian:buster-slim AS runtime
ARG BIN
COPY --from=all-in-one /app/bin/$BIN /app/bin/$BIN
COPY --from=all-in-one /usr/lib/vsmtp /usr/lib/vsmtp
RUN mkdir -p /etc/vsmtp/plugins
RUN ln -s /usr/lib/vsmtp/libvsmtp_plugin_mysql.so /etc/vsmtp/plugins/libvsmtp_plugin_mysql.so
RUN ln -s /usr/lib/vsmtp/libvsmtp_clamav_plugin.so /etc/vsmtp/plugins/libvsmtp_clamav_plugin.so
ENV BIN_=$BIN
ENV PATH="$PATH:/app/bin"
CMD $BIN_
EOF
}

if [ "$#" -ne 0 ]; then
    bins=("$@")
else
    bins=(vsmtp-broker all_in_one vsmtp-receiver vsmtp-working vsmtp-log-dispatcher vsmtp-maildir vsmtp-mbox vsmtp-basic vsmtp-forward)
fi
for i in "${bins[@]}"; do
    if [ "$i" == "vsmtp-broker" ]; then
        build_broker
    elif [ "$i" == "all_in_one" ]; then
        build_all_in_one
    else
        build_service "$i"
    fi
done
