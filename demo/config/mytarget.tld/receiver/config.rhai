fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:25"],
        addr_submission: ["0.0.0.0:587"],
    };

    config.broker.uri = "amqps://sink-mytarget:password@rabbitmq.mydomain.tld:5671/sink-mytarget";
    config.broker.certificate_chain = "/etc/ssl/certs/ca-certificates.crt";
    config.scripts.path = "/rules.rhai";

    config.logs.levels = #{
        "receiver": "warn",
        "delivery": "warn",
    };

    config.esmtp = #{
        pipelining: true,
        dsn: true,
    };

    config
}
