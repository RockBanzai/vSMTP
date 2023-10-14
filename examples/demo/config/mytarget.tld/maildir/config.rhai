fn on_config(config) {
    config.broker.uri = env::variable("BROKER");
    config.broker.extra_root_ca = crypto::load_cert("/etc/ssl/certs/ca-certificates.crt");
    config.user_lookup = "full_address";

    config.logs.default_level = "trace";
    config.logs.levels = #{
        lapin: "warn",
        pinky_swear: "warn",
        mio: "warn",
        polling: "warn",
        async_io: "warn",
    };

    config
}
