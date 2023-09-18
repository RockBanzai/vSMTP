fn on_config(config) {
    config.broker.uri = "amqp://guest:guest@broker.mydomain.tld:5672";
    config.service = "av";
    config.target = "smtp://clamsmtp:10026";
    config
}
