fn on_config(config) {
    config.interfaces = #{
        addr: ["0.0.0.0:25"],
        addr_submission: ["0.0.0.0:587"],
    };

    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev";
    config.broker.extra_root_ca = crypto::load_cert("/etc/ssl/certs/ca-certificates.crt");
    config.scripts.path = "/etc/vsmtp/receiver-smtp/filter.rhai";

    config
}
