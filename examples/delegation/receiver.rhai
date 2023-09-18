fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:25", "0.0.0.0:10025"],
    };
    config.broker.uri = "amqp://guest:guest@broker.mydomain.tld:5672";
    config.scripts.path = "/etc/vsmtp/receiver-smtp/filter.rhai";
    config
}
