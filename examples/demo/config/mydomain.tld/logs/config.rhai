fn on_config(config) {
    config.logs = #{
        default_level: "debug",
        levels: #{
            "external": "trace",
        },
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
