fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:10025"],
    };

    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev";
    config.broker.certificate_chain = "/etc/ssl/certs/ca-certificates.crt";
    config.scripts.path = "/rules.rhai";

    config.logs.levels = #{
        "receiver": "warn",
    };

    config.esmtp = #{
        pipelining: true,
        dsn: true,
    };

    config
}
