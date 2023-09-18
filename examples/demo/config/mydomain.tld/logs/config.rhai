fn on_config(config) {
    config.logs.default_level = "trace";
    config.logs.levels = #{
        lapin: "warn",
        pinky_swear: "warn",
        mio: "warn",
        polling: "warn",
        want: "warn",
        async_io: "warn",
    };

    config.topics = [
        #{
            type: "console",
            formatter: "compact",
        },
    ];

    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev";
    config
}
