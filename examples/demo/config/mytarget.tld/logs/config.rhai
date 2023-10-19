fn on_config(config) {
    config.logs.default_level = "trace";
    config.logs.levels = #{
        lapin: "warn",
        pinky_swear: "warn",
        mio: "warn",
        polling: "warn",
        async_io: "warn",
    };
    config.loggers = [
        #{
            topic: "system",
            type: "console",
        },
        #{
            topic: "app",
            type: "console",
        },
    ];
    config.broker.uri = env::variable("BROKER");
    config
}
