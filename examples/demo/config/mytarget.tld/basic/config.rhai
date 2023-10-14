fn on_config(config) {
    config.broker.uri = env::variable("BROKER");
    config.broker.extra_root_ca = crypto::load_cert("/etc/ssl/certs/ca-certificates.crt");

    config.dns.config = #{
        name_servers: [
            #{ socket_addr: "172.23.0.1:53" }
        ]
    };

    config.logs.default_level = "trace";
    config.logs.levels = #{
        lapin: "warn",
        pinky_swear: "warn",
        mio: "warn",
        polling: "warn",
        async_io: "warn",
    };

    config.extra_root_ca = crypto::load_cert("/etc/ssl/certs/mydomain.tld-root-ca.crt");

    config
}
