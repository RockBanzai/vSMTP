fn on_config(config) {
    config.logs = #{
        default_level: "debug",
        levels: #{
            "external": "trace",
        },
    };
    config.topics = [
        #{
            // rabbitmq topic: "console.info"
            type: "console", // stdout
            formatter: "compact", // default: "compact"
        },
        #{
            // rabbitmq topic: "syslog.error"
            type: "journald",
        },
        #{
            // rabbitmq topic: "syslog.error"
            type: "syslog",
            formatter: "rfc5424",
            protocol: "INET_STREAM",
            address: "localhost:514",
        },
        #{
            type: "file",
            folder: "./mylog",
            rotation: "hourly",
        }
    ];
    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev";
    config
}
