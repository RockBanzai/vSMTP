fn on_config(config) {
    config.logs = #{
        default_level: "trace",
        levels: #{
            external: "trace",
            lapin: "warn",
            pinky_swear: "warn",
            mio: "warn",
            polling: "warn",
            want: "warn",
            async_io: "warn",
            rustls: "warn",
        },
    };
    config.topics = [
        #{
            type: "console",
            formatter: "compact",
        },
    ];
    config.broker.uri = "amqps://sink-mytarget:password@rabbitmq.mydomain.tld:5671/sink-mytarget";
    config
}
