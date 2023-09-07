fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:25"],
        addr_submission: ["0.0.0.0:587"],
    };

    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev";
    config.broker.certificate_chain = "/etc/ssl/certs/ca-certificates.crt";
    config.scripts.path = "/rules.rhai";

    config.logs.default_level = "trace";
    config.logs.levels = #{
        lapin: "warn",
        pinky_swear: "warn",
        mio: "warn",
        polling: "warn",
        want: "warn",
        async_io: "warn",
    };

    config.esmtp = #{
        pipelining: true,
        dsn: true,
    };

    config
}
