fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:25"],
        addr_submission: ["0.0.0.0:587"],
    };

    config.broker.uri = "amqp://guest:guest@example.com:5672";

    config.scripts.path = "/etc/vsmtp/receiver-smtp/filter.rhai";

    return config;
}

