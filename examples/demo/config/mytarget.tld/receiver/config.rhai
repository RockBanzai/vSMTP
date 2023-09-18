fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:25"],
        addr_submission: ["0.0.0.0:587"],
        addr_submissions: ["0.0.0.0:465"],
    };

    config.broker.uri = "amqps://sink-mytarget:password@rabbitmq.mydomain.tld:5671/sink-mytarget";
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
        starttls: true,
    };

    config.tls = #{
        protocol_version: ["TLSv1.2", "TLSv1.3"],
        root: #{
            certificate: "/etc/ssl/certs/smtp-in.mytarget.tld.crt",
            private_key: "/etc/ssl/certs/smtp-in.mytarget.tld.key",
        }
    };

    config
}
