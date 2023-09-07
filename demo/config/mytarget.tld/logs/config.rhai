fn on_config(config) {
    config.logs = #{
        levels: #{
            "external": "trace",
            "log-dispatcher": "info",
        },
    };
    // NOT USED AT THE MOMENT
    config.topics = [
        #{
            // rabbitmq topic: "console.info"
            type: "console", // stdout
            formatter: "text", // default: "text"
        },
        #{
            // rabbitmq topic: "syslog.error"
            type: "journald",
            formatter: "json", // default: "json"
        },
    ];
    config.broker.uri = "amqps://sink-mytarget:password@rabbitmq.mydomain.tld:5671/sink-mytarget";
    config
}
