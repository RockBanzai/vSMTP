fn on_config(config) {
    config.broker.uri = "amqp://guest:guest@broker.mydomain.tld:5672";
    config.scripts.path = "/etc/vsmtp/working/script.rhai";
    config
}
