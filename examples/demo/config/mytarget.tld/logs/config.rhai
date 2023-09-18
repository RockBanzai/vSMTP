fn on_config(config) {
    config.logs = #{
        levels: #{
            "external": "trace",
            "log-dispatcher": "info",
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
