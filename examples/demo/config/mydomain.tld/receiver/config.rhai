fn on_config(config) {
    config.name = "smtp-msa.mydomain.tld";
    config.interfaces = #{
        addr: ["0.0.0.0:25"],
        addr_submission: ["0.0.0.0:587"],
        addr_submissions: ["0.0.0.0:465"],
    };

    config.broker.uri = env::variable("BROKER");
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

    config.tls = #{
        handshake_timeout: "5s",
        protocol_version: ["TLSv1.2", "TLSv1.3"],
        root: #{
            certificate: "/etc/ssl/certs/smtp-msa.mydomain.tld.crt",
            private_key: "/etc/ssl/certs/smtp-msa.mydomain.tld.key",
        },
    };

    config.esmtp = #{
        pipelining: true,
        dsn: true,
        starttls: true,
        auth: #{
            mechanisms: ["Plain", "Login"],
        }
    };

    config
}
