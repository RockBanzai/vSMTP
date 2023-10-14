fn on_config(config) {
    config.broker.uri = "amqps://vsmtp-dev:password@rabbitmq.mydomain.tld:5671/vsmtp-dev";
    config.broker.extra_root_ca = crypto::load_cert("/etc/ssl/certs/ca-certificates.crt");
    config.scripts.path = "/rules.rhai";

    config
}
