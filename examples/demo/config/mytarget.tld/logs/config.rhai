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
    config.loggers = [
        #{
            topic: "system",
            type: "console",
            formatter: "compact",
        },
    ];
    config.broker.uri = env::variable("BROKER");
    config
}
