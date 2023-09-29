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
            name: "system",
            type: "console",
            formatter: "rfc5424",
        },
        #{
            name: "journald",
            type: "journald",
        },
    ];

    let result = process::run(#{
        args: ["env"],
    });

    print(result.stdout);
    debug(result.stdout);

    config.broker.uri = env::variable("BROKER");
    config
}
