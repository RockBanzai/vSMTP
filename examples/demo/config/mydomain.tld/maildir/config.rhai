fn on_config(config) {
    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev";
    config.broker.certificate_chain = "/etc/ssl/certs/ca-certificates.crt";
    config.user_lookup = "full_address";

    config.logs.default_level = "trace";
    config.logs.levels = #{
        lapin: "warn",
        pinky_swear: "warn",
        mio: "warn",
        polling: "warn",
        want: "warn",
        async_io: "warn",
    };

    config
}
