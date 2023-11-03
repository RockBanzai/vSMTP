fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:25"],
        addr_submission: ["0.0.0.0:587"],
        addr_submissions: ["0.0.0.0:465"],
    };

    config.broker.uri = env::variable("BROKER");
    config.broker.extra_root_ca = crypto::load_cert("/etc/ssl/certs/ca-certificates.crt");

    config.logs.default_level = "trace";
    config.logs.levels = #{
        lapin: "warn",
        pinky_swear: "warn",
        mio: "warn",
        polling: "warn",
        async_io: "warn",
    };

    config.esmtp = #{
        pipelining: true,
        dsn: true,
        starttls: true,
        auth: #{
            mechanisms: ["PLAIN", "LOGIN"],
        }
    };

    config.name = "mytarget.tld";

    config.tls = #{
        protocol_version: ["TLSv1.2", "TLSv1.3"],
        root: #{
            certificate: crypto::load_cert("/etc/ssl/certs/mytarget.tld.crt"),
            private_key: crypto::load_pem_rsa_pkcs8("/etc/ssl/certs/mytarget.tld.key"),
        }
    };

    config
}
