fn on_config(config) {
    config.broker.uri = "amqp://guest:guest@example.com:5672";
    config.scripts.path = "/etc/vsmtp/working/script.rhai";
    config
}
