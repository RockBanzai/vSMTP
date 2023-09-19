fn on_config(config) {
    config.broker.uri = env::variable("BROKER");
    config.broker.certificate_chain = "/etc/ssl/certs/ca-certificates.crt";

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
        want: "warn",
        async_io: "warn",
    };

    config
}
